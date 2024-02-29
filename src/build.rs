use log::{error, info};
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
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

// TODO: version
// log file name: pkg_sucesss_timestamp.log

const CONTAINER_NAME: &str = "pacage_builder";

pub fn out_to_file(
    conf: &Conf,
    pkg: &str,
    out: &Vec<String>,
    success: bool,
) -> Result<(), io::Error> {
    if let Some(build_log_dir) = &conf.build_log_dir {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let path = build_log_dir.join(format!(
            "{}_{}_{}.log",
            pkg,
            if success { "SUCESS" } else { "ERROR" },
            ts
        ));
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);
        for line in out {
            writer.write(line.as_bytes())?;
            writer.write(b"\n")?;
        }
    }
    Ok(())
}

pub fn build_pkg(conf: &Conf, pkg: &str) -> Result<(), BuildError> {
    let mut pkg_cmd = Command::new("podman-remote");
    pkg_cmd.current_dir(&conf.server_dir).args([
        "exec",
        "--workdir=/build",
        "--env=HOME=/tmp",
        CONTAINER_NAME,
        "bash",
        &format!("/build/{}", BUILD_SCRIPT_FILE),
        pkg,
    ]);
    let (status, out) = command(pkg_cmd)?;
    if let Err(e) = out_to_file(conf, pkg, &out, status.success()) {
        error!("[{}] Failed to write output to logs: {}", pkg, e);
    }
    if !status.success() {
        error!("[{}] Failed to build: {:?}", pkg, out);
        Ok(())
    } else {
        info!("[{}] Build sucessfull", pkg);
        Err(CmdError::from_output(out))?
    }
}

pub fn build<'a>(conf: &'a Conf, pkgs: Vec<&'a str>) -> Result<Vec<&'a str>, BuildError> {
    let mut builded = Vec::new();
    if pkgs.is_empty() {
        info!("Nothing to build");
        return Ok(builded);
    }

    // Stop previous builds
    let mut stop_cmd = Command::new("podman-remote");
    stop_cmd
        .current_dir(&conf.server_dir)
        .args(["stop", CONTAINER_NAME]);
    command(stop_cmd).ok();

    let server_dir = conf.host_server_dir.as_deref();
    let server_dir = String::from_utf8_lossy(
        server_dir
            .unwrap_or(&conf.server_dir)
            .as_os_str()
            .as_encoded_bytes(),
    );
    info!("Starting builder container");
    match Command::new("podman-remote")
        .current_dir(&conf.server_dir)
        .args([
            "run",
            "--rm",
            "--name",
            CONTAINER_NAME,
            "-d", // detach
            &format!("-v={}:/build", server_dir),
            "archlinux:base-devel",
            "sh",
            "-c",
            "sleep infinity",
        ])
        .stdout(std::process::Stdio::null())
        .status()
    {
        Ok(status) => {
            if !status.success() {
                error!("Fail to spawn builder");
                unimplemented!();
            }
        }
        Err(e) => {
            error!("builder spawn error: {}", e);
            unimplemented!();
        }
    }
    for pkg in pkgs {
        info!("[{}] Starting build...", pkg);
        if let Ok(()) = build_pkg(conf, pkg) {
            builded.push(pkg);
        }
    }

    let mut stop_cmd = Command::new("podman-remote");
    stop_cmd
        .current_dir(&conf.server_dir)
        .args(["stop", CONTAINER_NAME]);
    let (status, out) = command(stop_cmd)?;
    if !status.success() {
        error!("Failed to stop builder: {:?}", out);
    }

    Ok(builded)
}
