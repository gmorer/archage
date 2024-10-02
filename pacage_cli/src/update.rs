use std::collections::{BTreeSet, HashSet};

use clap::Args;
use log::error;
use pacage::format::SrcInfo;

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
        let pkgbuilds = if !self.no_fetch {
            download_pkg(&mut conf, &name, false).map_err(cmd_err)?
        } else {
            let mut res = HashSet::new();
            res.insert(SrcInfo::new(&conf, name).map_err(|e| {
                error!("Failed to parse .SRCINFO: {}", e);
                2
            })?);
            res
        };
        if !builder::should_build(&pkgbuilds) {
            println!("Nothing to do :)");
            return Ok(());
        }
        let name = name.to_string();
        conf.ensure_pkg(&name);
        let pkg = conf.get(name);
        let builder = builder::Builder::new(&conf).map_err(cmd_err)?;
        builder
            .download_src(&conf, &pkg.name, pkg.makepkg.as_ref())
            .map_err(cmd_err)?;
        for pkgbuild in pkgbuilds {
            if pkgbuild.src == false {
                continue;
            }
            patch(&conf, &pkgbuild).map_err(cmd_err)?;
            builder.build_pkg(&conf, pkg).map_err(cmd_err)?;
            db::add(&conf, &pkgbuild).map_err(cmd_err)?;
        }
        Ok(())
    }

    fn update_all(&self, mut conf: Conf) -> Result<(), i32> {
        // TODO: get it from install db instead
        let mut to_dl = BTreeSet::new();
        for k in &conf.packages {
            to_dl.insert(k.name.clone());
        }
        let mut pkgbuilds = if self.no_fetch {
            let mut res = HashSet::new();
            for pkg in to_dl {
                match SrcInfo::new(&conf, &pkg) {
                    Ok(pkg) => {
                        res.insert(pkg);
                    }
                    Err(e) => error!("[{}] Fail to read .SRCINFO: {}", pkg, e),
                }
            }
            res
        } else {
            download_all(&mut conf, to_dl, true).map_err(cmd_err)?
        };

        // Check if package is already there
        if let Ok(dbpkgs) = db::list(&conf) {
            pkgbuilds.retain(|wanted_pkg| {
                for db_package in &dbpkgs {
                    if wanted_pkg.name == db_package.name
                        && wanted_pkg.get_version() == db_package.get_version()
                    {
                        return false;
                    }
                }
                true
            })
        }
        let builder = builder::Builder::new(&conf).map_err(cmd_err)?;
        for pkgbuild in pkgbuilds {
            if pkgbuild.src == false {
                continue;
            }
            let name = &pkgbuild.name;
            conf.ensure_pkg(name);
            let pkg = conf.get(name.clone());
            let makepkg = pkg.makepkg.as_ref();
            if let Err(e) = builder.download_src(&conf, name, makepkg).map_err(cmd_err) {
                error!(
                    "[{}] Skipping build, failed to download sources: {}",
                    name, e
                );
                continue;
            }
            if let Err(e) = patch(&conf, &pkgbuild).map_err(cmd_err) {
                error!("[{}] Skipping build, failed to patch: {}", name, e);
                continue;
            }
            if let Err(e) = builder.build_pkg(&conf, pkg).map_err(cmd_err) {
                error!("[{}] Skipping build, failed to build: {}", name, e);
                continue;
            }
            if let Err(e) = db::add(&conf, &pkgbuild).map_err(cmd_err) {
                error!(
                    "[{}] Skipping build, failed to insert in the database: {}",
                    name, e
                );
                continue;
            }
        }
        Ok(())
    }
}
