/*
bash-5.2.026-2-x86_64.pkg.tar.zst:
==== .PKGINFO ====
# Generated by makepkg 6.1.0
# using fakeroot version 1.35
pkgname = bash
pkgbase = bash
xdata = pkgtype=pkg
pkgver = 5.2.026-2
pkgdesc = The GNU Bourne Again shell
url = https://www.gnu.org/software/bash/bash.html
builddate = 1718499903
packager = tet <gmorer@pm.me>
size = 9441927
arch = x86_64
license = GPL-3.0-or-later
provides = sh
backup = etc/bash.bashrc
backup = etc/bash.bash_logout
backup = etc/skel/.bashrc
backup = etc/skel/.bash_profile
backup = etc/skel/.bash_logout
depend = readline
depend = libreadline.so=8-64
depend = glibc
depend = ncurses
optdepend = bash-completion: for tab completion
========
*/

use std::io::BufRead;

use thiserror::Error;

use crate::utils::version::Version;

use super::DbDesc;

#[derive(Debug, Error)]
pub enum PkgInfoError {
    #[error("Invalid data: {0}")]
    InvalidData(String),
}

// Look like we only actually need:  ensure $pkgname and $pkgver variables were found
pub struct PkgInfo {
    pub pkgname: String,
    // the pkgver there is actualy [${epoch}:]${pkgver}[-${pkgrel}] from .SRCINFO
    pkgver: String,
    pub version: Version,
    pkgbase: Option<String>,
    pkgdesc: Option<String>,
    size: Option<u32>,
    url: Option<String>,
    arch: Option<String>,
    builddate: Option<u32>,
    packager: Option<String>,
    groups: Vec<String>,
    license: Vec<String>,
    replaces: Vec<String>,
    depends: Vec<String>,
    conflicts: Vec<String>,
    provides: Vec<String>,
    optdepends: Vec<String>,
    makedepends: Vec<String>,
    checkdepends: Vec<String>,
}

impl PkgInfo {
    pub fn new(data: impl BufRead) -> Result<Self, PkgInfoError> {
        let mut pkgname = None;
        let mut pkgbase = None;
        let mut pkgver = None;
        let mut pkgdesc = None;
        let mut size = None;
        let mut url = None;
        let mut arch = None;
        let mut builddate = None;
        let mut packager = None;
        let mut groups = vec![];
        let mut license = vec![];
        let mut replaces = vec![];
        let mut depends = vec![];
        let mut conflicts = vec![];
        let mut provides = vec![];
        let mut optdepends = vec![];
        let mut makedepends = vec![];
        let mut checkdepends = vec![];
        for line in data.lines() {
            let Ok(line) = line else { continue };
            if let Some(n) = line.find('=') {
                if n == line.len() {
                    continue;
                }
                let key = line[..n].trim();
                let v = line[(n + 1)..].trim();
                match key {
                    "pkgname" => pkgname = Some(v.to_string()),
                    "pkgver" => pkgver = Some(v.to_string()),
                    "pkgbase" => pkgbase = Some(v.to_string()),
                    "pkgdesc" => pkgdesc = Some(v.to_string()),
                    "url" => url = Some(v.to_string()),
                    "arch" => arch = Some(v.to_string()),
                    "packager" => packager = Some(v.to_string()),
                    "groups" => groups.push(v.to_string()),
                    "license" => license.push(v.to_string()),
                    "replaces" => replaces.push(v.to_string()),
                    "depends" => depends.push(v.to_string()),
                    "conflicts" => conflicts.push(v.to_string()),
                    "provides" => provides.push(v.to_string()),
                    "optdepends" => optdepends.push(v.to_string()),
                    "makedepends" => makedepends.push(v.to_string()),
                    "checkdepends" => checkdepends.push(v.to_string()),
                    "size" => match v.parse::<u32>() {
                        Ok(r) => size = Some(r),
                        Err(e) => Err(PkgInfoError::InvalidData(format!(
                            "Invalid size from {}: {}",
                            v, e
                        )))?,
                    },
                    "builddate" => match v.parse::<u32>() {
                        Ok(r) => builddate = Some(r),
                        Err(e) => Err(PkgInfoError::InvalidData(format!(
                            "Invalid builddate from {}: {}",
                            v, e
                        )))?,
                    },
                    _ => {} // pkgname: String,
                            // size: Option<u32>,
                            // builddate: Option<u32>,
                }
            }
        }

        let Some(pkgname) = pkgname else {
            return Err(PkgInfoError::InvalidData(
                "Missing pkgname entry".to_string(),
            ));
        };
        let Some(pkgver) = pkgver else {
            return Err(PkgInfoError::InvalidData(
                "Missing pkgver entry".to_string(),
            ));
        };
        let version = Version::try_from(pkgver.as_str())
            .map_err(|e| PkgInfoError::InvalidData(format!("Invalid version: {}", e)))?;

        Ok(Self {
            pkgname,
            pkgver,
            version,
            pkgbase,
            pkgdesc,
            size,
            url,
            arch,
            builddate,
            packager,
            groups,
            license,
            replaces,
            depends,
            conflicts,
            provides,
            optdepends,
            makedepends,
            checkdepends,
        })
    }

    pub fn to_desc(
        &self,
        filename: String,
        csize: u64,
        sha256: String,
        pgpsig: Option<String>,
    ) -> DbDesc {
        DbDesc {
            filename,
            csize,
            pgpsig,
            name: self.pkgname.clone(),
            base: self.pkgbase.clone(),
            version: self.pkgver.clone(),
            desc: self.pkgdesc.clone(),
            groups: self.groups.clone(),
            isize: self.size,
            shasum: sha256,
            url: self.url.clone(),
            licenses: self.license.clone(),
            arch: self.arch.clone(),
            builddate: self.builddate,
            packager: self.packager.clone(),
            replaces: self.replaces.clone(),
            conflicts: self.conflicts.clone(),
            provides: self.provides.clone(),
            depends: self.depends.clone(),
            optdepends: self.optdepends.clone(),
            makedepends: self.makedepends.clone(),
            checkdepends: self.checkdepends.clone(),
        }
    }
}
