use log::{error, info};
use std::collections::HashSet;
use std::fmt::Display;
use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::Path;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use thiserror::Error;

use crate::cmd::{command, write_last_lines, CmdError, ExecError};
use crate::conf::Makepkg;
use crate::download::{PkgBuild, PkgBuildWithMakePkg};
use crate::patch::{patch, PatchError};
use crate::{Conf, BUILD_SCRIPT_FILE};

#[derive(Debug, Error)]
pub enum BuildError {
    #[error("System error: {0}")]
    ExecError(#[from] ExecError),
    #[error("Cmd error: {0}")]
    CmdError(#[from] CmdError),
    #[error("IO error: {0}")]
    IOError(#[from] io::Error),
    #[error("Patch error: {0}")]
    PatchError(#[from] PatchError),
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
    action: &str,
    out: &Vec<String>,
    success: bool,
) -> Result<Option<String>, io::Error> {
    if let Some(build_log_dir) = &conf.build_log_dir {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let path = build_log_dir.join(format!(
            "{}_{}_{}_{}.log",
            pkg,
            action,
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

pub fn build_pkg(
    conf: &Conf,
    pkg: &PkgBuild,
    makepkgconf: Option<&Makepkg>,
) -> Result<(), BuildError> {
    fs::write(
        Path::new(&conf.server_dir).join("makepkg.conf"),
        Makepkg::get_conf_file(&conf, makepkgconf, &pkg.name)?,
    )?;
    let start = Instant::now();
    info!("[{}] Downloading the sources...", pkg.name);
    let (status, out) = command(
        &[
            &conf.container_runner,
            "exec",
            "--workdir=/build",
            "--env=HOME=/tmp",
            "--env=CCACHE_DIR=/build/cache/ccache/",
            CONTAINER_NAME,
            "bash",
            &format!("/build/{}", BUILD_SCRIPT_FILE),
            "get",
            &pkg.name,
        ],
        &conf.server_dir,
    )?;
    match out_to_file(conf, &pkg.name, "get", &out, status.success()) {
        Ok(Some(file)) => info!("[{}] Get logs writed to {}", pkg.name, file),
        Ok(None) => {}
        Err(e) => error!("[{}] Failed to write output to logs: {}", pkg.name, e),
    }
    if !status.success() {
        error!("[{}] Failed to get sources ", pkg.name,);
        write_last_lines(&out, 10);
        Err(CmdError::from_output(out))?
    }
    patch(conf, pkg)?;
    info!("[{}] Building/packaging the sources...", pkg.name);
    let (status, out) = command(
        &[
            &conf.container_runner,
            "exec",
            "--workdir=/build",
            "--env=HOME=/tmp",
            "--env=CCACHE_DIR=/build/cache/ccache/",
            CONTAINER_NAME,
            "bash",
            &format!("/build/{}", BUILD_SCRIPT_FILE),
            "build",
            &pkg.name,
        ],
        &conf.server_dir,
    )?;
    match out_to_file(conf, &pkg.name, "build", &out, status.success()) {
        Ok(Some(file)) => info!("[{}] Build logs writed to {}", pkg.name, file),
        Ok(None) => {}
        Err(e) => error!("[{}] Failed to write output to logs: {}", pkg.name, e),
    }
    let elapsed = start.elapsed();
    if !status.success() {
        error!(
            "[{}] Failed to build in {} ->",
            pkg.name,
            DurationPrinter(elapsed)
        );
        write_last_lines(&out, 10);
        Err(CmdError::from_output(out))?
    } else {
        info!(
            "[{}] Build sucessfull in {}",
            pkg.name,
            DurationPrinter(elapsed)
        );
        Ok(())
    }
}

pub fn start_builder(conf: &Conf) -> Result<(), BuildError> {
    let server_dir = conf.host_server_dir.as_deref();
    let server_dir = String::from_utf8_lossy(
        server_dir
            .unwrap_or(&conf.server_dir)
            .as_os_str()
            .as_encoded_bytes(),
    );
    // Stop previous builds
    command(
        &[&conf.container_runner, "stop", CONTAINER_NAME],
        &conf.server_dir,
    )
    .ok();
    command(
        &[&conf.container_runner, "rm", CONTAINER_NAME],
        &conf.server_dir,
    )
    .ok();

    let (status, out) = command(
        &[
            &conf.container_runner,
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
        ],
        &conf.server_dir,
    )?;
    if !status.success() {
        error!("Fail to spawn builder");
        Err(CmdError::from_output(out))?;
    }
    let (status, out) = command(
        &[
            &conf.container_runner,
            "exec",
            "--workdir=/build",
            "--env=HOME=/tmp",
            "--env=CCACHE_DIR=/build/cache/ccache/",
            CONTAINER_NAME,
            "bash",
            &format!("/build/{}", BUILD_SCRIPT_FILE),
            "start",
        ],
        &conf.server_dir,
    )?;
    match out_to_file(conf, "pacage_builder", "start", &out, status.success()) {
        Ok(Some(file)) => info!("Start logs writed to {}", file),
        Ok(None) => {}
        Err(e) => error!("Failed to write output to logs: {}", e),
    }
    if !status.success() {
        error!("Failed to start builder");
        Err(CmdError::from_output(out))?;
    }
    Ok(())
}

pub fn build<'a, 'b>(
    conf: &'a Conf,
    pkgs: HashSet<PkgBuildWithMakePkg<'a>>,
) -> Result<HashSet<PkgBuild>, BuildError>
where
    'a: 'b,
{
    let mut built: HashSet<PkgBuild> = HashSet::new();
    if pkgs.is_empty() {
        info!("Nothing to build");
        // return Ok(built);
        unimplemented!()
    }

    info!("Initiating builder container...");
    start_builder(conf)?;
    info!("Builder container initiated");

    for PkgBuildWithMakePkg((pkg, makepkg)) in pkgs {
        info!("[{}] Starting build...", pkg.name);
        if let Ok(()) = build_pkg(conf, &pkg, makepkg) {
            built.insert(pkg);
        }
    }

    info!("Stoping builder...");
    let (status, out) = command(
        &[&conf.container_runner, "stop", CONTAINER_NAME],
        &conf.server_dir,
    )?;
    if !status.success() {
        error!("Failed to stop builder ->");
        write_last_lines(&out, 10);
    }
    info!("Builder stoped");

    Ok(built)
}
