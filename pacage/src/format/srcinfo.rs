use super::ParsingError;
use crate::cmd::{command, CmdError, NOENV};
use crate::conf::Conf;
use crate::utils::version::Version;
use std::borrow::Borrow;
use std::fs;
use std::io::{self, Write};
use std::io::{BufRead, BufReader};
use thiserror::Error;

/*
bash:
==== .SRCINFO ====
pkgbase = bash
    pkgdesc = The GNU Bourne Again shell
    pkgver = 5.2.026
    pkgrel = 5
    url = https://www.gnu.org/software/bash/bash.html
    install = bash.install
    arch = x86_64
    license = GPL-3.0-or-later
    depends = readline
    depends = libreadline.so
    depends = glibc
    depends = ncurses
    optdepends = bash-completion: for tab completion
    provides = sh
    backup = etc/bash.bashrc
    [...]
    backup = etc/skel/.bash_logout
    source = https://ftp.gnu.org/gnu/bash/bash-5.2.tar.gz
    source = https://ftp.gnu.org/gnu/bash/bash-5.2.tar.gz.sig
    source = bash-5.2_p15-configure-clang16.patch
    [...]
    source = https://ftp.gnu.org/gnu/bash/bash-5.2-patches/bash52-026.sig
    validpgpkeys = 7C0135FB088AAF6C66C650B9BB5869F064EA74AB
    b2sums = 51b196e710794ebad8eac28c31c93eb99ac1a7db30919a13271e39e1cb66a0672f242df75fc7d71627ea873dfbce53ec35c0c56a71c5167143070a7811343fd9
    b2sums = SKIP
    b2sums = 5ef332cd2847f46e351e5db6dda79d01d9853f5eda9762deeba0450c2bd400eec549bbb85696777b687f64d0977daac4883d6ce3f1e26cec0d5f73e8ee97f000
    [...]
    b2sums = d00a8b4fb3babf52c67a3e345158c1f70b5b45e5a54100a6671d96f9cfbf893143d5a23df7e7c5f4d5c0bd650519fb0c447b2304db2d6e0751dfffa651a7cf49
    b2sums = SKIP
    b2sums = b3b7e2511823a0527aeed5af2c8d9f44e5ab079fa8b3f48fe84b35a14327d0143e14e04316c16bfbe2a1cac0c7fcf7ab5058a2b00be38ed3243b53b786e969f1
    b2sums = SKIP
    [...]
    b2sums = ebe3bc47dadf5d689258c5ccf9883838d3383dc43bec68d2a6767b6348cf1515a98ec9e445c3110e8eb0d87e742c20a0d4ddb70649ec94217f55aad7d18552af
    b2sums = SKIP

pkgname = bash
========
*/

// TODO: Caution, from arch wiki:
// The following fields may, additionally, specify multiple architectures as shown below:
// source_x86_64 = https://foo.bar/file.tar.gz
// source_i686 = https://foo.bar/file_i686_patch.tar.gz
//     source
//     depends, checkdepends, makedepends, optdepends
//     provides, conflicts, replaces
//     md5sums, sha1sums, sha224sums, sha256sums, sha384sums, sha512sums

#[derive(Debug, Error)]
pub enum SrcInfoError {
    #[error("System command error: {0}")]
    Cmd(#[from] CmdError),

    #[error("Invalid data: {0}")]
    InvalidData(String),

    #[error("IO Error: {0}")]
    Io(io::Error),
}

#[derive(Debug)]
pub struct SrcInfo {
    pub name: String,
    pub pkgver: String, // Cannot contain "-"
    pub pkgrel: Option<String>,
    pub epoch: Option<u32>,
    pub deps: Vec<String>,
    pub src: bool,
    _version: Version,
}

impl std::cmp::PartialEq for SrcInfo {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl std::hash::Hash for SrcInfo {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}
impl std::cmp::Eq for SrcInfo {}

impl SrcInfo {
    fn parse<'a, I>(lines: I) -> Result<Self, ParsingError>
    where
        I: IntoIterator,
        I::Item: Borrow<str>,
    {
        // TODO: handle multiple pkgname
        let mut name = None;
        let mut version = None;
        let mut deps = Vec::new();
        let mut src = false;
        let mut epoch = None;
        let mut release = None;
        for line in lines {
            let line = line.borrow();
            if let Some(n) = line.find('=') {
                if n == line.len() {
                    continue;
                }
                let key = line[..n].trim();
                let v = line[(n + 1)..].trim();
                match key {
                    "pkgbase" => name = Some(v.to_string()),
                    "pkgver" => version = Some(v.to_string()),
                    "pkgrel" => release = Some(v.to_string()),
                    "epoch" => match v.parse::<u32>() {
                        Ok(r) => epoch = Some(r),
                        Err(e) => Err(SrcInfoError::InvalidData(format!(
                            "Invalid epoch from {}: {}",
                            v, e
                        )))?,
                    },
                    "depends" => deps.push(v.to_string()),
                    "source" => src = true,
                    _ => {}
                }
            }
        }
        match (&name, &version) {
            (Some(name), Some(version)) => {
                let version = version.to_string();
                return Ok(Self {
                    _version: Version::new(&version, release.as_deref(), epoch),
                    name: name.to_string(),
                    pkgver: version,
                    pkgrel: release,
                    epoch,
                    deps,
                    src,
                });
            }
            _ => Err(SrcInfoError::InvalidData(format!(
                "Missing field in pkgver, name: {:?} version: {:?} releasze: {:?}",
                name, version, release
            )))?,
        }
    }

    // Not the best way :/
    // TODO: dont do that
    // fn can_build(conf: &Conf, pkg_name: &str) -> Result<bool, ParsingError> {
    //     let path = conf.server_dir.join("pkgs").join(pkg_name).join("PKGBUILD");
    //     let file = fs::File::open(path).map_err(|e| ParsingError::PkgBuild(e.to_string()))?;
    //     for line in BufReader::new(file).lines() {
    //         if let Ok(line) = line {
    //             if line == "build() {" {
    //                 return Ok(true);
    //             }
    //         }
    //     }
    //     return Ok(false);
    // }

    pub fn new(conf: &Conf, pkg_name: &str) -> Result<Self, ParsingError> {
        let path = conf.server_dir.join("pkgs").join(pkg_name).join(".SRCINFO");
        // let build = Self::can_build(conf, pkg_name)?;
        if !path.exists() {
            let pkgs_dir = conf.server_dir.join("pkgs").join(pkg_name);
            let (status, out, _) =
                command(&["makepkg", "--printsrcinfo"], &pkgs_dir, NOENV).unwrap();
            if !status.success() {
                return Err(SrcInfoError::Cmd(CmdError::from_output(out)).into());
            }
            let content = out.join("\n");
            if let Ok(mut f) = fs::File::create(path) {
                f.write_all(content.as_bytes())
                    .map(|_| f.sync_all().ok())
                    .ok();
            }
            Self::parse(content.lines())
        } else {
            let file = fs::File::open(path).map_err(|e| SrcInfoError::Io(e))?;
            Self::parse(BufReader::new(file).lines().filter_map(|l| match l {
                Ok(l) => Some(l),
                Err(_) => None,
            }))
        }
    }

    pub fn get_version(&self) -> &Version {
        &self._version
    }
}
