use log::{error, info};
use std::collections::HashSet;
use std::fs;
use std::io::{self, BufRead, BufReader};
use std::path::PathBuf;

use crate::cmd::{command, CmdError, ExecError};
use crate::conf::Makepkg;
use crate::Conf;
// use rayon::prelude::*;
use thiserror::Error;

#[derive(Debug)]
pub struct PkgBuild {
    pub name: String,
    pub version: String,
    pub release: String,
    // TODO(feat): deps
    // deps: Vec<String>,
}

impl std::cmp::PartialEq for PkgBuild {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl std::hash::Hash for PkgBuild {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}
impl std::cmp::Eq for PkgBuild {}

pub struct PkgBuildWithMakePkg<'a>(pub (PkgBuild, Option<&'a Makepkg>));

impl std::cmp::PartialEq for PkgBuildWithMakePkg<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.0 .0.name == other.0 .0.name
    }
}
impl std::cmp::Eq for PkgBuildWithMakePkg<'_> {}

impl std::hash::Hash for PkgBuildWithMakePkg<'_> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0 .0.name.hash(state);
    }
}

impl PkgBuild {
    pub fn new(conf: &Conf, pkg_name: &str) -> Result<Self, io::Error> {
        let mut name = None;
        let mut version = None;
        let mut release = None;

        let path = conf.server_dir.join("pkgs").join(pkg_name).join("PKGBUILD");
        let file = fs::File::open(path)?;
        let reader = BufReader::new(file);
        for line in reader.lines() {
            if let Ok(line) = line {
                if name.is_none() && line.starts_with("pkgname=") {
                    name = Some(line[8..].to_string());
                } else if version.is_none() && line.starts_with("pkgver=") {
                    version = Some(line[7..].to_string());
                } else if release.is_none() && line.starts_with("pkgrel=") {
                    release = Some(line[7..].to_string());
                }
                if name.is_some() && version.is_some() && release.is_some() {
                    return Ok(Self {
                        name: name.unwrap(),
                        version: version.unwrap(),
                        release: release.unwrap(),
                    });
                }
            }
        }
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Missing field in pkgver, name: {:?} version: {:?} releasze: {:?}",
                name, version, release
            ),
        ))
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

fn download_pkg(conf: &Conf, pkg: &str) -> Result<PkgBuild, DownloadError> {
    let pkgs_dir = conf.server_dir.join("pkgs");
    let (status, out) = command(
        &["pkgctl", "repo", "clone", "--protocol=https", &pkg],
        &pkgs_dir,
    )?;
    if status.success() {
        info!("[{}] Download package", pkg);
        Ok(PkgBuild::new(conf, pkg)?)
    } else {
        error!("[{}] Failed to download", pkg);
        Err(CmdError::from_output(out))?
    }
}

fn update_pkg(
    conf: &Conf,
    pkg: &str,
    pkg_dir: &PathBuf,
    force_rebuild: bool,
) -> Result<(bool, PkgBuild), DownloadError> {
    info!("[{}] git rev-parse HEAD", pkg);
    let (status, previous) = command(&["git", "rev-parse", "HEAD"], &pkg_dir)?;
    if !status.success() {
        return Err((CmdError::from_output(previous)).into());
    };

    info!("[{}] git pull", pkg);
    let (status, out) = command(&["git", "pull"], &pkg_dir)?;
    if !status.success() {
        Err(CmdError::from_output(out))?
    }

    info!("[{}] git rev-parse HEAD", pkg);
    /* Getting the new version */
    let (status, new) = command(&["git", "rev-parse", "HEAD"], pkg_dir)?;
    if !status.success() {
        return Err((CmdError::from_output(new)).into());
    }
    let pkg_build = PkgBuild::new(conf, pkg)?;
    if previous.get(0) != new.get(0) || force_rebuild {
        Ok((true, pkg_build))
    } else {
        Ok((false, pkg_build))
    }
}

pub fn download_all<'a>(conf: &'a Conf, force_rebuild: bool) -> HashSet<PkgBuildWithMakePkg> {
    let mut to_build: HashSet<PkgBuildWithMakePkg> = HashSet::new();
    conf.packages
        .iter()
        // .chunks(PARALLEL_DOWNLOAD)
        .for_each(|(name, pkg)| {
            info!("Downloading the following package: {:?}", name);
            // to_build.append(
            // &mut chunk
            // .into_par_iter()
            // .filter_map(|pkg| {
            // let name = pkg.name.as_str();
            let pkg_dir = conf.pkg_dir(name);
            let exist = pkg_dir.exists();
            if exist && pkg_dir.join(".git").exists() && pkg_dir.join("PKGBUILD").exists() {
                match update_pkg(conf, name, &pkg_dir, force_rebuild) {
                    Ok((true, pkg_build)) => {
                        to_build.insert(PkgBuildWithMakePkg((pkg_build, pkg.makepkg.as_ref())));
                    }
                    Ok((false, _)) => {}
                    Err(e) => {
                        error!("[{}] Failed to update: {}", name, e);
                    }
                }
            } else {
                if exist {
                    fs::remove_dir_all(pkg_dir).ok();
                }
                match download_pkg(conf, name) {
                    Ok(pkgbuild) => {
                        to_build.insert(PkgBuildWithMakePkg((pkgbuild, pkg.makepkg.as_ref())));
                    }
                    Err(_) => {}
                }
            }
            // })
            // .collect::<Vec<&Package>>(),
            // )
        });
    to_build
}
