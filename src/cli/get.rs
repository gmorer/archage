use clap::Args;

use crate::builder;
use crate::patch::patch;
use crate::{
    cli::{cmd_err, CliCmd},
    download::download_pkg,
    repo,
};

#[derive(Args, Debug)]
pub struct Get {
    /// Package name
    pub name: String,

    /// Rebuild packages even if there is no new versions
    #[arg(short)]
    pub force_rebuild: bool,

    /// Save the package in the mconf
    #[arg(long)]
    pub save: bool,
}

impl CliCmd for Get {
    fn execute(&self, conf: &crate::Conf) -> Result<(), i32> {
        let pkgbuild = download_pkg(&conf, &self.name, self.force_rebuild).map_err(cmd_err)?;
        let makepkg = conf
            .packages
            .get(&self.name)
            .map(|p| p.makepkg.as_ref())
            .flatten();
        let builder = builder::Builder::new(&conf).map_err(cmd_err)?;
        builder
            .download_src(&conf, &self.name, makepkg)
            .map_err(cmd_err)?;
        patch(&conf, &pkgbuild).map_err(cmd_err)?;
        builder
            .build_pkg(conf, &self.name, makepkg)
            .map_err(cmd_err)?;
        repo::add(&conf, &self.name).map_err(cmd_err)?;
        Ok(())
    }
}
