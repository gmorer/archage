use log::{error, LevelFilter};
use std::fmt::Display;
use std::fs::{self, create_dir_all};
use std::path::Path;
use std::time::Duration;

pub mod conf;
pub use conf::Conf;

pub mod utils;

// pub mod builder;
pub mod patch;

pub mod cmd;

pub mod cli;
use cli::Cli;

pub mod builder;
mod db;

use crate::cli::CliCmd;

// pub mod build;

mod download;

const BUILD_SCRIPT_CONTENT: &str = std::include_str!("../resources/build_pkg.sh");
const BUILD_SCRIPT_FILE: &str = "pacage_build.sh";

pub struct DurationPrinter(Duration);

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

fn init(args: &Cli) -> Result<Conf, String> {
    env_logger::builder().filter_level(LevelFilter::Info).init();
    let conf =
        Conf::new(args.confdir.as_deref()).map_err(|e| format!("Failed to create conf: {}", e))?;
    create_dir_all(&conf.server_dir).map_err(|e| format!("Failed to create server dir: {}", e))?;
    let pkgs_dir = conf.server_dir.join("pkgs");
    create_dir_all(&pkgs_dir).map_err(|e| format!("Failed to create pkgs dir: {}", e))?;
    let srcs_dir = conf.server_dir.join("srcs");
    create_dir_all(&srcs_dir).map_err(|e| format!("Failed to create srcs dir: {}", e))?;
    if let Some(build_log_dir) = &conf.build_log_dir {
        create_dir_all(build_log_dir).map_err(|e| format!("Failed to create log dir: {}", e))?;
    }
    create_dir_all(conf.server_dir.join("repo"))
        .map_err(|e| format!("Failed to create repo dir: {}", e))?;
    create_dir_all(conf.server_dir.join("cache").join("pacman"))
        .map_err(|e| format!("Failed to create cache dir: {}", e))?;
    if conf
        .makepkg
        .as_ref()
        .is_some_and(|makepkg| makepkg.ccache.is_some_and(|a| a))
    {
        create_dir_all(conf.server_dir.join("cache").join("ccache"))
            .map_err(|e| format!("Failed to create ccache dir: {}", e))?;
    }
    fs::write(
        Path::new(&conf.server_dir).join(BUILD_SCRIPT_FILE),
        BUILD_SCRIPT_CONTENT,
    )
    .map_err(|e| format!("Failed to write build script: {}", e))?;
    Ok(conf)
}

fn main() {
    let args = Cli::get();

    let conf = match init(&args) {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to init: {}", e);
            std::process::exit(2);
        }
    };
    if let Err(e) = args.execute(conf) {
        std::process::exit(e)
    }

    /*
    info!("Downloading packages...");
    let to_build = if !args.skip_download {
        download::download_all(&conf, &conf.packages, args.force_rebuild)
    } else {
        // Only packages present on the file system
        let mut pkgs = HashSet::<PkgBuildWithMakePkg>::new();
        for (name, pkg) in conf.packages.iter() {
            if conf.pkg_dir(name.as_str()).exists() {
                match PkgBuild::new(&conf, &name) {
                    Ok(p) => {
                        pkgs.insert(PkgBuildWithMakePkg((p, pkg.makepkg.as_ref())));
                    }
                    Err(e) => {
                        error!("[{}] Failed to read pkgbuild: {}", name, e)
                    }
                }
            }
        }
        pkgs
    };
    info!("Building packages...");
    match build::build(&conf, to_build) {
        Ok(built) => {
            info!("Adding packages...");
            repo::add_all(&conf, built);
        }
        Err(e) => {
            error!("Failed to build packages: {}", e);
        }
    }
    */
}
