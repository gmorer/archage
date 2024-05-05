use flate2::read::GzDecoder;
use log::{error, info};
use std::collections::HashSet;
use std::fs;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use tar::Archive;
use thiserror::Error;

use crate::cmd::command;
use crate::cmd::write_last_lines;
use crate::conf::Conf;
use crate::download::PkgBuild;

#[derive(Debug, Error)]
pub enum RepoError {
    #[error("Missing repo database")]
    NoRepo,
    #[error("System error: {0}")]
    Io(#[from] std::io::Error),
}

pub struct DbPackage {
    name: String,
    // https://gitlab.archlinux.org/pacman/pacman/-/blob/master/lib/libalpm/version.c
    version: String,
    arch: String,
    packager: String,
    build_date: u32,
    // TODO:
    // build time
}

impl DbPackage {
    pub fn new(data: impl BufRead) -> Option<Self> {
        enum Key {
            Name,
            Version,
            Arch,
            Packager,
            BuildDate,
        }
        let mut key = None;
        let mut name = None;
        let mut version = None;
        let mut arch = None;
        let mut packager = None;
        let mut build_date = None;
        for line in data.lines() {
            if let Ok(line) = line {
                if line.is_empty() {
                    key = None;
                    continue;
                }
                match key {
                    None => {
                        key = match line.as_str() {
                            "%NAME%" => Some(Key::Name),
                            "%ARCH%" => Some(Key::Arch),
                            "%VERSION%" => Some(Key::Version),
                            "%BUILDDATE%" => Some(Key::BuildDate),
                            "%PACKAGER%" => Some(Key::Packager),
                            _ => None,
                        }
                    }
                    Some(Key::Name) => {
                        name = Some(line);
                        key = None;
                    }
                    Some(Key::Version) => {
                        version = Some(line);
                        key = None;
                    }
                    Some(Key::Arch) => {
                        arch = Some(line);
                        key = None;
                    }
                    Some(Key::Packager) => {
                        packager = Some(line);
                        key = None;
                    }
                    Some(Key::BuildDate) => {
                        build_date = match line.parse::<u32>() {
                            Ok(ts) => Some(ts),
                            Err(e) => {
                                error!("Failed to parse timestamp({}): {}", line, e);
                                None
                            }
                        };
                        key = None;
                    }
                }
            }
            if name.is_some()
                && version.is_some()
                && arch.is_some()
                && packager.is_some()
                && build_date.is_some()
            {
                return Some(Self {
                    name: name.unwrap(),
                    version: version.unwrap(),
                    arch: arch.unwrap(),
                    packager: packager.unwrap(),
                    build_date: build_date.unwrap(),
                });
            }
        }
        error!(
            "Failed to parse package from db, missing fields, name: {:?}, version: {:?}, arch: {:?}, packager: {:?}, build_date: {:?}",
            name, version, arch, packager, build_date,
        );

        None
    }
    pub fn print(&self) {
        println!("{} {} ", self.name, self.version);
    }
}

fn find_package(conf: &Conf, name: &str) -> Option<String> {
    let prefix = format!("{}-", name);
    let dir = match fs::read_dir(conf.server_dir.join("repo")) {
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
        if path.ends_with(".pkg.tar.zst") && path.starts_with(&prefix) {
            return Some(path.to_string());
        }
    }
    None
}

pub fn add(conf: &Conf, to_build: HashSet<PkgBuild>) {
    if to_build.is_empty() {
        info!("Nothing to add");
        return;
    }
    let binding = conf.get_repo();
    let db = binding.as_os_str().to_string_lossy();
    for pkg in to_build {
        if let Some(package_file) = find_package(conf, &pkg.name) {
            // Move the package next to the db
            let tmp = Path::new(&conf.server_dir).join("repo").join(&package_file);
            let moved_package_file = tmp.as_os_str().to_string_lossy();
            match command(&["repo-add", &db, &moved_package_file], &conf.server_dir) {
                Ok((status, _)) if status.success() => {}
                Ok((_, out)) => {
                    error!("[{}] Failed to add the package to the db ->", pkg.name);
                    write_last_lines(&out, 5);
                }
                Err(e) => {
                    error!("[{}] Failed to add to the db: {}", pkg.name, e);
                }
            };
        } else {
            error!("[{}] Failed to find package file", pkg.name);
        }
    }
}

pub fn list(conf: &Conf) -> Result<Vec<DbPackage>, RepoError> {
    let mut pkgs = Vec::new();
    let tar_gz = File::open(conf.get_repo()).map_err(|_| RepoError::NoRepo)?;
    let tar = GzDecoder::new(tar_gz);
    let mut archive = Archive::new(tar);
    for entry in archive.entries()? {
        if let Ok(entry) = entry {
            if let Ok(path) = entry.path() {
                if path
                    .file_name()
                    .is_some_and(|name| name.to_str() == Some("desc"))
                {
                    DbPackage::new(BufReader::new(entry)).map(|p| pkgs.push(p));
                }
            }
        }
    }
    Ok(pkgs)
}
