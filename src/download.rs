use log::{error, info};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{self, BufRead, BufReader};
use std::path::PathBuf;

use crate::cmd::{command, CmdError, ExecError};
use crate::conf::Package;
use crate::Conf;
// use rayon::prelude::*;
use thiserror::Error;

#[derive(Debug)]
pub struct SrcInfo {
    pub name: String,
    pub version: String,
    pub release: String,
    // TODO(feat): deps
    pub deps: Vec<String>,
    pub src: bool,
}

impl std::cmp::PartialEq for SrcInfo {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl std::hash::Hash for SrcInfo {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}
impl std::cmp::Eq for SrcInfo {}

// pub struct SrcInfoWithMakePkg<'a>(pub (SrcInfo, Option<&'a Makepkg>));

// impl std::cmp::PartialEq for SrcInfoWithMakePkg<'_> {
//     fn eq(&self, other: &Self) -> bool {
//         self.0 .0.name == other.0 .0.name
//     }
// }
// impl std::cmp::Eq for SrcInfoWithMakePkg<'_> {}

// impl std::hash::Hash for SrcInfoWithMakePkg<'_> {
//     fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
//         self.0 .0.name.hash(state);
//     }
// }

impl SrcInfo {
    pub fn new(conf: &Conf, pkg_name: &str) -> Result<Self, io::Error> {
        let mut name = None;
        let mut version = None;
        let mut release = None;
        let mut deps = Vec::new();
        let mut src = false;

        let path = conf.server_dir.join("pkgs").join(pkg_name).join(".SRCINFO");
        let file = fs::File::open(path)?;
        let reader = BufReader::new(file);
        for line in reader.lines() {
            if let Ok(line) = line {
                if let Some(n) = line.find('=') {
                    if n == line.len() {
                        continue;
                    }
                    let key = line[..n].trim();
                    let v = line[(n + 1)..].trim();
                    match key {
                        "pkgbase" => name = Some(v.to_string()),
                        "pkgver" => version = Some(v.to_string()),
                        "pkgrel" => release = Some(v.to_string()),
                        "depends" => deps.push(v.to_string()),
                        "source" => src = true,
                        _ => {}
                    }
                }
            }
        }
        if name.is_some() && version.is_some() && release.is_some() {
            return Ok(Self {
                name: name.unwrap(),
                version: version.unwrap(),
                release: release.unwrap(),
                deps,
                src,
            });
        }
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Missing field in pkgver, name: {:?} version: {:?} releasze: {:?}",
                name, version, release
            ),
        ))
    }
    pub fn check_hash(name: String) -> Self {
        Self {
            name,
            version: "".to_string(),
            release: "".to_string(),
            deps: Vec::new(),
            src: false,
        }
    }
}

#[derive(Debug, Error)]
pub enum DownloadError {
    #[error("System error: {0}")]
    System(#[from] ExecError),
    #[error("Cmd error: Erno: {0}")]
    Cmd(#[from] CmdError),
    #[error("Failed to parse PKGBUILD : {0}")]
    PkgBuild(#[from] io::Error),
}

// IO error
// Cmd error
// Exec error

// Should return a list of packages to build

// const PARALLEL_DOWNLOAD: usize = 5;

fn fetch_pkg(conf: &Conf, pkg: &str) -> Result<SrcInfo, DownloadError> {
    let pkgs_dir = conf.server_dir.join("pkgs");
    let (status, out, _) = command(
        &["pkgctl", "repo", "clone", "--protocol=https", &pkg],
        &pkgs_dir,
    )?;
    if status.success() {
        info!("[{}] Download package", pkg);
        Ok(SrcInfo::new(conf, pkg)?)
    } else {
        error!("[{}] Failed to download", pkg);
        Err(CmdError::from_output(out))?
    }
}

fn update_pkg(conf: &Conf, pkg: &str, pkg_dir: &PathBuf) -> Result<(bool, SrcInfo), DownloadError> {
    info!("[{}] git rev-parse HEAD", pkg);
    let (status, previous, _) = command(&["git", "rev-parse", "HEAD"], &pkg_dir)?;
    if !status.success() {
        return Err((CmdError::from_output(previous)).into());
    };

    info!("[{}] git pull", pkg);
    let (status, out, _) = command(&["git", "pull"], &pkg_dir)?;
    if !status.success() {
        Err(CmdError::from_output(out))?
    }

    info!("[{}] git rev-parse HEAD", pkg);
    /* Getting the new version */
    let (status, new, _) = command(&["git", "rev-parse", "HEAD"], pkg_dir)?;
    if !status.success() {
        return Err((CmdError::from_output(new)).into());
    }
    let pkg_build = SrcInfo::new(conf, pkg)?;
    if previous.get(0) != new.get(0) {
        Ok((true, pkg_build))
    } else {
        Ok((false, pkg_build))
    }
}

// TODO: check for deps there
pub fn download_pkg(
    conf: &Conf,
    name: &str,
    force_rebuild: bool,
) -> Result<HashSet<SrcInfo>, DownloadError> {
    let mut res: HashSet<SrcInfo> = HashSet::new();
    _download_pkg(conf, name, force_rebuild, &mut res).map(|_| res)
}

fn _download_pkg(
    conf: &Conf,
    name: &str,
    force_rebuild: bool,
    res: &mut HashSet<SrcInfo>,
) -> Result<(), DownloadError> {
    info!("Downloading the following package: {}", name);
    let pkg_dir = conf.pkg_dir(name);
    let exist = pkg_dir.exists();
    let pkg_build = if exist && pkg_dir.join(".git").exists() && pkg_dir.join("PKGBUILD").exists() {
        match update_pkg(conf, name, &pkg_dir) {
            Ok((true, pkg_build)) => pkg_build,
            Ok((false, pkg_build)) => {
                if force_rebuild {
                    pkg_build
                } else {
                    // False, we should check the db
                    info!("[{}] Already up to date", name);
                    return Ok(());
                }
            }
            Err(e) => {
                error!("[{}] Failed to update: {}", name, e);
                Err(e)?
            }
        }
    } else {
        if exist {
            fs::remove_dir_all(pkg_dir).ok();
        }
        fetch_pkg(conf, name)?
    };
    let deps = pkg_build.deps.clone();
    res.insert(pkg_build);
    if conf.need_deps(name) {
        info!(
            "[{}] Downloading the following dependencies: {}",
            name,
            deps.join(" ")
        );
        for dep in deps {
            if !res.contains(&SrcInfo::check_hash(dep.clone())) {
                _download_pkg(conf, &dep, force_rebuild, res)?
            }
        }
    }
    Ok(())
}

// TODO: HashSet instead of hashmap
pub fn download_all<'a>(
    conf: &'a Conf,
    pkgs: &'a HashMap<String, Package>,
    force_rebuild: bool,
    continue_on_err: bool,
) -> Result<HashSet<SrcInfo>, DownloadError> {
    let mut to_build: HashSet<SrcInfo> = HashSet::new();
    for (name, _) in pkgs.iter() {
        match _download_pkg(conf, name, force_rebuild, &mut to_build) {
            Ok(()) => {}
            Err(e) => {
                error!("[{}] Fail to download: {}", name, e);
                if !continue_on_err {
                    return Err(e);
                }
            }
        }
    }
    Ok(to_build)
}
