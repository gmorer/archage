use log::{error, info, LevelFilter};
use std::fs::{self, create_dir_all};
use std::path::Path;

pub mod conf;
pub use conf::Conf;

pub mod cmd;

pub mod cli;
use cli::Args;

mod repo;

use crate::conf::Package;

pub mod build;

mod download;

const BUILD_SCRIPT_CONTENT: &str = std::include_str!("../resources/build_pkg.sh");
const BUILD_SCRIPT_FILE: &str = "pacage_build.sh";

fn init(args: &Args) -> Result<Conf, String> {
    env_logger::builder().filter_level(LevelFilter::Info).init();
    let conf =
        Conf::new(args.conffile.as_deref()).map_err(|e| format!("Failed to create conf: {}", e))?;
    conf.print();
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
    let args = Args::get();

    let conf = match init(&args) {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to init: {}", e);
            std::process::exit(2);
        }
    };

    if args.list_pkgs {
        for pkg in repo::list(&conf) {
            pkg.print()
        }
        return;
    }

    info!("Downloading packages...");
    let to_build = if !args.skip_download {
        download::download_all(&conf, args.force_rebuild)
    } else {
        // Only packages present on the file system
        conf.packages
            .iter()
            .filter(|(name, _)| conf.pkg_dir(name.as_str()).exists())
            .collect::<Vec<(&String, &Package)>>()
    };
    info!("Building packages...");
    match build::build(&conf, to_build) {
        Ok(built) => {
            info!("Adding packages...");
            repo::add(&conf, built);
        }
        Err(e) => {
            error!("Failed to build packages: {}", e);
        }
    }
}
