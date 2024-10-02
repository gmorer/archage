use clap::{command, Args, Subcommand};
use pacage::conf::Conf;

use crate::CliCmd;

#[derive(Args, Debug)]
pub struct Clean {
    #[command(subcommand)]
    what: What,

    /// Will ist changes without applying them
    #[arg(long, default_value_t = false)]
    dry_run: bool,
}

#[derive(Subcommand, Debug)]
pub enum What {
    /// Clear all unused packages
    Repo,
    /// Clear all the packages sources files
    Srcs,
    /// Clear all pakaghes logs
    Logs,

    All,
}

impl CliCmd for Clean {
    fn execute(&self, conf: Conf) -> Result<(), i32> {
        match self.what {
            What::Repo => clear_repo(conf),
            What::Srcs => clear_srcs(conf),
            What::Logs => clear_logs(conf),
            What::All => clear_all(conf),
        }
    }
}

fn clear_srcs(conf: Conf) -> Result<(), i32> {
    unimplemented!()
}
fn clear_repo(conf: Conf) -> Result<(), i32> {
    unimplemented!()
}
fn clear_logs(conf: Conf) -> Result<(), i32> {
    unimplemented!()
}
fn clear_all(conf: Conf) -> Result<(), i32> {
    unimplemented!()
}
