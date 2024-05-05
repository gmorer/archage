use std::fmt::Display;

use clap::{Parser, Subcommand};

use crate::Conf;

mod build;
mod download;
mod get;
mod list;
mod update;

/*
TODO:
Patch:
# Start the creatation of a new patch, and cd into that dir
$> cabage patch start <pkg_name>
# Check if the patched sources can build
$> cabage patch build
# Save the patch
$> cabage patch finish

Clean:
$> cabage clean (<pkg_name>)
*/

pub fn cmd_err(e: impl Display) -> i32 {
    eprintln!("{}", e);
    2
}

pub trait CliCmd {
    fn execute(&self, conf: &Conf) -> Result<(), i32>;
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Cli {
    /// directory to load conf from, default is <DEFAULT>
    #[arg(short)]
    pub confdir: Option<String>,

    /// Rebuild packages even if there is no new versions
    #[arg(short)]
    pub force_rebuild: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Download and build a package
    Get(get::Get),
    /// Only download
    Download(download::Download),
    /// Only build
    Build(build::Build),
    /// List built packages
    List(list::List),
    /// Update packages
    Update(update::Update),
}

impl CliCmd for Commands {
    fn execute(&self, conf: &Conf) -> Result<(), i32> {
        match self {
            Commands::Get(a) => a.execute(&conf),
            Commands::Download(a) => a.execute(&conf),
            Commands::Build(a) => a.execute(&conf),
            Commands::List(a) => a.execute(&conf),
            Commands::Update(a) => a.execute(&conf),
        }
    }
}

impl Cli {
    pub fn get() -> Self {
        Self::parse()
    }
}

impl CliCmd for Cli {
    fn execute(&self, conf: &crate::Conf) -> Result<(), i32> {
        self.command.execute(conf)
    }
}
