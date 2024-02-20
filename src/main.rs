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

fn find_package(conf: &Conf, name: &str) -> Option<String> {
    // TODO: symlink instead?
    let paths = fs::read_dir(conf.pkg_dir(name)).unwrap();
    for path in paths {
        let path = path.unwrap().file_name().into_string().unwrap();
        if path.ends_with(".pkg.tar.zst") {
            return Some(path);
        }
    }
    None
}

// aerc-0.16.0-1-x86_64.pkg.tar.zst
fn repo_add(conf: &Conf, to_build: Vec<&String>) {
    let db = Path::new(&conf.server_dir).join("pacage.db.tar.gz");
    for pkg in to_build {
        if let Some(package_file) = find_package(conf, &pkg) {
            // Move the package next to the db
            let moved_package_file = Path::new(&conf.server_dir).join(&package_file);
            std::fs::rename(conf.pkg_dir(pkg).join(&package_file), &moved_package_file).unwrap();
            let mut cmd = Command::new("repo-add");
            cmd.current_dir(&conf.server_dir)
                .args([&db, &moved_package_file]);
            let (status, _) = command(cmd).unwrap();
            if !status.success() {
                eprintln!("Failed to add {} to the db:", pkg);
            }
        } else {
            eprintln!("Failed to find {} package file", pkg);
        }
    }
}

fn main() {
    let args = Args::get();
    let conf = Conf::new(args.conf.as_deref());

    conf.print();

    fs::create_dir_all(&conf.server_dir).unwrap();
    println!("Downloading packages...");
    let to_build = download::download_all(&conf, args.force_rebuild);
    println!("Building packages...");
    build::build(&conf, &to_build);
    println!("Adding packages...");
    repo_add(&conf, to_build);
}
