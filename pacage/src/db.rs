use flate2::read::GzDecoder;
use flate2::GzBuilder;
use log::{error, warn};
use nix::NixPath;
use ruzstd::StreamingDecoder;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{self, BufReader, Write};
use std::path::{Path, PathBuf};
use tar::Archive;
use thiserror::Error;

use crate::conf::Conf;

use crate::format::{DbDesc, DbDescError, PkgInfo, SrcInfo};
use crate::utils::file_lock::DirLock;
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

#[derive(Debug, Error)]
pub enum AddError {
    #[error("Failed to parse all the packages")]
    Nothing,
    #[error("Failed to lock database")]
    DbLockError,
    #[error("System error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Parsing error: {0}")]
    Parsing(String),
    #[error("Encoding error: {0}")]
    Encoding(String),
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
        res.extend_from_slice(file.as_bytes());
    }
    res
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
    AddError,
> {
    let mut tar_zst = File::open(pkgfile)
        .inspect_err(|e| error!("Failed to open the archive({}): {}", pkgfile, e))?;
    let mut archive = Archive::new(
        StreamingDecoder::new(&tar_zst)
            .map_err(|e| AddError::Encoding(format!("Zstd error: {}", e)))?,
    );
    let entries = archive
        .entries()
        .map_err(|e| AddError::Encoding(format!("Archive listing error: {}", e)))?;
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
            pkginfo = Some(
                PkgInfo::new(BufReader::new(entry))
                    .map_err(|e| AddError::Parsing(format!("Fail to parse .PKGINFO: {}", e)))?,
            );
        } else if !path.starts_with(".") && entry.header().entry_type() == tar::EntryType::Regular {
            files.insert(path.to_string());
        }
    }
    let Some(pkginfo) = pkginfo else {
        Err(AddError::Encoding(
            "Missing .PKGINFO in the pkg archive".to_string(),
        ))?
    };

    let mut hasher = Sha256::new();
    let csize = io::copy(&mut tar_zst, &mut hasher)
        .inspect_err(|e| error!("Failed to read pkg archive to get the hash: {}", e))?;
    let sha256 = base16ct::lower::encode_string(&hasher.finalize());
    Ok((pkginfo, csize, sha256, files))
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
    adding: &[AddEntry],
    to_remove: &mut Vec<String>,
) -> Result<(), AddError>
where
    T: Write,
{
    let repo = File::open(&repo_path).inspect_err(|e| error!("Failed to open the db: {}", e))?;
    let mut archive = Archive::new(GzDecoder::new(&repo));
    let dbentries = archive
        .entries()
        .inspect_err(|e| error!("failed to read db: {}", e))?;
    for dbentry in dbentries {
        if let Ok(mut entry) = dbentry {
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
                    match adding
                        .iter()
                        .find(|(_, pkginfo, _, _, _)| pkginfo.pkgname == ename)
                    {
                        Some((_, pkginfo, _, _, _)) => {
                            if eversion > pkginfo.version {
                                // warning "$(gettext "A newer version for '%s' is already present in database")" "$pkgname"
                                // if (( PREVENT_DOWNGRADE )); then
                                // 	return 0
                                warn!("[{}] A newer version({}) is already present in database, trying to install: {}", ename, eversion, pkginfo.version);
                                unimplemented!();
                            } else if eversion < pkginfo.version {
                                let edesc =
                                    DbDesc::new(BufReader::new(&mut entry)).map_err(|e| {
                                        AddError::Parsing(format!(
                                            "Failed to parse old entry desc: {}",
                                            e
                                        ))
                                    })?;
                                to_remove.push(edesc.filename);
                            }
                        }
                        None => {
                            let epath = PathBuf::from(&*path);
                            let mut header = entry.header().clone();
                            let reader = BufReader::new(entry);
                            out_tar
                                .append_data(&mut header, epath, reader)
                                .inspect_err(|e| {
                                    error!("failed to copy db entry to output db: {}", e)
                                })?;
                        }
                    }
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
    // pkgname: &String,
    adding: &[AddEntry],
) -> Result<(), io::Error>
where
    T: Write,
{
    let repo =
        File::open(&files_path).inspect_err(|e| error!("Failed to open the files db: {}", e))?;
    let mut archive = Archive::new(GzDecoder::new(&repo));
    let entries = archive
        .entries()
        .inspect_err(|e| error!("failed to read the files db: {}", e))?;
    for entry in entries {
        if let Ok(entry) = entry {
            if let Ok(path) = entry.path() {
                let Some(parent) = path.parent() else {
                    continue;
                };
                if parent.is_empty() {
                    continue;
                }

                let (ename, _) = match parse_path_name(&parent) {
                    Ok(a) => a,
                    Err(e) => {
                        warn!(
                            "Invalid entry in the files db '{}': {}",
                            path.to_string_lossy(),
                            e
                        );
                        continue;
                    }
                };
                if adding
                    .iter()
                    .find(|(_, pkginfo, _, _, _)| pkginfo.pkgname == ename)
                    .is_none()
                {
                    let epath = PathBuf::from(&*path);
                    let mut header = entry.header().clone();
                    let reader = BufReader::new(entry);
                    out_tar
                        .append_data(&mut header, epath, reader)
                        .inspect_err(|e| {
                            error!("failed to copy files db entry to output files db: {}", e)
                        })?;
                }
            }
        }
    }
    Ok(())
}

type AddEntry = (
    String, /* pkgfile*/
    PkgInfo,
    u64,    /* csize */
    String, /* sha256 */
    BTreeSet<String>,
);

/// Basicly repo-add reimplementation
pub fn add(conf: &Conf, pkgs: &[SrcInfo]) -> Result<(), AddError> {
    // TODO: take in multiple packages
    let mut pkgfiles: Vec<AddEntry> = Vec::new();
    let mut to_remove = vec![];

    for pkg in pkgs {
        let pkgfile = conf
            .server_dir
            .join("repo")
            .join(format!(
                "{}-{}-{}.pkg.tar.zst",
                pkg.name,
                pkg.get_version(),
                pkg.arch
            ))
            .to_string_lossy()
            .to_string();
        let (pkginfo, csize, sha256, files) = match read_package(&pkgfile) {
            Ok(v) => v,
            Err(e) => {
                error!("[{}] {}", pkg.name, e);
                continue;
            }
        };
        if &pkginfo.version != pkg.get_version() {
            error!(
                "[{}] Version mismatch from created package({}) to request package({})",
                pkg.name,
                pkginfo.version,
                pkg.get_version()
            );
            continue;
        }
        pkgfiles.push((pkgfile, pkginfo, csize, sha256, files));
    }
    if pkgfiles.len() == 0 {
        return Err(AddError::Nothing);
    }

    let repo_lock = DirLock::new(conf.get_repo_db().with_extension("lock")).map_err(|e| {
        error!("Failed to lock db: {}", e);
        AddError::DbLockError
    })?;

    // Create 2 new temporary dbs
    let tar_new_db_path = repo_lock.path().join(TMP_DB);
    let tar_new_files_path = repo_lock.path().join(TMP_FILES);
    let mut tar_new_db = tar::Builder::new(
        GzBuilder::new().write(
            File::create(&tar_new_db_path)
                .inspect_err(|e| error!("Failed to create tmp out db: {}", e))?,
            flate2::Compression::default(),
        ),
    );
    let mut tar_new_files = tar::Builder::new(
        GzBuilder::new().write(
            File::create(&tar_new_files_path)
                .inspect_err(|e| error!("Failed to create tmp out files: {}", e))?,
            flate2::Compression::default(),
        ),
    );

    // Copy old relevant(everything except our package) entries into the new db
    let repo_path = conf.get_repo_db();
    let files_path = conf.get_repo_files_db();
    if repo_path.exists() {
        copy_old_db(&mut tar_new_db, &repo_path, &pkgfiles, &mut to_remove)?;
    }
    if files_path.exists() {
        copy_old_files(&mut tar_new_files, &files_path, &pkgfiles)?;
    }

    for (pkgfile, pkginfo, csize, sha256, files) in pkgfiles {
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
        desc.write(&mut desc_raw)
            .inspect_err(|e| error!("Fail to create new desc file: {}", e))?;
        let mut desc_header = tar::Header::new_gnu();
        desc_header.set_size(desc_raw.len() as u64);
        tar_new_db
            .append_data(&mut desc_header, &desc_path, desc_raw.as_slice())
            .inspect_err(|e| error!("failed to copy db entry to output db: {}", e))?;
        tar_new_files
            .append_data(&mut desc_header, desc_path, desc_raw.as_slice())
            .inspect_err(|e| error!("failed to copy db entry to output files: {}", e))?;

        // Write files in files
        let new_files_path = format!("{}-{}/files", pkginfo.pkgname, version);
        let files_content = generate_files_file(files);
        let mut files_header = tar::Header::new_gnu();
        files_header.set_size(files_content.len() as u64);
        tar_new_files
            .append_data(&mut files_header, &new_files_path, files_content.as_slice())
            .inspect_err(|e| error!("failed to copy db entry to output files: {}", e))?;
    }
    // Write both to disc
    let db_out = tar_new_db
        .into_inner()
        .inspect_err(|e| error!("Failed to write out db archive: {}", e))?
        .finish()
        .inspect_err(|e| error!("Failed to write out db gz: {}", e))?;
    db_out.sync_all().inspect_err(|e| {
        error!("Failed to sync out db gz: {}", e);
    })?;
    drop(db_out);
    let files_out = tar_new_files
        .into_inner()
        .inspect_err(|e| error!("Failed to write out files archive: {}", e))?
        .finish()
        .inspect_err(|e| error!("Failed to write out files gz: {}", e))?;
    files_out
        .sync_all()
        .inspect_err(|e| error!("Failed to sync out files gz: {}", e))?;
    drop(files_out);

    // Atomic update of both dbs
    fs::rename(tar_new_db_path, repo_path)
        .inspect_err(|e| error!("Failed to overwrite old db with new one: {}", e))?;
    fs::rename(tar_new_files_path, files_path)
        .inspect_err(|e| error!("Failed to overwrite old files with new one: {}", e))?;

    // Remove old pkg archives not present in the db any more
    let a = conf.server_dir.join("repo");
    for file in to_remove {
        if let Err(e) = fs::remove_file(a.join(&file)) {
            error!("Failed to remove old package file({}): {}", file, e);
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
        let pkginfo1 = SrcInfo::new(&a, "fake_pkg1").unwrap();
        add(&a, &[pkginfo1]).unwrap();
        let pkg_list = list(&a).unwrap();
        assert_eq!(pkg_list.len(), 1);
        let entry = pkg_list.get(0).unwrap();
        assert_eq!(entry.name, "fake_pkg1", "Checking entry name");
        assert_eq!(entry.version, "2024.04.07-2");
        let pkginfo2 = SrcInfo::new(&a, "fake_pkg2").unwrap();
        add(&a, &[pkginfo2]).unwrap();
        let pkg_list = list(&a).unwrap();
        assert_eq!(pkg_list.len(), 2);
    }
    #[test]
    fn add_2_items_to_db() {
        let a = Conf::_test_builder().server_dir("../tmp".into()).call();
        assert!(matches!(list(&a).unwrap_err(), RepoError::NoRepo));
        let pkginfo1 = SrcInfo::new(&a, "fake_pkg1").unwrap();
        let pkginfo2 = SrcInfo::new(&a, "fake_pkg2").unwrap();
        add(&a, &[pkginfo1, pkginfo2]).unwrap();
        let pkg_list = list(&a).unwrap();
        assert_eq!(pkg_list.len(), 2);
    }
}
