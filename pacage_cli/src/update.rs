use std::collections::{BTreeSet, HashSet};

use clap::Args;
use crossbeam_channel::{unbounded, Receiver};
use log::{error, info};
use pacage::conf::Package;
use pacage::format::SrcInfo;

use crate::util::dl_and_build;
use crate::{cmd_err, CliCmd};
use pacage::builder;
use pacage::patch::patch;
use pacage::{
    conf::Conf,
    db,
    download::{download_all, download_pkg},
};

#[derive(Args, Debug)]
pub struct Update {
    /// Rebuild packages even if there is no new versions
    #[arg(short)]
    pub force_rebuild: bool,

    /// Skip initial fetch of any news version of the packages, only usefull when some build previously failed
    #[arg(long)]
    pub no_fetch: bool,

    /// Package name
    pub name: Option<String>,
}

impl CliCmd for Update {
    fn execute(&self, conf: Conf) -> Result<(), i32> {
        match &self.name {
            Some(name) => self.update_one(conf, &name),
            None => self.update_all(conf),
        }
    }
}

impl Update {
    fn update_one(&self, mut conf: Conf, name: &str) -> Result<(), i32> {
        let (pkgbuildssender, pkgbuilds) = unbounded::<(SrcInfo, Package)>();
        let pkg = conf.resolve(name);
        if self.no_fetch {
            match SrcInfo::new(&conf.pkgs_dir(), &pkg, false) {
                Ok(srcinfo) => {
                    conf.ensure_pkg(pkg.as_str());
                    let pkg = conf.get(pkg.as_str()).clone();
                    pkgbuildssender.send((srcinfo, pkg)).unwrap();
                }
                Err(e) => error!("[{}] Fail to read .SRCINFO: {}", pkg, e),
            }
            drop(pkgbuildssender);
        } else {
            download_pkg(&mut conf, pkg.as_str(), false, pkgbuildssender).map_err(cmd_err)?;
        }
        let num = dl_and_build(&conf, pkgbuilds, true).map_err(cmd_err)?;
        info!("Updated {} packages(s)", num);
        Ok(())
    }

    fn update_all(&self, mut conf: Conf) -> Result<(), i32> {
        // TODO: get it from install db instead
        let (pkgbuildssender, pkgbuilds) = unbounded::<(SrcInfo, Package)>();
        let mut to_dl = BTreeSet::new();
        for k in &conf.packages {
            to_dl.insert(k.name.clone());
        }
        if self.no_fetch {
            for pkg in to_dl {
                let pkg = conf.resolve(pkg.as_str());
                match SrcInfo::new(&conf.pkgs_dir(), &pkg, false) {
                    Ok(srcinfo) => {
                        conf.ensure_pkg(pkg.as_str());
                        let pkg = conf.get(pkg.as_str()).clone();
                        pkgbuildssender.send((srcinfo, pkg)).unwrap();
                    }
                    Err(e) => error!("[{}] Fail to read .SRCINFO: {}", pkg, e),
                }
            }
            drop(pkgbuildssender);
        } else {
            download_all(&mut conf, to_dl, true, pkgbuildssender).map_err(cmd_err)?;
        };
        let num = dl_and_build(&conf, pkgbuilds, true).map_err(cmd_err)?;
        info!("Updated {} packages(s)", num);
        Ok(())
    }
}
