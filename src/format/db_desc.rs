use log::{error, warn};
use std::io;
use std::io::{BufRead, Lines};
use thiserror::Error;

/*
pacage.db.tar.gz:
==== bash/desc ====
%FILENAME%
bash-5.2.026-2-x86_64.pkg.tar.zst

%NAME%
bash

%BASE%
bash

%VERSION%
5.2.026-2

%DESC%
The GNU Bourne Again shell

%CSIZE%
1911631

%ISIZE%
9441927

%SHA256SUM%
b5430cfb37427821e4b2ba8bacb7271c3af2fa57af51ef4eee2d419f1c07352a

%URL%
https://www.gnu.org/software/bash/bash.html

%LICENSE%
GPL-3.0-or-later

%ARCH%
x86_64

%BUILDDATE%
1718499903

%PACKAGER%
tet <gmorer@pm.me>

%PROVIDES%
sh

%DEPENDS%
readline
libreadline.so=8-64
glibc
ncurses

%OPTDEPENDS%
bash-completion: for tab completion
========
*/

#[derive(Debug, Error)]
pub enum DbDescError {
    #[error("Invalid data: {0}")]
    InvalidData(String),

    #[error("IO Error: {0}")]
    Io(#[from] io::Error),
}

pub struct DbDesc {
    // Original
    file_name: String,
    pub name: String,
    // https://gitlab.archlinux.org/pacman/pacman/-/blob/master/lib/libalpm/version.c
    base: Option<String>,
    pub version: String,
    desc: Option<String>,
    groups: Vec<String>,
    csize: u32,
    isize: u32,
    shasum: String,
    pgpsig: Option<String>,
    url: Option<String>,
    licenses: Vec<String>,
    arch: String,
    builddate: u32,
    packager: String,
    replaces: Vec<String>,
    conflicts: Vec<String>,
    provides: Vec<String>,
    depends: Vec<String>,
    optdepends: Vec<String>,
    makedepends: Vec<String>,
    checkdepends: Vec<String>,
    // Extension
    epoch: Option<u32>,
    pkgrel: String,
}

fn get_val_string(
    lines: &mut Lines<impl BufRead>,
    key: &'static str,
) -> Result<String, DbDescError> {
    let line = match lines.next() {
        Some(Ok(line)) => line,
        Some(Err(e)) => return Err(e.into()),
        None => return Err(DbDescError::InvalidData(format!("Missing {} value", key))),
    };
    match lines.next() {
        Some(Ok(line)) => {
            if !line.is_empty() {
                return Err(DbDescError::InvalidData(format!("{} Too many values", key)));
            }
        }
        Some(Err(e)) => return Err(e.into()),
        None => { /* Last key-value doesnt have a new line*/ }
    }
    Ok(line)
}

fn get_val_vec_string(
    lines: &mut Lines<impl BufRead>,
    _key: &'static str,
) -> Result<Vec<String>, DbDescError> {
    let mut res = Vec::new();
    while let Some(line) = lines.next() {
        match line {
            Ok(line) => {
                if line.is_empty() {
                    break;
                } else {
                    res.push(line);
                }
            }
            Err(e) => return Err(e.into()),
        };
    }
    Ok(res)
}

fn get_val_u32(lines: &mut Lines<impl BufRead>, key: &'static str) -> Result<u32, DbDescError> {
    let res = match lines.next() {
        Some(Ok(line)) => match line.parse::<u32>() {
            Ok(res) => res,
            Err(e) => {
                return Err(DbDescError::InvalidData(format!(
                    "Failed to convert '{}' to u32 for {}: {}",
                    line, key, e
                )))
            }
        },
        Some(Err(e)) => return Err(e.into()),
        None => return Err(DbDescError::InvalidData(format!("Missing {} value", key))),
    };
    match lines.next() {
        Some(Ok(line)) => {
            if !line.is_empty() {
                return Err(DbDescError::InvalidData(format!("{} Too many values", key)));
            }
        }
        Some(Err(e)) => return Err(e.into()),
        None => { /* Last key-value doesnt have a new line*/ }
    }
    Ok(res)
}

impl DbDesc {
    pub fn new(data: impl BufRead) -> Result<Self, DbDescError> {
        let mut name = None;
        let mut file_name = None;
        let mut base = None;
        let mut version = None;
        let mut desc = None;
        let mut groups = Vec::new();
        let mut csize = None;
        let mut isize = None;
        let mut shasum = None;
        let mut pgpsig = None;
        let mut arch = None;
        let mut url = None;
        let mut licenses = Vec::new();
        let mut packager = None;
        let mut replaces = Vec::new();
        let mut conflicts = Vec::new();
        let mut provides = Vec::new();
        let mut depends = Vec::new();
        let mut builddate = None;
        let mut optdepends = Vec::new();
        let mut makedepends = Vec::new();
        let mut checkdepends = Vec::new();
        let mut epoch = None;
        let mut pkgrel = None;
        let mut lines = data.lines();
        while let Some(line) = lines.next() {
            if let Ok(line) = line {
                if line.is_empty() {
                    // key = None;
                    continue;
                }
                match line.as_str() {
                    "%FILENAME%" => file_name = Some(get_val_string(&mut lines, "%FILENAME%")?),
                    "%NAME%" => name = Some(get_val_string(&mut lines, "%NAME%")?),
                    "%BASE%" => base = Some(get_val_string(&mut lines, "%BASE%")?),
                    "%VERSION%" => version = Some(get_val_string(&mut lines, "%VERSION%")?),
                    "%DESC%" => desc = Some(get_val_string(&mut lines, "%DESC%")?),
                    "%GROUPS%" => groups = get_val_vec_string(&mut lines, "%GROUPS%")?,
                    "%CSIZE%" => csize = Some(get_val_u32(&mut lines, "%CSIZE%")?),
                    "%ISIZE%" => isize = Some(get_val_u32(&mut lines, "%ISIZE%")?),
                    "%SHA256SUM%" => shasum = Some(get_val_string(&mut lines, "%SHA256SUM%")?),
                    "%PGPSIG%" => pgpsig = Some(get_val_string(&mut lines, "%PGPSIG%")?),
                    "%URL%" => url = Some(get_val_string(&mut lines, "%URL%")?),
                    "%LICENSE%" => licenses = get_val_vec_string(&mut lines, "%LICENSE%")?,
                    "%ARCH%" => arch = Some(get_val_string(&mut lines, "%ARCH%")?),
                    "%BUILDDATE%" => builddate = Some(get_val_u32(&mut lines, "%BUILDDATE%")?),
                    "%PACKAGER%" => packager = Some(get_val_string(&mut lines, "%PACKAGER%")?),
                    "%REPLACES%" => replaces = get_val_vec_string(&mut lines, "%REPLACES%")?,
                    "%CONFLICTS%" => conflicts = get_val_vec_string(&mut lines, "%CONFLICTS%")?,
                    "%PROVIDES%" => provides = get_val_vec_string(&mut lines, "%PROVIDES%")?,
                    "%DEPENDS%" => depends = get_val_vec_string(&mut lines, "%DEPENDS%")?,
                    "%OPTDEPENDS%" => optdepends = get_val_vec_string(&mut lines, "%OPTDEPENDS%")?,
                    "%MAKEDEPENDS%" => {
                        makedepends = get_val_vec_string(&mut lines, "%MAKEDEPENDS%")?
                    }
                    "%CHECKDEPENDS%" => {
                        checkdepends = get_val_vec_string(&mut lines, "%CHECKDEPENDS%")?
                    }
                    // Extension
                    "%EPOCH%" => epoch = Some(get_val_u32(&mut lines, "%EPOCH%")?),
                    "%PKGREL%" => pkgrel = Some(get_val_string(&mut lines, "%RELEASE%")?),
                    a => warn!("DB desc unknown property: {}", a),
                }
            }
        }
        let Some(file_name) = file_name else {
            return Err(DbDescError::InvalidData(
                "Missing %FILENAME% value".to_string(),
            ));
        };
        let Some(name) = name else {
            return Err(DbDescError::InvalidData("Missing %NAME% value".to_string()));
        };
        let Some(version) = version else {
            return Err(DbDescError::InvalidData(
                "Missing %VERSION% value".to_string(),
            ));
        };
        let Some(isize) = isize else {
            return Err(DbDescError::InvalidData(
                "Missing %ISIZE% value".to_string(),
            ));
        };
        let Some(csize) = csize else {
            return Err(DbDescError::InvalidData(
                "Missing %CSIZE% value".to_string(),
            ));
        };
        let Some(shasum) = shasum else {
            return Err(DbDescError::InvalidData(
                "Missing %SHA256SUM% value".to_string(),
            ));
        };
        let Some(arch) = arch else {
            return Err(DbDescError::InvalidData("Missing %ARCH% value".to_string()));
        };
        let Some(builddate) = builddate else {
            return Err(DbDescError::InvalidData(
                "Missing %BUILDDATE% value".to_string(),
            ));
        };
        let Some(packager) = packager else {
            return Err(DbDescError::InvalidData(
                "Missing %PACKAGER% value".to_string(),
            ));
        };
        let Some(pkgrel) = pkgrel else {
            return Err(DbDescError::InvalidData(
                "Missing %PKGREL% value".to_string(),
            ));
        };
        Ok(Self {
            file_name,
            name,
            base,
            version,
            desc,
            groups,
            csize,
            isize,
            shasum,
            pgpsig,
            url,
            licenses,
            arch,
            builddate,
            packager,
            replaces,
            conflicts,
            provides,
            depends,
            optdepends,
            makedepends,
            checkdepends,
            pkgrel,
            epoch,
        })
    }
}
