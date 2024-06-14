use std::collections::BTreeSet;

use clap::Args;

use crate::builder;
use crate::patch::patch;
use crate::{
    cli::{cmd_err, CliCmd},
    download::{download_all, download_pkg},
    repo, Conf,
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
    fn execute(&self, conf: &Conf) -> Result<(), i32> {
        match &self.name {
            Some(name) => self.update_one(conf, &name),
            None => self.update_all(conf),
        }
    }
}

impl Update {
    fn update_one(&self, conf: &Conf, name: &str) -> Result<(), i32> {
        let pkgbuilds = download_pkg(&conf, &name, self.force_rebuild, false).map_err(cmd_err)?;
        if !builder::should_build(&pkgbuilds) {
            println!("Nothing to do :)");
            return Ok(());
        }
        let makepkg = conf
            .packages
            .get(name)
            .map(|p| p.makepkg.as_ref())
            .flatten();
        let builder = builder::Builder::new(&conf).map_err(cmd_err)?;
        builder
            .download_src(&conf, name, makepkg)
            .map_err(cmd_err)?;
        for pkgbuild in pkgbuilds {
            if pkgbuild.src == false {
                continue;
            }
            patch(&conf, &pkgbuild).map_err(cmd_err)?;
            builder
                .build_pkg(conf, &pkgbuild.name, makepkg)
                .map_err(cmd_err)?;
            repo::add(&conf, name).map_err(cmd_err)?;
        }
        Ok(())
    }

    fn update_all(&self, conf: &Conf) -> Result<(), i32> {
        let mut to_dl = BTreeSet::new();
        for (k, _) in &conf.packages {
            to_dl.insert(k.to_string());
        }
        let pkgbuilds = download_all(&conf, to_dl, self.force_rebuild, true).map_err(cmd_err)?;
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
            let makepkg = conf
                .packages
                .get(name)
                .map(|p| p.makepkg.as_ref())
                .flatten();
            builder
                .download_src(&conf, name, makepkg)
                .map_err(cmd_err)?;
            patch(&conf, &pkgbuild).map_err(cmd_err)?;
            builder.build_pkg(conf, name, makepkg).map_err(cmd_err)?;
            repo::add(&conf, name).map_err(cmd_err)?;
        }
        Ok(())
    }
}
