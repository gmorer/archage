use std::fs;
use std::path::PathBuf;
use std::process::Command;

use crate::cmd::{command, CmdError, ExecError};
use crate::Conf;
use rayon::prelude::*;
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

// TODO split ito smaller fn

const PARALLEL_DOWNLOAD: usize = 5;

fn download_pkg(conf: &Conf, pkg: &str) -> Result<(), DownloadError> {
    let pkgs_dir = conf.server_dir.join("pkgs");
    let mut cmd = Command::new("pkgctl");
    cmd.current_dir(&pkgs_dir)
        .args(["repo", "clone", "--protocol=https", &pkg]);
    let (status, out) = command(cmd)?;
    if status.success() {
        println!("Download package: {}", pkg);
        Ok(())
    } else {
        eprintln!("Failed to download {}", pkg);
        Err(CmdError::from_output(out))?
    }
}

fn update_pkg(pkg: &str, pkg_dir: &PathBuf, force_rebuild: bool) -> Result<bool, DownloadError> {
    println!("[{}] git rev-parse HEAD", pkg);
    let mut cmd = Command::new("git");
    cmd.current_dir(pkg_dir).args(["rev-parse", "HEAD"]);
    let (status, previous) = command(cmd)?;
    if !status.success() {
        return Err((CmdError::from_output(previous)).into());
    };

    println!("[{}] git pull", pkg);
    let mut cmd = Command::new("git");
    cmd.current_dir(pkg_dir).arg("pull");
    let (status, out) = command(cmd)?;
    if !status.success() {
        Err(CmdError::from_output(out))?
    }

    println!("[{}] git rev-parse HEAD", pkg);
    /* Getting the new version */
    let mut cmd = Command::new("git");
    cmd.current_dir(pkg_dir).args(["rev-parse", "HEAD"]);
    let (status, new) = command(cmd)?;
    if !status.success() {
        return Err((CmdError::from_output(new)).into());
    }
    if previous.get(0) != new.get(0) || force_rebuild {
        Ok(true)
    } else {
        Ok(false)
    }
}

pub fn download_all<'a>(conf: &'a Conf, force_rebuild: bool) -> Vec<&'a str> {
    let mut to_build: Vec<&str> = Vec::new();
    conf.packages.chunks(PARALLEL_DOWNLOAD).for_each(|chunk| {
        println!("Downloading the following packages: {:?}", chunk);
        to_build.append(
            &mut chunk
                .into_par_iter()
                .filter_map(|pkg| {
                    let pkg = pkg.as_str();
                    let pkg_dir = conf.pkg_dir(pkg);
                    let exist = pkg_dir.exists();
                    if exist && pkg_dir.join(".git").exists() && pkg_dir.join("PKGBUILD").exists() {
                        match update_pkg(pkg, &pkg_dir, force_rebuild) {
                            Ok(true) => Some(pkg),
                            Ok(false) => None,
                            Err(e) => {
                                eprintln!("Failed to update {}: {}", pkg, e);
                                None
                            }
                        }
                    } else {
                        if exist {
                            fs::remove_dir_all(pkg_dir).ok();
                        }
                        match download_pkg(conf, pkg) {
                            Ok(()) => Some(pkg),
                            Err(e) => {
                                eprintln!("Failed to download {}: {}", pkg, e);
                                None
                            }
                        }
                    }
                })
                .collect::<Vec<&str>>(),
        )
    });
    to_build
}
