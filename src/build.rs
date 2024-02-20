use std::fs;
use std::path::Path;
use std::process::Command;

use crate::cmd::command;
use crate::Conf;

const BUILD_SCRIPT_CONTENT: &str = std::include_str!("../resources/build_pkg.sh");
const BUILD_SCRIPT_FILE: &str = "pacage_build.sh";

pub fn build(conf: &Conf, pkgs: &Vec<&String>) {
    if pkgs.is_empty() {
        println!("Nothing to build");
        return;
    }
    fs::write(
        Path::new(&conf.server_dir).join(BUILD_SCRIPT_FILE),
        BUILD_SCRIPT_CONTENT,
    )
    .unwrap();
    fs::write(
        Path::new(&conf.server_dir).join("makepkg.conf"),
        conf.makepkg.to_file(),
    )
    .unwrap();
    let server_dir = conf.host_server_dir.as_deref();
    let server_dir = server_dir.unwrap_or(&conf.server_dir);
    let server_dir = server_dir.to_str().unwrap();
    let mut cmd = Command::new("podman-remote");
    println!("pkgs => {:?}", pkgs);
    cmd.current_dir(&conf.server_dir)
        .args([
            "run",
            "--rm",
            "-it",
            &format!("-v={}:/build", server_dir),
            "--workdir=/build",
            "--env=HOME=/tmp",
            "archlinux:base-devel",
            "bash",
            &format!("/build/{}", BUILD_SCRIPT_FILE),
        ])
        .args(pkgs);
    let (status, _) = command(cmd).unwrap();

    if !status.success() {
        eprintln!("Fail to build")
    }
}
