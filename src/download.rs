use std::fs;
use std::process::Command;

use crate::cmd::command;
use crate::Conf;

// Should return a list of packages to build

pub fn download_all<'a>(conf: &'a Conf, force_rebuild: bool) -> Vec<&'a String> {
    let pkgs_dir = conf.server_dir.join("pkgs");
    fs::create_dir_all(&pkgs_dir).unwrap();
    let mut to_build = Vec::new();
    // git pull otherwise , clone
    let mut to_update = Vec::new();
    for pkg in &conf.packages {
        let pkg_dir = conf.pkg_dir(pkg);
        // TODO: more test: .git exist and PKGBUILD exist
        if pkg_dir.exists() {
            if pkg_dir.join(".git").exists() && pkg_dir.join("PKGBUILD").exists() {
                to_update.push(pkg);
            } else {
                fs::remove_dir_all(pkg_dir).ok();
                to_build.push(pkg);
            }
        } else {
            to_build.push(pkg);
        }
    }
    if !to_build.is_empty() {
        println!("Downloading the following packages: {:?}", to_build);
        let mut cmd = Command::new("pkgctl");
        cmd.current_dir(&pkgs_dir)
            .args(["repo", "clone", "--protocol=https"])
            .args(&to_build);
        let (status, _) = command(cmd).unwrap();
        if !status.success() {
            eprintln!("Failed to download packages",)
        }
    }

    // TODO(perf): parallelez
    for pkg in to_update {
        /* Getting the previous version */
        println!("[{}] git rev-parse HEAD", pkg);
        let mut cmd = Command::new("git");
        cmd.current_dir(&conf.pkg_dir(pkg))
            .args(["rev-parse", "HEAD"]);
        let (status, previous) = command(cmd).unwrap();

        println!("[{}] git pull", pkg);
        let mut cmd = Command::new("git");
        cmd.current_dir(&conf.pkg_dir(pkg)).arg("pull");
        let (status, out) = command(cmd).unwrap();
        if !status.success() {
            eprintln!("Failed to download packages",)
        }

        println!("[{}] git rev-parse HEAD", pkg);
        /* Getting the new version */
        let mut cmd = Command::new("git");
        cmd.current_dir(&conf.pkg_dir(pkg))
            .args(["rev-parse", "HEAD"]);
        let (status, new) = command(cmd).unwrap();
        if previous.get(0) != new.get(0) || force_rebuild {
            to_build.push(pkg)
        }
    }
    to_build
}
