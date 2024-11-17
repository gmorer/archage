use std::collections::BTreeSet;

use clap::Args;
use crossbeam_channel::unbounded;
use log::info;
use pacage::conf::Package;
use pacage::format::SrcInfo;

use crate::util::dl_and_build;
use crate::{cmd_err, CliCmd};
use pacage::builder::Builder;
use pacage::download::download_all;

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
    fn execute(&self, mut conf: crate::Conf) -> Result<(), i32> {
        let (pkgbuildssender, pkgbuilds) = unbounded::<(SrcInfo, Package)>();
        let mut to_dl = BTreeSet::new();
        to_dl.insert(self.name.clone());

        let builder_recv = Builder::new_async(&conf);
        download_all(&mut conf, to_dl, true, pkgbuildssender).map_err(cmd_err)?;
        let num = dl_and_build(&conf, pkgbuilds, builder_recv, true).map_err(cmd_err)?;
        info!("Added {} packages(s)", num);
        Ok(())
        // let pkgbuilds = download_pkg(&mut conf, &self.name, true).map_err(cmd_err)?;
        // if !builder::should_build(&pkgbuilds) {
        //     println!("Nothing to do :)");
        //     return Ok(());
        // }
        // let builder = builder::Builder::new(&conf).map_err(cmd_err)?;
        // for pkgbuild in pkgbuilds {
        //     if pkgbuild.src == false {
        //         continue;
        //     }
        //     conf.ensure_pkg(&pkgbuild.name);
        //     let pkg = conf.get(pkgbuild.name.clone());
        //     let makepkg = pkg.makepkg.as_ref();
        //     if let Err(e) = builder
        //         .download_src(&conf, &pkg.name, makepkg)
        //         .map_err(cmd_err)
        //     {
        //         if self.continue_on_error {
        //             error!("[{}] Source download error: {}", pkg.name, e);
        //             continue;
        //         } else {
        //             return Err(e);
        //         }
        //     }
        //     if let Err(e) = patch(&conf, &pkgbuild).map_err(cmd_err) {
        //         if self.continue_on_error {
        //             error!("[{}] Patch error: {}", pkg.name, e);
        //             continue;
        //         } else {
        //             return Err(e);
        //         }
        //     }
        //     if let Err(e) = builder.build_pkg(&conf, pkg).map_err(cmd_err) {
        //         if self.continue_on_error {
        //             error!("[{}] Build error: {}", pkg.name, e);
        //             continue;
        //         } else {
        //             return Err(e);
        //         }
        //     }
        //     if let Err(e) = db::add(&conf, &[pkgbuild]).map_err(cmd_err) {
        //         if self.continue_on_error {
        //             error!("[{}] Database error: {}", pkg.name, e);
        //             continue;
        //         } else {
        //             return Err(e);
        //         }
        //     }
        // }
        // Ok(())
    }
}
