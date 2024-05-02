use log::{error, info};
use std::fs;
use std::path::PathBuf;

use crate::cmd::{command, CmdError, ExecError};
use crate::conf::Package;
use crate::Conf;
// use rayon::prelude::*;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DownloadError {
    #[error("System error: {0}")]
    System(#[from] ExecError),
    #[error("Cmd error: Erno: {0}")]
    Cmd(#[from] CmdError),
}

// IO error
// Cmd error
// Exec error

// Should return a list of packages to build

// const PARALLEL_DOWNLOAD: usize = 5;

fn download_pkg(conf: &Conf, pkg: &str) -> Result<(), DownloadError> {
    let pkgs_dir = conf.server_dir.join("pkgs");
    let (status, out) = command(
        &["pkgctl", "repo", "clone", "--protocol=https", &pkg],
        &pkgs_dir,
    )?;
    if status.success() {
        info!("[{}] Download package", pkg);
        Ok(())
    } else {
        error!("[{}] Failed to download", pkg);
        Err(CmdError::from_output(out))?
    }
}

fn update_pkg(pkg: &str, pkg_dir: &PathBuf, force_rebuild: bool) -> Result<bool, DownloadError> {
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
    if previous.get(0) != new.get(0) || force_rebuild {
        Ok(true)
    } else {
        Ok(false)
    }
}

pub fn download_all<'a>(conf: &'a Conf, force_rebuild: bool) -> Vec<(&'a String, &'a Package)> {
    let mut to_build: Vec<(&String, &Package)> = Vec::new();
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
                match update_pkg(name, &pkg_dir, force_rebuild) {
                    Ok(true) => to_build.push((name, pkg)),
                    Ok(false) => {}
                    Err(e) => {
                        error!("[{}] Failed to update: {}", name, e);
                    }
                }
            } else {
                if exist {
                    fs::remove_dir_all(pkg_dir).ok();
                }
                match download_pkg(conf, name) {
                    Ok(()) => to_build.push((name, pkg)),
                    Err(_) => {}
                }
            }
            // })
            // .collect::<Vec<&Package>>(),
            // )
        });
    to_build
}
