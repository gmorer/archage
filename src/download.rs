use log::{error, info};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;

use crate::cmd::{command, CmdError, ExecError};
use crate::conf::{Package, Repo};
use crate::format::{ParsingError, SrcInfo};
use crate::Conf;
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
    #[error("Not Found")]
    NotFound(Vec<String>),

    #[error("Parsing error: {0}")]
    Parsing(#[from] ParsingError),
}

// IO error
// Cmd error
// Exec error

// Should return a list of packages to build

// const PARALLEL_DOWNLOAD: usize = 5;

pub fn fetch_pkg(conf: &Conf, pkg: &Package) -> Result<SrcInfo, DownloadError> {
    let pkg_dir = conf.pkg_dir(&pkg.name);
    if pkg_dir.exists() {
        fs::remove_dir_all(pkg_dir).ok();
    }
    let pkgs_dir = conf.server_dir.join("pkgs");
    let (status, out, _) = match &pkg.repo {
        Repo::None => command(
            &["pkgctl", "repo", "clone", "--protocol=https", &pkg.name],
            &pkgs_dir,
            Some([("GIT_TERMINAL_PROMPT", "0")]),
        )?,
        Repo::Aur => command(
            &[
                "git",
                "clone",
                &format!("https://aur.archlinux.org/{}.git", pkg.name),
            ],
            &pkgs_dir,
            Some([("GIT_TERMINAL_PROMPT", "0")]),
        )?,
        Repo::Git(a) => command(
            &["git", "clone", &a],
            &pkgs_dir,
            Some([("GIT_TERMINAL_PROMPT", "0")]),
        )?,
        Repo::File(_d) => {
            unimplemented!()
        }
    };
    if status.success() {
        info!("[{}] Download package", pkg.name);
        Ok(SrcInfo::new(conf, &pkg.name)?)
    } else {
        error!("[{}] Failed to download", pkg.name);
        Err(DownloadError::NotFound(out))?
        // Err(CmdError::from_output(out))?
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
) -> Result<HashSet<SrcInfo>, DownloadError> {
    let mut pkgs = BTreeSet::new();
    pkgs.insert(name.to_string());
    download_all(conf, pkgs, continue_on_err)
}

pub fn download_all<'a>(
    conf: &'a mut Conf,
    mut pkgs: BTreeSet<String>,
    continue_on_err: bool,
) -> Result<HashSet<SrcInfo>, DownloadError> {
    let mut done: HashMap<String, SrcInfo> = HashMap::new();
    let mut errored: HashMap<String, DownloadError> = HashMap::new();

    while let Some(pkg) = pkgs.pop_first() {
        if done.contains_key(&pkg) || errored.contains_key(&pkg) {
            continue;
        }
        info!("[{}] Downloading...", pkg);
        conf.ensure_pkg(pkg.as_str());
        let pkg = conf.get(pkg);
        let pkg_build = match fetch_pkg(conf, &pkg) {
            Ok(p) => p,
            Err(e) => {
                if continue_on_err {
                    errored.insert(pkg.name.clone(), e);
                    continue;
                } else {
                    return Err(e);
                }
            }
        };
        if conf.need_deps(&pkg) {
            for dep in &pkg_build.deps {
                if !done.contains_key(dep) && !errored.contains_key(dep) {
                    pkgs.insert(dep.clone());
                }
            }
        }
        info!("[{}] Downloaded", pkg.name);
        // TODO: no clone
        done.insert(pkg.name.clone(), pkg_build);
    }
    let mut res = HashSet::with_capacity(done.len());
    for (_pkg, infos) in done {
        // if infos.build {
        res.insert(infos);
        // } else {
        // info!("[{}] Wont be build, it cannot be", pkg);
        // }
    }
    if !errored.is_empty() {
        error!("Issues while downloading pkgs: ");
        for (name, e) in errored {
            error!("[{}] Failed: {:?}", name, e);
        }
    }
    Ok(res)
}
