use std::process::Command;
use thiserror::Error;

use crate::cmd::{command, CmdError, ExecError};
use crate::{Conf, BUILD_SCRIPT_FILE};

#[derive(Debug, Error)]
pub enum BuildError {
    #[error("System error: {0}")]
    ExecError(#[from] ExecError),
    #[error("IO error: {0}")]
    CmdError(#[from] CmdError),
}

pub fn build(conf: &Conf, pkgs: &Vec<&str>) -> Result<(), BuildError> {
    if pkgs.is_empty() {
        println!("Nothing to build");
        return Ok(());
    }
    let server_dir = conf.host_server_dir.as_deref();
    let server_dir = String::from_utf8_lossy(
        server_dir
            .unwrap_or(&conf.server_dir)
            .as_os_str()
            .as_encoded_bytes(),
    );
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
    let (status, out) = command(cmd)?;

    if !status.success() {
        eprintln!("Fail to build");
        Err(CmdError::from_output(out))?
    }
    Ok(())
}
