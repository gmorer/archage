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
        let pkgbuild = download_pkg(&conf, &name, self.force_rebuild).map_err(cmd_err)?;
        let makepkg = conf
            .packages
            .get(name)
            .map(|p| p.makepkg.as_ref())
            .flatten();
        let builder = builder::Builder::new(&conf).map_err(cmd_err)?;
        builder
            .download_src(&conf, name, makepkg)
            .map_err(cmd_err)?;
        patch(&conf, &pkgbuild).map_err(cmd_err)?;
        builder.build_pkg(conf, name, makepkg).map_err(cmd_err)?;
        repo::add(&conf, name).map_err(cmd_err)?;
        Ok(())
    }

    fn update_all(&self, conf: &Conf) -> Result<(), i32> {
        let pkgbuilds =
            download_all(&conf, &conf.packages, self.force_rebuild, false).map_err(cmd_err)?;
        let builder = builder::Builder::new(&conf).map_err(cmd_err)?;
        for pkg in pkgbuilds {
            let name = &pkg.0 .0.name;
            let makepkg = conf
                .packages
                .get(name)
                .map(|p| p.makepkg.as_ref())
                .flatten();
            builder
                .download_src(&conf, name, makepkg)
                .map_err(cmd_err)?;
            patch(&conf, &pkg.0 .0).map_err(cmd_err)?;
            builder.build_pkg(conf, name, makepkg).map_err(cmd_err)?;
            repo::add(&conf, name).map_err(cmd_err)?;
        }
        Ok(())
    }
}
