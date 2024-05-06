use crate::builder;
use crate::cli::{cmd_err, CliCmd};
use crate::download::download_pkg;
use clap::Args;

#[derive(Args, Debug)]
pub struct Download {
    /// Package name
    pub name: String,
}

impl CliCmd for Download {
    fn execute(&self, conf: &crate::Conf) -> Result<(), i32> {
        let pkgbuilds = download_pkg(&conf, &self.name, true).map_err(cmd_err)?;
        if !builder::should_build(&pkgbuilds) {
            println!("Nothing to do :)");
            return Ok(());
        }
        let makepkg = conf
            .packages
            .get(&self.name)
            .map(|p| p.makepkg.as_ref())
            .flatten();
        let builder = builder::Builder::new(&conf).map_err(cmd_err)?;
        for pkgbuild in pkgbuilds {
            builder
                .download_src(&conf, &pkgbuild.name, makepkg)
                .map_err(cmd_err)?;
            println!("{} - {} downloaded", pkgbuild.name, pkgbuild.version);
        }
        Ok(())
    }
}
