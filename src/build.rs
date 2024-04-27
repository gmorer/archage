use log::{error, info};
use std::fmt::Display;
use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use thiserror::Error;

use crate::cmd::{command, write_last_lines, CmdError, ExecError};
use crate::conf::Package;
use crate::{Conf, BUILD_SCRIPT_FILE};

#[derive(Debug, Error)]
pub enum BuildError {
    #[error("System error: {0}")]
    ExecError(#[from] ExecError),
    #[error("Cmd error: {0}")]
    CmdError(#[from] CmdError),
    #[error("IO error: {0}")]
    IOError(#[from] io::Error),
}

// TODO: version
// log file name: pkgname_successs_timestamp.log

const CONTAINER_NAME: &str = "pacage_builder";

struct DurationPrinter(Duration);

impl Display for DurationPrinter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let secs = self.0.as_secs();
        let hours = (secs / 3600) as u16;
        let minutes = ((secs / 60) % 60) as u16;
        let seconds = (secs % 60) as u16;
        if hours == 1 {
            write!(f, "{} hour ", hours)?;
        } else if hours > 1 {
            write!(f, "{} hours ", hours)?;
        }
        if minutes == 1 {
            write!(f, "{} minute ", minutes)?;
        } else if minutes > 1 {
            write!(f, "{} minutes ", minutes)?;
        }
        if seconds == 1 {
            write!(f, "{} second ", seconds)?;
        } else if seconds > 1 {
            write!(f, "{} seconds", seconds)?;
        }
        Ok(())
    }
}

pub fn out_to_file(
    conf: &Conf,
    pkg: &str,
    out: &Vec<String>,
    success: bool,
) -> Result<Option<String>, io::Error> {
    if let Some(build_log_dir) = &conf.build_log_dir {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let path = build_log_dir.join(format!(
            "{}_{}_{}.log",
            pkg,
            if success { "SUCCESS" } else { "ERROR" },
            ts
        ));
        let file = File::create(&path)?;
        let mut writer = BufWriter::new(file);
        for line in out {
            writer.write(line.as_bytes())?;
            writer.write(b"\n")?;
        }
        Ok(path.to_str().map(ToString::to_string))
    } else {
        Ok(None)
    }
}

pub fn build_pkg(conf: &Conf, name: &String, pkg: &Package) -> Result<(), BuildError> {
    // Write specific makepkg
    fs::write(
        Path::new(&conf.server_dir).join("makepkg.conf"),
        pkg.get_makepkg(&conf, name)?,
    )?;
    let mut pkg_cmd = Command::new(&conf.container_runner);
    let start = Instant::now();
    pkg_cmd.current_dir(&conf.server_dir).args([
        "exec",
        "--workdir=/build",
        "--env=HOME=/tmp",
        "--env=CCACHE_DIR=/build/cache/ccache/",
        CONTAINER_NAME,
        "bash",
        &format!("/build/{}", BUILD_SCRIPT_FILE),
        name,
    ]);
    let (status, out) = command(pkg_cmd)?;
    match out_to_file(conf, name, &out, status.success()) {
        Ok(Some(file)) => info!("[{}] Build logs writed to {}", name, file),
        Ok(None) => {}
        Err(e) => error!("[{}] Failed to write output to logs: {}", name, e),
    }
    let elapsed = start.elapsed();
    if !status.success() {
        error!(
            "[{}] Failed to build in {} ->",
            name,
            DurationPrinter(elapsed)
        );
        write_last_lines(&out, 10);
        Err(CmdError::from_output(out))?
    } else {
        info!(
            "[{}] Build sucessfull in {}",
            name,
            DurationPrinter(elapsed)
        );
        Ok(())
    }
}

pub fn build<'a>(
    conf: &'a Conf,
    pkgs: Vec<(&'a String, &'a Package)>,
) -> Result<Vec<&'a String>, BuildError> {
    let mut built = Vec::new();
    if pkgs.is_empty() {
        info!("Nothing to build");
        return Ok(built);
    }

    // Stop previous builds
    let mut stop_cmd = Command::new(&conf.container_runner);
    stop_cmd
        .current_dir(&conf.server_dir)
        .args(["stop", CONTAINER_NAME]);
    command(stop_cmd).ok();

    let mut stop_cmd = Command::new(&conf.container_runner);
    stop_cmd
        .current_dir(&conf.server_dir)
        .args(["rm", CONTAINER_NAME]);
    command(stop_cmd).ok();

    let server_dir = conf.host_server_dir.as_deref();
    let server_dir = String::from_utf8_lossy(
        server_dir
            .unwrap_or(&conf.server_dir)
            .as_os_str()
            .as_encoded_bytes(),
    );
    info!("Starting builder container");
    let mut builder_cmd = Command::new(&conf.container_runner);
    builder_cmd.current_dir(&conf.server_dir).args([
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
    ]);
    let (status, out) = command(builder_cmd)?;
    if !status.success() {
        error!("Fail to spawn builder");
        Err(CmdError::from_output(out))?;
    }
    for (name, pkg) in pkgs {
        info!("[{}] Starting build...", name);
        if let Ok(()) = build_pkg(conf, name, pkg) {
            built.push(name);
        }
    }

    let mut stop_cmd = Command::new(&conf.container_runner);
    stop_cmd
        .current_dir(&conf.server_dir)
        .args(["stop", CONTAINER_NAME]);
    let (status, out) = command(stop_cmd)?;
    if !status.success() {
        error!("Failed to stop builder ->");
        write_last_lines(&out, 10);
    }

    Ok(built)
}
