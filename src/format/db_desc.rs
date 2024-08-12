use log::{error, warn};
use std::io::{self, Write};
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
mod desc {
    pub const FILENAME: &str = "%FILENAME%";
    pub const NAME: &str = "%NAME%";
    pub const BASE: &str = "%BASE%";
    pub const VERSION: &str = "%VERSION%";
    pub const DESC: &str = "%DESC%";
    pub const GROUPS: &str = "%GROUPS%";
    pub const CSIZE: &str = "%CSIZE%";
    pub const ISIZE: &str = "%ISIZE%";
    pub const SHA256SUM: &str = "%SHA256SUM%";
    pub const PGPSIG: &str = "%PGPSIG%";
    pub const URL: &str = "%URL%";
    pub const LICENSE: &str = "%LICENSE%";
    pub const ARCH: &str = "%ARCH%";
    pub const BUILDDATE: &str = "%BUILDDATE%";
    pub const PACKAGER: &str = "%PACKAGER%";
    pub const REPLACES: &str = "%REPLACES%";
    pub const CONFLICTS: &str = "%CONFLICTS%";
    pub const PROVIDES: &str = "%PROVIDES%";
    pub const DEPENDS: &str = "%DEPENDS%";
    pub const OPTDEPENDS: &str = "%OPTDEPENDS%";
    pub const MAKEDEPENDS: &str = "%MAKEDEPENDS%";
    pub const CHECKDEPENDS: &str = "%CHECKDEPENDS%";
}

#[derive(PartialEq, Eq, Debug)]
pub struct DbDesc {
    // Original
    pub filename: String,
    pub name: String,
    pub base: Option<String>,
    // [epoch:]version[-release]
    pub version: String,
    pub desc: Option<String>,
    pub groups: Vec<String>,
    pub csize: u64,
    pub isize: Option<u32>,
    pub shasum: String,
    pub pgpsig: Option<String>,
    pub url: Option<String>,
    pub licenses: Vec<String>,
    pub arch: Option<String>,
    pub builddate: Option<u32>,
    pub packager: Option<String>,
    pub replaces: Vec<String>,
    pub conflicts: Vec<String>,
    pub provides: Vec<String>,
    pub depends: Vec<String>,
    pub optdepends: Vec<String>,
    pub makedepends: Vec<String>,
    pub checkdepends: Vec<String>,
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
        let mut lines = data.lines();
        while let Some(line) = lines.next() {
            if let Ok(line) = line {
                if line.is_empty() {
                    continue;
                }
                match line.as_str() {
                    desc::FILENAME => file_name = Some(get_val_string(&mut lines, desc::FILENAME)?),
                    desc::NAME => name = Some(get_val_string(&mut lines, desc::NAME)?),
                    desc::BASE => base = Some(get_val_string(&mut lines, desc::BASE)?),
                    desc::VERSION => version = Some(get_val_string(&mut lines, desc::VERSION)?),
                    desc::DESC => desc = Some(get_val_string(&mut lines, desc::DESC)?),
                    desc::GROUPS => groups = get_val_vec_string(&mut lines, desc::GROUPS)?,
                    desc::CSIZE => csize = Some(get_val_u32(&mut lines, desc::CSIZE)? as u64),
                    desc::ISIZE => isize = Some(get_val_u32(&mut lines, desc::ISIZE)?),
                    desc::SHA256SUM => shasum = Some(get_val_string(&mut lines, desc::SHA256SUM)?),
                    desc::PGPSIG => pgpsig = Some(get_val_string(&mut lines, desc::PGPSIG)?),
                    desc::URL => url = Some(get_val_string(&mut lines, desc::URL)?),
                    desc::LICENSE => licenses = get_val_vec_string(&mut lines, desc::LICENSE)?,
                    desc::ARCH => arch = Some(get_val_string(&mut lines, desc::ARCH)?),
                    desc::BUILDDATE => builddate = Some(get_val_u32(&mut lines, desc::BUILDDATE)?),
                    desc::PACKAGER => packager = Some(get_val_string(&mut lines, desc::PACKAGER)?),
                    desc::REPLACES => replaces = get_val_vec_string(&mut lines, desc::REPLACES)?,
                    desc::CONFLICTS => conflicts = get_val_vec_string(&mut lines, desc::CONFLICTS)?,
                    desc::PROVIDES => provides = get_val_vec_string(&mut lines, desc::PROVIDES)?,
                    desc::DEPENDS => depends = get_val_vec_string(&mut lines, desc::DEPENDS)?,
                    desc::OPTDEPENDS => {
                        optdepends = get_val_vec_string(&mut lines, desc::OPTDEPENDS)?
                    }
                    desc::MAKEDEPENDS => {
                        makedepends = get_val_vec_string(&mut lines, desc::MAKEDEPENDS)?
                    }
                    desc::CHECKDEPENDS => {
                        checkdepends = get_val_vec_string(&mut lines, desc::CHECKDEPENDS)?
                    }
                    // Extension
                    a => warn!("DB desc unknown property: {}", a),
                }
            }
        }
        let Some(file_name) = file_name else {
            return Err(DbDescError::InvalidData(format!(
                "Missing {} value",
                desc::FILENAME
            )));
        };
        let Some(name) = name else {
            return Err(DbDescError::InvalidData(format!(
                "Missing {} value",
                desc::NAME
            )));
        };
        let Some(version) = version else {
            return Err(DbDescError::InvalidData(format!(
                "Missing {} value",
                desc::VERSION
            )));
        };
        let Some(csize) = csize else {
            return Err(DbDescError::InvalidData(format!(
                "Missing {} value",
                desc::CSIZE
            )));
        };
        let Some(shasum) = shasum else {
            return Err(DbDescError::InvalidData(format!(
                "Missing {} value",
                desc::SHA256SUM
            )));
        };
        Ok(Self {
            filename: file_name,
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
        })
    }

    fn write_list(
        list: &Vec<String>,
        writer: &mut impl Write,
        key: &'static str,
    ) -> Result<(), io::Error> {
        if !list.is_empty() {
            writer.write(b"\n\n")?;
            writer.write(key.as_bytes())?;
            for item in list {
                writer.write(b"\n")?;
                writer.write(item.as_bytes())?;
            }
        }
        Ok(())
    }

    pub fn write(&self, mut writer: impl Write) -> Result<(), DbDescError> {
        for (value, key) in [
            (&self.filename, desc::FILENAME),
            (&self.name, desc::NAME),
            (&self.version, desc::VERSION),
            (&self.shasum, desc::SHA256SUM),
        ] {
            writer.write(b"\n\n")?;
            writer.write(key.as_bytes())?;
            writer.write(b"\n")?;
            writer.write(value.as_bytes())?;
        }
        writer.write(format!("\n\n{}\n{}", desc::CSIZE, self.csize).as_bytes())?;
        if let Some(isize) = self.isize {
            writer.write(format!("\n\n{}\n{}", desc::ISIZE, isize).as_bytes())?;
        }
        if let Some(builddate) = self.builddate {
            writer.write(format!("\n\n{}\n{}", desc::BUILDDATE, builddate).as_bytes())?;
        }
        for (value, key) in [
            (&self.base, desc::BASE),
            (&self.desc, desc::DESC),
            (&self.pgpsig, desc::PGPSIG),
            (&self.url, desc::URL),
            (&self.arch, desc::ARCH),
            (&self.packager, desc::PACKAGER),
        ] {
            if let Some(value) = value {
                writer.write(b"\n\n")?;
                writer.write(key.as_bytes())?;
                writer.write(b"\n")?;
                writer.write(value.as_bytes())?;
            }
        }
        for (list, key) in [
            (&self.groups, desc::GROUPS),
            (&self.licenses, desc::LICENSE),
            (&self.replaces, desc::REPLACES),
            (&self.conflicts, desc::CONFLICTS),
            (&self.provides, desc::PROVIDES),
            (&self.depends, desc::DEPENDS),
            (&self.optdepends, desc::OPTDEPENDS),
            (&self.makedepends, desc::MAKEDEPENDS),
            (&self.checkdepends, desc::CHECKDEPENDS),
        ] {
            Self::write_list(list, &mut writer, key)?;
        }
        Ok(())
    }
}

// key = None;
#[cfg(test)]
mod tests {
    use io::BufReader;

    use super::*;

    #[test]
    fn valid() {
        let orig = DbDesc {
            filename: "test.pkg".to_string(),
            name: "pkgname".to_string(),
            base: None,
            version: "aaaa".to_string(),
            desc: Some("Some random pkg test".to_string()),
            groups: vec!["base".to_string()],
            csize: 32,
            isize: Some(32),
            shasum: "crypto".to_string(),
            pgpsig: None,
            url: Some("www.com".to_string()),
            licenses: vec!["aaaa".to_string()],
            arch: Some("aarch64".to_string()),
            builddate: Some(32),
            packager: Some("madness".to_string()),
            replaces: vec![],
            conflicts: vec![],
            provides: vec!["good_testing".to_string()],
            depends: vec!["good_coding".to_string()],
            optdepends: vec![],
            makedepends: vec![],
            checkdepends: vec!["testsss".to_string()],
            // Extension
        };
        let mut data = Vec::new();
        orig.write(&mut data).unwrap();
        let reader = BufReader::new(data.as_slice());
        let res = DbDesc::new(reader).unwrap();
        assert_eq!(res, orig);
    }
}
