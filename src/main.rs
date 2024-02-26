use std::fs;
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
            eprintln!("Fail to open pkg dir for {}: {}", name, e);
            return None;
        }
    };
    for entry in dir {
        let path = match entry {
            Ok(e) => e.file_name(),
            Err(e) => {
                eprintln!("Fail to check for file in {} pkg dir: {}", name, e);
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
    let db = Path::new(&conf.server_dir).join("pacage.db.tar.gz");
    for pkg in to_build {
        if let Some(package_file) = find_package(conf, &pkg) {
            // Move the package next to the db
            let moved_package_file = Path::new(&conf.server_dir).join(&package_file);
            let orig_package_file = conf.pkg_dir(pkg).join(&package_file);
            if let Err(e) = std::fs::rename(&orig_package_file, &moved_package_file) {
                eprintln!(
                    "Failed to move {} to {}: {}",
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
                    eprintln!("Failed to add {} to the db: {:?}", pkg, out);
                }
                Err(e) => {
                    eprintln!("Failed to add {} to the db: {}", pkg, e);
                }
            };
        } else {
            eprintln!("Failed to find {} package file", pkg);
        }
    }
}

fn to_string<T: std::string::ToString>(e: T) -> String {
    e.to_string()
}

fn init(args: &Args) -> Result<Conf, String> {
    let conf = Conf::new(args.conf.as_deref()).map_err(to_string)?;
    conf.print();
    fs::create_dir_all(&conf.server_dir).map_err(to_string)?;
    let pkgs_dir = conf.server_dir.join("pkgs");
    fs::create_dir_all(&pkgs_dir).map_err(to_string)?;
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
            eprintln!("Failed to init: {}", e);
            std::process::exit(2);
        }
    };

    println!("Downloading packages...");
    let to_build = if !args.skip_download {
        download::download_all(&conf, args.force_rebuild)
    } else {
        // Not nice
        conf.packages
            .iter()
            .map(|a| a.as_str())
            .collect::<Vec<&str>>()
    };
    println!("Building packages...");
    if let Err(e) = build::build(&conf, &to_build) {
        eprintln!("Failed to build packages: {}", e);
    }
    println!("Adding packages...");
    repo_add(&conf, to_build);
}
