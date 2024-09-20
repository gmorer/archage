use crate::{cmd_err, CliCmd};
use clap::Args;
use pacage::builder;
use pacage::download::download_pkg;

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
    fn execute(&self, mut conf: crate::Conf) -> Result<(), i32> {
        let pkgbuilds =
            download_pkg(&mut conf, &self.name, self.continue_on_error).map_err(cmd_err)?;
        if self.only_pkgbuild {
            println!("PKGBUILD downloaded");
            return Ok(());
        }
        if !builder::should_build(&pkgbuilds) {
            println!("Nothing to do :)");
            return Ok(());
        }
        conf.ensure_pkg(&self.name);
        let pkg = conf.get(self.name.clone());
        let makepkg = pkg.makepkg.as_ref();
        let builder = builder::Builder::new(&conf).map_err(cmd_err)?;
        for pkgbuild in pkgbuilds {
            builder
                .download_src(&conf, &pkgbuild.name, makepkg)
                .map_err(cmd_err)?;
            println!("{} - {} downloaded", pkgbuild.name, pkgbuild.pkgver);
        }
        Ok(())
    }
}
