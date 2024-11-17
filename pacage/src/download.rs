use crossbeam_channel::Sender;
use log::{error, info};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::Mutex;
use std::{fs, io, thread};

use crate::cmd::{command, CmdError, ExecError};
use crate::conf::{Conf, PkgsDir};
use crate::conf::{Package, Repo};
use crate::format::{ParsingError, SrcInfo};
use thiserror::Error;

// TODO: git goes brr: git clone --filter=tree:0 <repo>

#[derive(Debug, Error)]
pub enum DownloadError {
    #[error("System error: {0}")]
    System(#[from] ExecError),

    #[error("Cmd error: Erno: {0}")]
    Cmd(#[from] CmdError),

    // #[error("Failed to parse PKGBUILD : {0}")]
    // PkgBuild(#[from] io::Error),
    #[error("Not Found: {0:?}")]
    NotFound(Vec<String>),

    #[error("Parsing error: {0}")]
    Parsing(#[from] ParsingError),

    #[error("Missing PKGBUILD: {0}")]
    MissingPkgbuild(io::Error),
}

// IO error
// Cmd error
// Exec error

// Should return a list of packages to build

// const PARALLEL_DOWNLOAD: usize = 5;

pub fn fetch_pkg(pkgs_dir: &PkgsDir, name: &str, repo: &Repo) -> Result<SrcInfo, DownloadError> {
    let pkg_dir = pkgs_dir.pkg(name);
    if pkg_dir.exists() {
        fs::remove_dir_all(pkg_dir).ok();
    }
    // let pkgs_dir = conf.server_dir.join("pkgs");
    let (status, out, _) = match repo {
        Repo::None => command(
            &["pkgctl", "repo", "clone", "--protocol=https", name],
            pkgs_dir.path(),
            Some([("GIT_TERMINAL_PROMPT", "0")]),
        )?,
        Repo::Aur => command(
            &[
                "git",
                "clone",
                &format!("https://aur.archlinux.org/{}.git", name),
            ],
            pkgs_dir.path(),
            Some([("GIT_TERMINAL_PROMPT", "0")]),
        )?,
        Repo::Git(a) => command(
            &["git", "clone", &a],
            pkgs_dir.path(),
            Some([("GIT_TERMINAL_PROMPT", "0")]),
        )?,
        Repo::File(_d) => {
            unimplemented!()
        }
    };
    if status.success() {
        Ok(SrcInfo::new(pkgs_dir, name, false)?)
    } else {
        Err(DownloadError::NotFound(out))?
    }
}

// fn update_pkg(conf: &Conf, pkg: &str, pkg_dir: &PathBuf) -> Result<(bool, SrcInfo), DownloadError> {
//     info!("[{}] git rev-parse HEAD", pkg);
//     let (status, previous, _) = command(
//         &["git", "rev-parse", "HEAD"],
//         &pkg_dir,
//         None::<Vec<(String, String)>>,
//     )?;
//     if !status.success() {
//         return Err((CmdError::from_output(previous)).into());
//     };

//     info!("[{}] git pull", pkg);
//     let (status, out, _) = command(&["git", "pull"], &pkg_dir, NOENV)?;
//     if !status.success() {
//         Err(CmdError::from_output(out))?
//     }

//     info!("[{}] git rev-parse HEAD", pkg);
//     /* Getting the new version */
//     let (status, new, _) = command(&["git", "rev-parse", "HEAD"], pkg_dir, NOENV)?;
//     if !status.success() {
//         return Err((CmdError::from_output(new)).into());
//     }
//     let pkg_build = SrcInfo::new(conf, pkg)?;
//     if previous.get(0) != new.get(0) {
//         Ok((true, pkg_build))
//     } else {
//         Ok((false, pkg_build))
//     }
// }

// TODO: check for deps there
pub fn download_pkg(
    conf: &mut Conf,
    name: &str,
    continue_on_err: bool,
    ret: Sender<(SrcInfo, Package)>,
) -> Result<(), DownloadError> {
    let mut pkgs = BTreeSet::new();
    pkgs.insert(name.to_string());
    download_all(conf, pkgs, continue_on_err, ret)
}

pub fn download_all<'a>(
    conf: &'a mut Conf,
    pkgs: BTreeSet<String>,
    continue_on_err: bool,
    ret: Sender<(SrcInfo, Package)>,
) -> Result<(), DownloadError> {
    let mut pdone = HashSet::new();
    let mut perrored: HashMap<String, DownloadError> = HashMap::new();
    let done: Mutex<&mut HashSet<String>> = Mutex::new(&mut pdone);
    let errored = Mutex::new(&mut perrored);
    let max_par_dl = conf.max_par_dl;
    let pkgs_dir = conf.pkgs_dir();
    let pkgs = pkgs
        .iter()
        .map(|a| conf.resolve(a))
        .collect::<Vec<String>>();
    let pconf = Mutex::new(conf);
    let waiting = AtomicUsize::new(0);
    let (new_pkg, worker) = crossbeam_channel::unbounded::<Option<String>>();
    for pkg in pkgs {
        new_pkg.send(Some(pkg)).expect("recv should be there");
    }
    thread::scope(|s| {
        let ret = &ret;
        let pkgs_dir = &pkgs_dir;
        let new_pkg = &new_pkg;
        let worker = &worker;
        let conf = &pconf;
        let done = &done;
        let errored = &errored;
        let waiting = &waiting;
        for _ in 0..max_par_dl {
            s.spawn(move || {
                // If the channel is empty and everyone else is waiting close it
                loop {
                    if waiting.fetch_add(1, Ordering::Relaxed) == max_par_dl - 1
                        && worker.is_empty()
                    {
                        for _ in 0..(max_par_dl - 1) {
                            new_pkg.send(None).expect("Some one wasnt listenening");
                        }
                        return;
                    }
                    let Ok(Some(name)) = worker.recv() else {
                        return;
                    };
                    waiting.fetch_sub(1, Ordering::Relaxed);
                    {
                        let mut done = done.lock().unwrap();
                        let errored = errored.lock().unwrap();
                        if done.contains(name.as_str()) || errored.contains_key(name.as_str()) {
                            continue;
                        }
                        done.insert(name.clone());
                    }
                    info!("[{}] Downloading...", name);
                    let (need_deps, pkg) = {
                        let mut conf = conf.lock().unwrap();
                        conf.ensure_pkg(name.as_str());
                        let pkg = conf.get(name.as_str()).clone();
                        let need_deps = conf.need_deps(&pkg);
                        (need_deps, pkg)
                    };
                    let pkg_build = match fetch_pkg(pkgs_dir, &name, &pkg.repo) {
                        Ok(p) => p,
                        Err(e) => {
                            if continue_on_err {
                                errored.lock().unwrap().insert(name.clone(), e);
                                continue;
                            } else {
                                error!("[{}] fail to download: {}", name, e);
                                unimplemented!();
                                // return Err(e);
                            }
                        }
                    };
                    println!("need deps: {}", need_deps);
                    if need_deps {
                        let to_send = {
                            let conf = conf.lock().unwrap();
                            pkg_build
                                .deps
                                .iter()
                                .map(|a| conf.resolve(a))
                                .collect::<Vec<String>>()
                        };
                        for dep in to_send {
                            /* TODO: send */
                            new_pkg
                                .send(Some(dep))
                                .expect("Failed to queue no pkg to fetch");
                        }
                    }
                    info!("[{}] Downloaded", name);
                    // TODO: no clone
                    ret.send((pkg_build, pkg))
                        .expect("Failed to send fetched pkg");
                }
            });
        }
    });
    let errored = errored.into_inner().unwrap();
    if !errored.is_empty() {
        error!("Issues while downloading pkgs: ");
        for (name, e) in errored {
            error!("[{}] Failed: {:?}", name, e);
        }
    }
    Ok(())
}
