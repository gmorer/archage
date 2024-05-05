use crate::cli::CliCmd;
use clap::Args;

use crate::repo;

use super::cmd_err;

#[derive(Args, Debug)]
pub struct List {}

impl CliCmd for List {
    fn execute(&self, conf: &crate::Conf) -> Result<(), i32> {
        for p in repo::list(&conf).map_err(cmd_err)? {
            p.print();
        }
        Ok(())
    }
}
