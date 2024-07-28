use flate2::read::GzDecoder;
use log::error;
use std::fs::{self, File};
use std::io::BufReader;
use std::path::Path;
use tar::Archive;
use thiserror::Error;

use crate::cmd::{command, write_last_lines, NOENV};
use crate::conf::Conf;

use crate::format::DbDesc;

#[derive(Debug, Error)]
pub enum RepoError {
    #[error("Missing repo database")]
    NoRepo,
    #[error("System error: {0}")]
    Io(#[from] std::io::Error),
}

pub fn list(conf: &Conf) -> Result<Vec<DbDesc>, RepoError> {
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
                    DbDesc::new(BufReader::new(entry)).map(|p| pkgs.push(p));
                }
            }
        }
    }
    Ok(pkgs)
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

pub fn add(conf: &Conf, name: &str) -> Result<(), String> {
    if let Some(package_file) = find_package(conf, name) {
        let binding = conf.get_repo();
        let db = binding.as_os_str().to_string_lossy();
        // Move the package next to the db
        let tmp = Path::new(&conf.server_dir).join("repo").join(&package_file);
        let moved_package_file = tmp.as_os_str().to_string_lossy();
        match command(
            &["repo-add", &db, &moved_package_file],
            &conf.server_dir,
            NOENV,
        ) {
            Ok((status, _, _)) if status.success() => {}
            Ok((_, out, _)) => {
                error!("[{}] Failed to add the package to the db ->", name);
                write_last_lines(&out, 5);
            }
            Err(e) => {
                error!("[{}] Failed to add to the db: {}", name, e);
            }
        };
    } else {
        error!("[{}] Failed to find package file", name);
    }
    Ok(())
}
