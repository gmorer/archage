use crate::builder;
use crate::cli::{cmd_err, CliCmd};
use crate::download::download_pkg;
use clap::Args;

#[derive(Args, Debug)]
pub struct Download {
    /// Package name
    pub name: String,

    /// Will not stop on error from the package or its depencies
    #[arg(long)]
    pub continue_on_error: bool,

    /// Will not download packages src
    #[arg(long)]
    pub only_pkgbuild: bool,
}

impl CliCmd for Download {
    fn execute(&self, conf: &crate::Conf) -> Result<(), i32> {
        let pkgbuilds =
            download_pkg(&conf, &self.name, true, self.continue_on_error).map_err(cmd_err)?;
        if self.only_pkgbuild {
            println!("PKGBUILD downloaded");
            return Ok(());
        }
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
