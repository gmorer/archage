use std::fs;
use std::path::Path;
use std::process::Command;

use crate::Conf;

const BUILD_SCRIPT_CONTENT: &str = std::include_str!("../resources/build_pkg.sh");
const BUILD_SCRIPT_FILE: &str = "archage_build.sh";

pub fn build_all(conf: &Conf) {
    fs::write(
        Path::new(&conf.server_dir).join(BUILD_SCRIPT_FILE),
        BUILD_SCRIPT_CONTENT,
    )
    .unwrap();
    let server_dir = conf.host_server_dir.as_deref();
    let server_dir = server_dir.unwrap_or(&conf.server_dir);
    let server_dir = server_dir.to_str().unwrap();
    let output = Command::new("podman-remote")
        .current_dir(&conf.server_dir)
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
        .output()
        .unwrap();
    if !output.status.success() {
        eprintln!("Fail to build: {}", String::from_utf8_lossy(&output.stderr))
    }
    println!(
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
