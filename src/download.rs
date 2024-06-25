use log::{error, info};
use std::borrow::Borrow;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::io::{self, BufRead, BufReader};

use crate::cmd::{command, CmdError, ExecError, NOENV};
use crate::conf::{Package, Repo};
use crate::Conf;
use thiserror::Error;

#[derive(Debug)]
pub struct SrcInfo {
    pub name: String,
    pub version: String,
    pub release: String,
    // TODO(feat): deps
    pub deps: Vec<String>,
    pub src: bool,

    // The package doesnt need to be build
    pub build: bool,
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

// pub struct SrcInfoWithMakePkg<'a>(pub (SrcInfo, Option<&'a Makepkg>));

// impl std::cmp::PartialEq for SrcInfoWithMakePkg<'_> {
//     fn eq(&self, other: &Self) -> bool {
//         self.0 .0.name == other.0 .0.name
//     }
// }
// impl std::cmp::Eq for SrcInfoWithMakePkg<'_> {}

// impl std::hash::Hash for SrcInfoWithMakePkg<'_> {
//     fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
//         self.0 .0.name.hash(state);
//     }
// }

#[derive(Debug, Error)]
pub enum ParsingError {
    #[error("Failed to generate SRCINFO: {0}")]
    Cmd(#[from] CmdError),

    #[error("System errro while parsing .SRCINFO : {0}")]
    SrcInfo(#[from] io::Error),

    #[error("System errro while reading PKGBUILD : {0}")]
    PkgBuild(String),
}

impl SrcInfo {
    fn parse<'a, I>(lines: I, build: bool) -> Result<Self, ParsingError>
    where
        I: IntoIterator,
        I::Item: Borrow<str>,
    {
        let mut name = None;
        let mut version = None;
        let mut release = None;
        let mut deps = Vec::new();
        let mut src = false;
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
                    "depends" => deps.push(v.to_string()),
                    "source" => src = true,
                    _ => {}
                }
            }
        }
        if name.is_some() && version.is_some() && release.is_some() {
            return Ok(Self {
                name: name.unwrap(),
                version: version.unwrap(),
                release: release.unwrap(),
                deps,
                src,
                build,
            });
        }
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Missing field in pkgver, name: {:?} version: {:?} releasze: {:?}",
                name, version, release
            ),
        ))?
    }

    // Not the best way :/
    // TODO: dont do that
    fn can_build(conf: &Conf, pkg_name: &str) -> Result<bool, ParsingError> {
        let path = conf.server_dir.join("pkgs").join(pkg_name).join("PKGBUILD");
        let file = fs::File::open(path).map_err(|e| ParsingError::PkgBuild(e.to_string()))?;
        for line in BufReader::new(file).lines() {
            if let Ok(line) = line {
                if line == "build() {" {
                    return Ok(true);
                }
            }
        }
        return Ok(false);
    }

    pub fn new(conf: &Conf, pkg_name: &str) -> Result<Self, ParsingError> {
        let path = conf.server_dir.join("pkgs").join(pkg_name).join(".SRCINFO");
        let build = Self::can_build(conf, pkg_name)?;
        if !path.exists() {
            let pkgs_dir = conf.server_dir.join("pkgs").join(pkg_name);
            let (status, out, _) =
                command(&["makepkg", "--printsrcinfo"], &pkgs_dir, NOENV).unwrap();
            if !status.success() {
                return Err(ParsingError::Cmd(CmdError::from_output(out)));
            }
            let content = out.join("\n");
            Self::parse(content.lines(), build)
        } else {
            let file = fs::File::open(path)?;
            Self::parse(
                BufReader::new(file).lines().filter_map(|l| match l {
                    Ok(l) => Some(l),
                    Err(_) => None,
                }),
                build,
            )
        }
    }
}

#[derive(Debug, Error)]
pub enum DownloadError {
    #[error("System error: {0}")]
    System(#[from] ExecError),

    #[error("Cmd error: Erno: {0}")]
    Cmd(#[from] CmdError),

    // #[error("Failed to parse PKGBUILD : {0}")]
    // PkgBuild(#[from] io::Error),
    #[error("Not Found")]
    NotFound(Vec<String>),

    #[error("Parsing error: {0}")]
    Parsing(#[from] ParsingError),
}

// IO error
// Cmd error
// Exec error

// Should return a list of packages to build

// const PARALLEL_DOWNLOAD: usize = 5;

pub fn fetch_pkg(conf: &Conf, pkg: &Package) -> Result<SrcInfo, DownloadError> {
    let pkg_dir = conf.pkg_dir(&pkg.name);
    if pkg_dir.exists() {
        fs::remove_dir_all(pkg_dir).ok();
    }
    match &pkg.repo {
        Repo::None => {
            let pkg = conf
                .resolver
                .get(&pkg.name)
                .map(|a| a.as_str())
                .unwrap_or(pkg.name.as_str());
            let pkgs_dir = conf.server_dir.join("pkgs");
            let (status, out, _) = command(
                &["pkgctl", "repo", "clone", "--protocol=https", &pkg],
                &pkgs_dir,
                Some([("GIT_TERMINAL_PROMPT", "0")]),
            )?;
            if status.success() {
                info!("[{}] Download package", pkg);
                Ok(SrcInfo::new(conf, pkg)?)
            } else {
                error!("[{}] Failed to download", pkg);
                Err(DownloadError::NotFound(out))?
                // Err(CmdError::from_output(out))?
            }
        }
        Repo::Aur => {
            unimplemented!()
        }
        Repo::Git(_a) => {
            unimplemented!()
        }
        Repo::File(_d) => {
            unimplemented!()
        }
    }
}

// fn update_pkg(conf: &Conf, pkg: &str, pkg_dir: &PathBuf) -> Result<(bool, SrcInfo), DownloadError> {
//     info!("[{}] git rev-parse HEAD", pkg);
//     let (status, previous, _) = command(
//         &["git", "rev-parse", "HEAD"],
//         &pkg_dir,
//         None::<Vec<(String, String)>>,
//     )?;
//     if !status.success() {
//         return Err((CmdError::from_output(previous)).into());
//     };

//     info!("[{}] git pull", pkg);
//     let (status, out, _) = command(&["git", "pull"], &pkg_dir, NOENV)?;
//     if !status.success() {
//         Err(CmdError::from_output(out))?
//     }

//     info!("[{}] git rev-parse HEAD", pkg);
//     /* Getting the new version */
//     let (status, new, _) = command(&["git", "rev-parse", "HEAD"], pkg_dir, NOENV)?;
//     if !status.success() {
//         return Err((CmdError::from_output(new)).into());
//     }
//     let pkg_build = SrcInfo::new(conf, pkg)?;
//     if previous.get(0) != new.get(0) {
//         Ok((true, pkg_build))
//     } else {
//         Ok((false, pkg_build))
//     }
// }

// TODO: check for deps there
pub fn download_pkg(
    conf: &mut Conf,
    name: &str,
    continue_on_err: bool,
) -> Result<HashSet<SrcInfo>, DownloadError> {
    let mut pkgs = BTreeSet::new();
    pkgs.insert(name.to_string());
    download_all(conf, pkgs, continue_on_err)
}

pub fn download_all<'a>(
    conf: &'a mut Conf,
    mut pkgs: BTreeSet<String>,
    continue_on_err: bool,
) -> Result<HashSet<SrcInfo>, DownloadError> {
    let mut done: HashMap<String, SrcInfo> = HashMap::new();
    let mut errored: HashMap<String, DownloadError> = HashMap::new();

    while let Some(pkg) = pkgs.pop_first() {
        if done.contains_key(&pkg) || errored.contains_key(&pkg) {
            continue;
        }
        info!("[{}] Downloading...", pkg);
        conf.ensure_pkg(pkg.as_str());
        let pkg = conf.get(&pkg);
        let pkg_build = match fetch_pkg(conf, &pkg) {
            Ok(p) => p,
            Err(e) => {
                if continue_on_err {
                    errored.insert(pkg.name.clone(), e);
                    continue;
                } else {
                    return Err(e);
                }
            }
        };
        if conf.need_deps(&pkg) {
            for dep in &pkg_build.deps {
                if !done.contains_key(dep) && !errored.contains_key(dep) {
                    pkgs.insert(dep.clone());
                }
            }
        }
        info!("[{}] Downloaded", pkg.name);
        // TODO: no clone
        done.insert(pkg.name.clone(), pkg_build);
    }
    let mut res = HashSet::with_capacity(done.len());
    for (pkg, infos) in done {
        // if infos.build {
        res.insert(infos);
        // } else {
        // info!("[{}] Wont be build, it cannot be", pkg);
        // }
    }
    if !errored.is_empty() {
        error!("Issues while downloading pkgs: ");
        for (name, e) in errored {
            error!("[{}] Failed: {:?}", name, e);
        }
    }
    Ok(res)
}

// TODO: HashSet instead of hashmap
// pub fn download_all<'a>(
// ) -> Result<HashSet<SrcInfo>, DownloadError> {
//     for (name, _) in pkgs.iter() {
//         match _download_pkg(conf, name, force_rebuild, &mut to_build) {
//             Ok(res) => res,
//             Err(e) => {
//                 error!("[{}] Fail to download: {}", name, e);
//                 if !continue_on_err {
//                     return Err(e);
//                 }
//             }
//         }
//     }
//     Ok(to_build)
// }
