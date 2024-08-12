use flate2::read::GzDecoder;
use log::{error, warn};
// use log::error;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{self, BufReader};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use tar::Archive;
use thiserror::Error;

use crate::cmd::{command, write_last_lines, NOENV};
use crate::conf::Conf;

use crate::format::{DbDesc, DbDescError, PkgInfo};
use crate::utils::file_lock::FileLock;
use crate::utils::version::Version;

// TODO: Look into pacman's "vercmp"

/*
===== pacage.db.tar.gz =====
├ ${pkgname1}-${pkgver1}/    # One directory per package - version
│ └ desc                     # One desc file per directory (format::DbDesc)
├ ${pkgname2}-${pkgver2}/
│ └ desc
├ ${pkgname3}-${pkgver3}/
│ └ desc
├ [...]
========
===== pacage.files.tar.gz =====
├ ${pkgname1}-${pkgver1}/    # One directory per package - version
│ ├ files                    # List of all the files (sort -u) (except hidden) in pkgfile
│ └ desc                     # Same desc file as db (format::DbDesc)
├ ${pkgname2}-${pkgver2}/
│ ├ files
│ └ desc
├ ${pkgname3}-${pkgver3}/
│ ├ files
│ └ desc
├ [...]
========
==== pacage.files.tar.gz/bash-5.2.002-2/files
%FILES%
etc/
etc/bash.bash_logout
etc/bash.bashrc
etc/skel/
etc/skel/.bash_logout
etc/skel/.bash_profile
etc/skel/.bashrc
usr/
usr/bin/
usr/bin/bash
usr/bin/bashbug
[...]
usr/share/man/man1/bashbug.1.gz
*/

#[derive(Debug, Error)]
pub enum RepoError {
    #[error("Missing repo database")]
    NoRepo,
    #[error("System error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Parsing error: {0}")]
    Parsing(#[from] DbDescError),
}

pub fn list(conf: &Conf) -> Result<Vec<DbDesc>, RepoError> {
    let mut pkgs = Vec::new();
    let tar_gz = File::open(conf.get_repo()).map_err(|_| RepoError::NoRepo)?;
    let tar = GzDecoder::new(tar_gz);
    let mut archive = Archive::new(tar);
    // TODO: check for duplicated pkgs (same pkg not the same version)
    for entry in archive.entries()? {
        if let Ok(entry) = entry {
            if let Ok(path) = entry.path() {
                if path
                    .file_name()
                    .is_some_and(|name| name.to_str() == Some("desc"))
                {
                    DbDesc::new(BufReader::new(entry)).map(|p| pkgs.push(p))?;
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
    /*
    In house imp:
    untar the package_file especially pkgfile
    parse .PKGINFO
    - LOCK DB -
    create db if dont exist
    check for already existing in the db.tar (especially newer version, and same one)
    check csize match / replace with actual size
    atomic replace db
    compute pgp sig if package_file.sig exist
    compute sha256sum
    remove any pkgname entry
    create new ${pkgname}-${pkgver}/desc entry
    generate pkg.files.tar.gz entry (echo "%FILES%" >"$files_path" && bsdtar --exclude='^.*' -tf "$pkgfile" | LC_ALL=C sort -u >>"$files_path" )
    - UNLOCK DB -
    */
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

fn parse_path_name(path: &Path) -> Result<(String /* pkgname */, Version), String> {
    let path = path.to_string_lossy();
    let second = path
        .rfind('-')
        .ok_or_else(|| format!("Missing '-' in database entry"))?;
    let first = path[..second]
        .rfind('-')
        .ok_or_else(|| format!("Missing second '-' in database entry"))?;
    if second + 1 >= path.len() {
        return Err("Package release missing".to_string());
    } else if first == 0 {
        return Err("Package name missing".to_string());
    }
    let name = path[..first].to_string();
    let version = Version::try_from(&path[(first + 1)..])
        .map_err(|e| format!("Failed to parse db entry version: {}", e))?;
    return Ok((name, version));
}

pub fn add_test(conf: &Conf, name: &str) -> Result<(), i32> {
    let Some(pkgfile) = find_package(conf, name) else {
        eprintln!("Could not find the package archive");
        return Err(2);
    };
    let mut tar_gz = File::open(&pkgfile).map_err(|e| {
        eprintln!("Failed to open the arhive: {}", e);
        9
    })?;
    let mut archive = Archive::new(GzDecoder::new(&tar_gz));
    let entries = archive.entries().map_err(|e| {
        eprintln!("Failed to parse the arhive: {}", e);
        9
    })?;
    let mut pkginfo = None;
    for entry in entries {
        let Ok(entry) = entry else {
            continue;
        };
        let Ok(path) = entry.path() else {
            continue;
        };
        if path == Path::new(".PKGINFO") {
            pkginfo = Some(BufReader::new(entry));
            break;
        }
    }
    let Some(pkginfo) = pkginfo else {
        eprintln!("Missing .PKGINFO in the pkg archive");
        return Err(3);
    };

    let pkginfo = PkgInfo::new(pkginfo).map_err(|e| {
        eprintln!("Failed to parse .PKGINFO: {}", e);
        5
    })?;

    let repo_lock = FileLock::new(conf.get_repo().with_extension("lock")).map_err(|e| {
        eprintln!("Failed to acquire db lock: {}", e);
        3
    })?;
    println!("Locked");
    let repo_path = conf.get_repo();
    let mut tmp_repo_path = repo_path.clone();
    tmp_repo_path.set_extension("tmp");
    if repo_path.exists() {
        fs::copy(&repo_path, &tmp_repo_path).map_err(|e| {
            eprintln!("Failed to copy the db: {}", e);
            4
        })?;
    }
    let repo = File::create(&tmp_repo_path).map_err(|e| {
        eprintln!("Failed to open the db: {}", e);
        5
    })?;
    let tar = GzDecoder::new(repo);
    let mut archive = Archive::new(tar);
    let entries = archive.entries().map_err(|e| {
        eprintln!("failed to read db: {}", e);
        6
    })?;
    let mut to_remove = None;
    for entry in entries {
        if let Ok(entry) = entry {
            if let Ok(path) = entry.path() {
                let (ename, eversion) = match parse_path_name(&path) {
                    Ok(a) => a,
                    Err(e) => {
                        warn!(
                            "Invalid entry in the db '{}': {}",
                            path.to_string_lossy(),
                            e
                        );
                        continue;
                    }
                };
                if ename != name {
                    continue;
                }
                if eversion > pkginfo.version {
                    // warning "$(gettext "A newer version for '%s' is already present in database")" "$pkgname"
                    // if (( PREVENT_DOWNGRADE )); then
                    // 	return 0
                    unimplemented!();
                }
                let edesc = File::open(path.join("desc")).map_err(|e| {
                    eprintln!("Failed to open old entry desc: {}", e);
                    7
                })?;
                let edesc = DbDesc::new(BufReader::new(edesc)).map_err(|e| {
                    eprintln!("Failed to parse old entry desc: {}", e);
                    7
                })?;
                to_remove = Some(edesc.filename);
            }
        }
    }
    if PathBuf::from(format!("{}.sig", pkgfile)).exists() {
        // compute base64'd PGP signature
        unimplemented!()
    }
    let mut hasher = Sha256::new();
    let csize = io::copy(&mut tar_gz, &mut hasher).map_err(|e| {
        eprintln!("Failed to read pkg archive to get the hash: {}", e);
        10
    })?;
    let sha256 = base16ct::lower::encode_string(&hasher.finalize());
    let desc = pkginfo.to_desc(pkgfile, csize, sha256, None);
    unimplemented!();
    // TODO: new entry to the archive
    // remove any pkgname entry and from files as well
    // create new ${pkgname}-${pkgver}/desc entry
    // generate pkg.files.tar.gz entry (echo "%FILES%" >"$files_path" && bsdtar --exclude='^.*' -tf "$pkgfile" | LC_ALL=C sort -u >>"$files_path" )
    // atomic replace db
    // Ensure repo_lock stay alive until the end
    println!("Unlocked");
    drop(repo_lock);
    Ok(())
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn parse_path_name() {
        let v = Version::new;
        for (test, expected) in [
            ("bash-5.43-2", Ok(("bash", v("5.43", Some("2"), None)))),
            (
                "bash-ex-5.43-2",
                Ok(("bash-ex", v("5.43", Some("2"), None))),
            ),
            ("vi-1:070224-6", Ok(("vi", v("070224", Some("6"), Some(1))))),
            ("bash-5.42", Err("Missing second '-' in database entry")),
            ("bash-5.42-", Err("Package release missing")),
            ("-5.42-42", Err("Package name missing")),
        ] {
            let res = super::parse_path_name(Path::new(test));
            let expected = expected
                .map(|(name, version)| (name.to_string(), version))
                .map_err(|e| e.to_string());
            assert_eq!(res, expected, "Testing {}", test);
        }
    }
}
