use std::collections::BTreeSet;

use clap::Args;

use crate::builder;
use crate::patch::patch;
use crate::{
    cli::{cmd_err, CliCmd},
    db,
    download::{download_all, download_pkg},
    Conf,
};

#[derive(Args, Debug)]
pub struct Update {
    /// Rebuild packages even if there is no new versions
    #[arg(short)]
    pub force_rebuild: bool,

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
        let pkgbuilds = download_pkg(&mut conf, &name, false).map_err(cmd_err)?;
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
            db::add(&conf, &pkg.name).map_err(cmd_err)?;
        }
        Ok(())
    }

    fn update_all(&self, mut conf: Conf) -> Result<(), i32> {
        // TODO: get it from install db instead
        // TODO: check if we have src from these, get SRCINFO from it
        // TODO: redownload theses :/ (maybe we shoud not :/)
        let mut to_dl = BTreeSet::new();
        for k in &conf.packages {
            to_dl.insert(k.name.clone());
        }
        let pkgbuilds = download_all(&mut conf, to_dl, true).map_err(cmd_err)?;
        // Check whats installed
        if !builder::should_build(&pkgbuilds) {
            println!("Nothing to do :)");
            return Ok(());
        }
        println!("pkgs to build: {:?}", pkgbuilds);
        let builder = builder::Builder::new(&conf).map_err(cmd_err)?;
        for pkgbuild in pkgbuilds {
            if pkgbuild.src == false {
                continue;
            }
            let name = &pkgbuild.name;
            conf.ensure_pkg(name);
            let pkg = conf.get(name.clone());
            let makepkg = pkg.makepkg.as_ref();
            builder
                .download_src(&conf, name, makepkg)
                .map_err(cmd_err)?;
            patch(&conf, &pkgbuild).map_err(cmd_err)?;
            builder.build_pkg(&conf, pkg).map_err(cmd_err)?;
            db::add(&conf, name).map_err(cmd_err)?;
        }
        Ok(())
    }
}
