use log::{error, info, LevelFilter};
use std::fs::{self, create_dir_all};
use std::path::Path;
use std::process::Command;

pub mod conf;
pub use conf::Conf;

pub mod cmd;
use cmd::command;

pub mod cli;
use cli::Args;

pub mod build;

mod download;

const BUILD_SCRIPT_CONTENT: &str = std::include_str!("../resources/build_pkg.sh");
const BUILD_SCRIPT_FILE: &str = "pacage_build.sh";

fn find_package(conf: &Conf, name: &str) -> Option<String> {
    // TODO: symlink instead?
    let dir = match fs::read_dir(conf.pkg_dir(name)) {
        Ok(d) => d,
        Err(e) => {
            error!("[{}] Fail to open pkg dir: {}", name, e);
            return None;
        }
    };
    for entry in dir {
        let path = match entry {
            Ok(e) => e.file_name(),
            Err(e) => {
                error!("[{}] Fail to check for files in pkg dir: {}", name, e);
                return None;
            }
        };
        let path = String::from_utf8_lossy(path.as_encoded_bytes());
        if path.ends_with(".pkg.tar.zst") {
            return Some(path.to_string());
        }
    }
    None
}

// aerc-0.16.0-1-x86_64.pkg.tar.zst
fn repo_add(conf: &Conf, to_build: Vec<&str>) {
    if to_build.is_empty() {
        info!("Nothing to add");
        return;
    }
    let db = Path::new(&conf.server_dir).join("pacage.db.tar.gz");
    for pkg in to_build {
        if let Some(package_file) = find_package(conf, &pkg) {
            // Move the package next to the db
            let moved_package_file = Path::new(&conf.server_dir).join(&package_file);
            let orig_package_file = conf.pkg_dir(pkg).join(&package_file);
            if let Err(e) = std::fs::rename(&orig_package_file, &moved_package_file) {
                error!(
                    "[{}] Failed to move {} to {}: {}",
                    pkg,
                    orig_package_file.display(),
                    moved_package_file.display(),
                    e
                );
                continue;
            }
            let mut cmd = Command::new("repo-add");
            cmd.current_dir(&conf.server_dir)
                .args([&db, &moved_package_file]);
            match command(cmd) {
                Ok((status, _)) if status.success() => {}
                Ok((_, out)) => {
                    error!("[{}] Failed to add the package to the db: {:?}", pkg, out);
                }
                Err(e) => {
                    error!("[{}] Failed to add to the db: {}", pkg, e);
                }
            };
        } else {
            error!("[{}] Failed to find package file", pkg);
        }
    }
}

fn to_string<T: std::string::ToString>(e: T) -> String {
    e.to_string()
}

fn init(args: &Args) -> Result<Conf, String> {
    env_logger::builder().filter_level(LevelFilter::Info).init();
    let conf = Conf::new(args.conf.as_deref()).map_err(to_string)?;
    conf.print();
    create_dir_all(&conf.server_dir).map_err(to_string)?;
    let pkgs_dir = conf.server_dir.join("pkgs");
    create_dir_all(&pkgs_dir).map_err(to_string)?;
    info!("BUILDL LGO DIR: {:?}", conf.build_log_dir);
    if let Some(build_log_dir) = &conf.build_log_dir {
        create_dir_all(build_log_dir).map_err(to_string)?;
    }
    fs::write(
        Path::new(&conf.server_dir).join("makepkg.conf"),
        conf.makepkg.to_file().map_err(to_string)?,
    )
    .map_err(to_string)?;
    fs::write(
        Path::new(&conf.server_dir).join(BUILD_SCRIPT_FILE),
        BUILD_SCRIPT_CONTENT,
    )
    .map_err(to_string)?;
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

    info!("Downloading packages...");
    let to_build = if !args.skip_download {
        download::download_all(&conf, args.force_rebuild)
    } else {
        // Not nice
        conf.packages
            .iter()
            .map(|a| a.as_str())
            .collect::<Vec<&str>>()
    };
    info!("Building packages...");
    match build::build(&conf, to_build) {
        Ok(builded) => {
            info!("Adding packages...");
            repo_add(&conf, builded);
        }
        Err(e) => {
            error!("Failed to build packages: {}", e);
        }
    }
}
