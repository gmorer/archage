use clap::Args;

use crate::builder;
use crate::cli::{cmd_err, CliCmd};
use crate::db;
use crate::download::SrcInfo;
use crate::patch::patch;

#[derive(Args, Debug)]
pub struct Build {
    /// Package name
    pub name: String,
}

impl CliCmd for Build {
    fn execute(&self, mut conf: crate::Conf) -> Result<(), i32> {
        let pkg_build = SrcInfo::new(&conf, &self.name).map_err(cmd_err)?;
        if !conf.pkg_src(&self.name).exists() {
            Err(cmd_err(format!(
                "Missing packages sources, run 'pacage download {}' to get them",
                self.name
            )))?;
        }
        let builder = builder::Builder::new(&conf).map_err(cmd_err)?;
        patch(&conf, &pkg_build).map_err(cmd_err)?;
        conf.ensure_pkg(&self.name);
        let pkg = conf.get(&self.name);
        builder
            .build_pkg(&conf, pkg)
            // .build_pkg(conf, &self.name, makepkg)
            .map_err(cmd_err)?;
        db::add(&conf, &self.name).map_err(cmd_err)?;
        Ok(())
    }
}
