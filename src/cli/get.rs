use clap::Args;
use log::error;

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

    /// Will not stop on error from the package or its depencies
    #[arg(long)]
    pub continue_on_error: bool,
}

impl CliCmd for Get {
    fn execute(&self, conf: &crate::Conf) -> Result<(), i32> {
        let pkgbuilds =
            download_pkg(&conf, &self.name, self.force_rebuild, true).map_err(cmd_err)?;
        if !builder::should_build(&pkgbuilds) {
            println!("Nothing to do :)");
            return Ok(());
        }
        let builder = builder::Builder::new(&conf).map_err(cmd_err)?;
        for pkgbuild in pkgbuilds {
            if pkgbuild.src == false {
                continue;
            }
            let makepkg = conf
                .packages
                .get(&pkgbuild.name)
                .map(|p| p.makepkg.as_ref())
                .flatten();
            if let Err(e) = builder
                .download_src(&conf, &pkgbuild.name, makepkg)
                .map_err(cmd_err)
            {
                if self.continue_on_error {
                    error!("[{}] Source download error: {}", pkgbuild.name, e);
                    continue;
                } else {
                    return Err(e);
                }
            }
            if let Err(e) = patch(&conf, &pkgbuild).map_err(cmd_err) {
                if self.continue_on_error {
                    error!("[{}] Patch error: {}", pkgbuild.name, e);
                    continue;
                } else {
                    return Err(e);
                }
            }
            if let Err(e) = builder
                .build_pkg(conf, &pkgbuild.name, makepkg)
                .map_err(cmd_err)
            {
                if self.continue_on_error {
                    error!("[{}] Build error: {}", pkgbuild.name, e);
                    continue;
                } else {
                    return Err(e);
                }
            }
            if let Err(e) = repo::add(&conf, &pkgbuild.name).map_err(cmd_err) {
                if self.continue_on_error {
                    error!("[{}] Database error: {}", pkgbuild.name, e);
                    continue;
                } else {
                    return Err(e);
                }
            }
        }
        Ok(())
    }
}
