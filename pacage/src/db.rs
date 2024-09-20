use flate2::read::GzDecoder;
use flate2::GzBuilder;
use log::{error, warn};
use ruzstd::StreamingDecoder;
// use log::error;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::env;
use std::fs::{self, File};
use std::io::{self, BufReader, Write};
use std::path::{Path, PathBuf};
use tar::Archive;
use thiserror::Error;

use crate::cmd::{command, write_last_lines, NOENV};
use crate::conf::Conf;

use crate::format::{DbDesc, DbDescError, PkgInfo};
use crate::utils::file_lock::FileLock;
use crate::utils::version::Version;

const TMP_DB: &str = "pacage.db.tmp";
const TMP_FILES: &str = "pacage.files.tmp";

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
    let tar_gz = File::open(conf.get_repo_db()).map_err(|_| RepoError::NoRepo)?;
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

/// Generate the content of the "files" file
fn generate_files_file(files: BTreeSet<String>) -> Vec<u8> {
    let mut res = Vec::from(b"%FILES%\n");
    for file in files {
        res.push(b'\n');
        res.copy_from_slice(file.as_bytes());
    }
    res
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
            Ok(e) => {
                if !(e
                    .file_name()
                    .to_string_lossy()
                    .to_string()
                    .starts_with(&prefix))
                {
                    continue;
                }
                e.path()
            }
            Err(e) => {
                error!("[{}] Fail to check for files in pkg dir: {}", name, e);
                return None;
            }
        };
        let path = path.to_string_lossy().to_string();
        if path.ends_with(".pkg.tar.zst") {
            return Some(path.to_string());
        }
    }
    error!("[{}] Didnt find any package", name);
    None
}

fn read_package(
    pkgfile: &str,
) -> Result<
    (
        PkgInfo,
        u64,              /* csize  */
        String,           /* sha256 */
        BTreeSet<String>, /* FILES  */
    ),
    i32,
> {
    let mut tar_zst = File::open(pkgfile).map_err(|e| {
        eprintln!("Failed to open the archive: {}", e);
        9
    })?;
    let mut archive = Archive::new(StreamingDecoder::new(&tar_zst).map_err(|e| {
        eprintln!("Decompression error: {}", e);
        19
    })?);
    let entries = archive.entries().map_err(|e| {
        eprintln!("Failed to parse the archive: {}", e);
        9
    })?;
    let mut pkginfo = None;
    let mut files = BTreeSet::new();
    for entry in entries {
        if let Err(e) = &entry {
            eprintln!("error: {}", e);
        }
        let Ok(entry) = entry else {
            continue;
        };
        let Ok(path) = entry.path() else {
            continue;
        };
        let Some(path) = path.to_str() else {
            continue;
        };
        if path == ".PKGINFO" {
            pkginfo = Some(PkgInfo::new(BufReader::new(entry)).map_err(|e| {
                eprintln!("Failed to parse .PKGINFO: {}", e);
                5
            })?);
        } else if !path.starts_with(".") && entry.header().entry_type() == tar::EntryType::Regular {
            files.insert(path.to_string());
        }
    }
    let Some(pkginfo) = pkginfo else {
        eprintln!("Missing .PKGINFO in the pkg archive");
        return Err(3);
    };

    let mut hasher = Sha256::new();
    let csize = io::copy(&mut tar_zst, &mut hasher).map_err(|e| {
        eprintln!("Failed to read pkg archive to get the hash: {}", e);
        10
    })?;
    let sha256 = base16ct::lower::encode_string(&hasher.finalize());
    Ok((pkginfo, csize, sha256, files))
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
        let binding = conf.get_repo_db();
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

/// Copy the db to a new location omiting the pkginfo.pkgname pkgs
fn copy_old_db<T>(
    out_tar: &mut tar::Builder<T>,
    repo_path: &Path,
    pkginfo: &PkgInfo,
    to_remove: &mut Vec<String>,
) -> Result<(), i32>
where
    T: Write,
{
    let repo = File::open(&repo_path).map_err(|e| {
        eprintln!("Failed to open the db: {}", e);
        5
    })?;
    let mut archive = Archive::new(GzDecoder::new(&repo));
    let entries = archive.entries().map_err(|e| {
        eprintln!("failed to read db: {}", e);
        6
    })?;
    for entry in entries {
        if let Ok(mut entry) = entry {
            if let Ok(path) = entry.path() {
                if path
                    .file_name()
                    .is_some_and(|name| name.to_str() == Some("desc"))
                {
                    let Some(parent) = path.parent() else {
                        eprintln!("Missing parent for {}", path.to_string_lossy());
                        continue;
                    };

                    let (ename, eversion) = match parse_path_name(&parent) {
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
                    if ename != pkginfo.pkgname {
                        let epath = PathBuf::from(&*path);
                        let mut header = entry.header().clone();
                        let reader = BufReader::new(entry);
                        out_tar
                            .append_data(&mut header, epath, reader)
                            .map_err(|e| {
                                eprintln!("failed to copy db entry to output db: {}", e);
                                9
                            })?;
                        continue;
                    }
                    if eversion > pkginfo.version {
                        // warning "$(gettext "A newer version for '%s' is already present in database")" "$pkgname"
                        // if (( PREVENT_DOWNGRADE )); then
                        // 	return 0
                        unimplemented!();
                    }
                    let edesc = DbDesc::new(BufReader::new(&mut entry)).map_err(|e| {
                        eprintln!("Failed to parse old entry desc: {}", e);
                        7
                    })?;
                    to_remove.push(edesc.filename);
                }
            }
        }
    }
    Ok(())
}

/// Copy the files db to a new location omiting the pkginfo.pkgname pkgs
fn copy_old_files<T>(
    out_tar: &mut tar::Builder<T>,
    files_path: &Path,
    pkgname: &String,
) -> Result<(), i32>
where
    T: Write,
{
    let repo = File::open(&files_path).map_err(|e| {
        eprintln!("Failed to open the db: {}", e);
        5
    })?;
    let mut archive = Archive::new(GzDecoder::new(&repo));
    let entries = archive.entries().map_err(|e| {
        eprintln!("failed to read db: {}", e);
        6
    })?;
    for entry in entries {
        if let Ok(entry) = entry {
            if let Ok(path) = entry.path() {
                let Some(parent) = path.parent() else {
                    continue;
                };

                let (ename, _) = match parse_path_name(&parent) {
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
                if &ename != pkgname {
                    let epath = PathBuf::from(&*path);
                    let mut header = entry.header().clone();
                    let reader = BufReader::new(entry);
                    out_tar
                        .append_data(&mut header, epath, reader)
                        .map_err(|e| {
                            eprintln!("failed to copy db entry to output db: {}", e);
                            9
                        })?;
                    continue;
                }
            }
        }
    }
    Ok(())
}

// TODO: handle multiples packages as well
// TODO: Error type
// TODO: Testing
pub fn add_test(conf: &Conf, name: &str) -> Result<(), i32> {
    let Some(pkgfile) = find_package(conf, name) else {
        eprintln!("Could not find the package archive");
        return Err(2);
    };
    let (pkginfo, csize, sha256, files) = read_package(&pkgfile)?;
    let mut to_remove = vec![];

    let repo_lock = FileLock::new(conf.get_repo_db().with_extension("lock")).map_err(|e| {
        eprintln!("Failed to acquire db lock: {}", e);
        3
    })?;

    // Create 2 new temporary dbs
    let tar_new_db_path = env::temp_dir().join(TMP_DB);
    let tar_new_files_path = env::temp_dir().join(TMP_FILES);
    let mut tar_new_db = tar::Builder::new(GzBuilder::new().write(
        File::create(&tar_new_db_path).map_err(|e| {
            eprintln!("Failed to create tmp out db: {}", e);
            15
        })?,
        flate2::Compression::default(),
    ));
    let mut tar_new_files = tar::Builder::new(GzBuilder::new().write(
        File::create(&tar_new_files_path).map_err(|e| {
            eprintln!("Failed to create tmp out files: {}", e);
            15
        })?,
        flate2::Compression::default(),
    ));

    // Copy old relevant(everything except our package) entries into the new db
    let repo_path = conf.get_repo_db();
    let files_path = conf.get_repo_files_db();
    if repo_path.exists() {
        copy_old_db(&mut tar_new_db, &repo_path, &pkginfo, &mut to_remove)?;
    }
    if files_path.exists() {
        copy_old_files(&mut tar_new_files, &files_path, &pkginfo.pkgname)?;
    }

    // Write desc in both db and files
    let pgpsig = if PathBuf::from(format!("{}.sig", pkgfile)).exists() {
        // compute base64'd PGP signature
        unimplemented!()
    } else {
        None
    };
    let version = pkginfo.version.to_string();
    let desc_path = format!("{}-{}/desc", pkginfo.pkgname, &version);
    let desc = pkginfo.to_desc(pkgfile, csize, sha256, pgpsig);
    let mut desc_raw = vec![];
    desc.write(&mut desc_raw).map_err(|e| {
        eprintln!("Fail to create new desc file: {}", e);
        8
    })?;
    let mut desc_header = tar::Header::new_gnu();
    desc_header.set_size(desc_raw.len() as u64);
    tar_new_db
        .append_data(&mut desc_header, &desc_path, desc_raw.as_slice())
        .map_err(|e| {
            eprintln!("failed to copy db entry to output db: {}", e);
            9
        })?;
    tar_new_files
        .append_data(&mut desc_header, desc_path, desc_raw.as_slice())
        .map_err(|e| {
            eprintln!("failed to copy db entry to output files: {}", e);
            9
        })?;

    // Write files in files
    let files_path = format!("{}-{}/files", pkginfo.pkgname, version);
    let files_content = generate_files_file(files);
    let mut files_header = tar::Header::new_gnu();
    files_header.set_size(files_content.len() as u64);
    tar_new_files
        .append_data(&mut files_header, &files_path, files_content.as_slice())
        .map_err(|e| {
            eprintln!("failed to copy db entry to output files: {}", e);
            9
        })?;

    // Write both to disc
    let db_out = tar_new_db
        .into_inner()
        .map_err(|e| {
            eprintln!("Failed to write out db archive: {}", e);
            10
        })?
        .finish()
        .map_err(|e| {
            eprintln!("Failed to write out db gz: {}", e);
            11
        })?;
    db_out.sync_all().map_err(|e| {
        eprintln!("Failed to sync out db gz: {}", e);
        11
    })?;
    drop(db_out);
    let files_out = tar_new_files
        .into_inner()
        .map_err(|e| {
            eprintln!("Failed to write out files archive: {}", e);
            10
        })?
        .finish()
        .map_err(|e| {
            eprintln!("Failed to write out files gz: {}", e);
            11
        })?;
    files_out.sync_all().map_err(|e| {
        eprintln!("Failed to sync out files gz: {}", e);
        11
    })?;
    drop(files_out);

    // Atomic update of both dbs
    // TODO: renameing like that does not work because /tmp is not on the same device :O
    // This will not work if the new name is on a different mount point.
    println!("renaming from {:?} to {:?}", tar_new_db_path, repo_path);
    fs::rename(tar_new_db_path, repo_path).map_err(|e| {
        eprintln!("Failed to overwrite old db with new one: {}", e);
        11
    })?;
    fs::rename(tar_new_files_path, files_path).map_err(|e| {
        eprintln!("Failed to overwrite old files with new one: {}", e);
        11
    })?;

    // Remove old pkg archives not present in the db any more
    let a = conf.server_dir.clone();
    for file in to_remove {
        if let Err(e) = fs::remove_file(a.join(file)) {
            eprintln!("Failed to remove old package file: {}", e);
        }
    }
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

    #[test]
    fn add_items_to_db() {
        let a = Conf::_test_builder().server_dir("../tmp".into()).call();
        assert!(matches!(list(&a).unwrap_err(), RepoError::NoRepo));
        add_test(&a, "testing_fake_pkg").unwrap();
        let pkg_list = list(&a).unwrap();
        assert_eq!(pkg_list.len(), 0);
        let entry = pkg_list.get(0).unwrap();
        assert_eq!(entry.name, "testing_fake_pkg", "Checking entry name");
        assert_eq!(entry.version, "2-2024.04.07");
    }
}
