use crate::conf::Conf;
use log::{error, info};
use std::ffi::OsStr;
use std::fs::{read_dir, File};
use std::io::{self, ErrorKind};
use std::path::PathBuf;
use thiserror::Error;

use crate::cmd::{command, out_to_file, write_last_lines, ExecError, NOENV};
use crate::format::SrcInfo;

// Create a patch: diff  -Naru  ex-070224{,.patched} > file.patch
// Apply patch: patch -p1 < file.patch

#[derive(Debug, Error)]
pub enum PatchError {
    #[error("IO error: {0}")]
    IOError(#[from] io::Error),
    #[error("System error: {0}")]
    ExecError(#[from] ExecError),
    #[error("Failed to apply patch")]
    PatchApply(),
}

pub fn find_src(conf: &Conf, pkg: &SrcInfo) -> Option<PathBuf> {
    let src_dir = conf.pkg_src(&pkg.name).join("src");
    if src_dir.join(&pkg.name).exists() {
        return Some(src_dir.join(&pkg.name));
    }
    let name_version = src_dir.join(format!("{}-{}", pkg.name, pkg.pkgver));
    if name_version.exists() {
        return Some(name_version);
    }
    // Looks for root dir with Makefile
    let dir = match read_dir(&src_dir) {
        Ok(d) => d,
        Err(e) => {
            error!("[{}] Failed to open src dir: {}", pkg.name, e);
            return None;
        }
    };
    for file in dir {
        if let Ok(file) = file {
            let dir_name = match file.file_type() {
                Ok(typ) if typ.is_dir() => file.path(),
                _ => continue,
            };
            for test in [
                "Makefile",
                "CMakeLists.txt",
                "meson.build",
                "BUILD",
                "BUILD.bazel",
            ] {
                if dir_name.join(test).exists() {
                    return Some(dir_name);
                }
            }
        }
    }

    return None;
}

pub fn get_patches(conf: &Conf, name: &str) -> Result<Option<Vec<String>>, PatchError> {
    let patch_dir_path = conf.conf_dir.join("patchs").join(name);
    let patch_dir = match read_dir(&patch_dir_path) {
        Ok(d) => d,
        Err(e) => {
            return if e.kind() != ErrorKind::NotFound {
                error!("[{}] Fail to open patch dir: {}", name, e);
                Err(e)?
            } else {
                Ok(None)
            }
        }
    };
    let mut patches = Vec::new();
    for file in patch_dir {
        let file = match file {
            Ok(file) => file,
            Err(e) => {
                error!("[{}] Could not get file metadata in patch dir: {}", name, e);
                continue;
            }
        };
        match file.file_type() {
            Ok(typ) if !typ.is_file() => continue,
            Err(e) => {
                error!("[{}] Failed to check file type: {}", name, e);
                continue;
            }
            _ => {}
        };
        if file.path().extension() == Some(OsStr::new("patch")) {
            patches.push(file.path().to_string_lossy().to_string());
        }
    }
    patches.sort();
    Ok(Some(patches))
}

pub fn patch_dir(
    conf: &Conf,
    dir: &PathBuf,
    name: &str,
    patches: Vec<String>,
) -> Result<(), PatchError> {
    for patch in patches {
        info!("[{}] applying {}...", name, patch);
        let (status, out, _) = command(
            &["bash", "-c", &format!("patch -p1 < {}", &patch)],
            &dir,
            NOENV,
        )?;
        if !status.success() {
            error!("[{}] Failed to apply patch {:?}", name, patch,);
            write_last_lines(&out, 10);
            match out_to_file(&conf.build_log_dir, name, "patch", &out, false) {
                Ok(Some(file)) => info!("Full failed patch writed to {}", file),
                Ok(None) => {}
                Err(e) => error!("Failed to write patch output to logs: {}", e),
            }
            Err(PatchError::PatchApply())?
        } else {
            info!("[{}] Successfully applied {:?}", name, patch)
        }
    }
    Ok(())
}

// TODO: Real lock file (doesm this exist?)
pub fn patch(conf: &Conf, pkg: &SrcInfo) -> Result<Option<()>, PatchError> {
    let patch_marker = conf.pkg_src(&pkg.name).join(".pacage_patched");
    if patch_marker.exists() {
        return Ok(None);
    }
    let Some(patches) = get_patches(conf, &pkg.name)? else {
        return Ok(None);
    };
    if patches.is_empty() {
        return Ok(None);
    }
    let pkg_src = match find_src(conf, pkg) {
        Some(src) => src,
        None => {
            error!("[{}] Fail to find src dir to apply patches", pkg.name);
            Err(std::io::Error::new(
                ErrorKind::NotFound,
                "Could not find source dir",
            ))?
        }
    };
    info!("[{}] found src dir: {}", pkg.name, pkg_src.display());
    patch_dir(conf, &pkg_src, &pkg.name, patches)?;
    File::create(patch_marker)?;
    Ok(Some(()))
}
