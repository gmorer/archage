use clap::{command, Args, Parser, Subcommand};
use log::{error, LevelFilter};
use std::fmt::Display;

use pacage::conf::Conf;

mod build;
mod download;
mod get;
mod patch;
mod status;
mod update;

pub fn cmd_err(e: impl Display) -> i32 {
    eprintln!("{}", e);
    2
}

pub trait CliCmd {
    fn execute(&self, conf: Conf) -> Result<(), i32>;
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
    /// Update packages
    Update(update::Update),
    /// Check status
    Status(status::Status),
    /// Patch utilities
    #[command(subcommand)]
    Patch(patch::Patch),
}

#[derive(Args, Debug)]
pub struct RepoAdd {
    /// Package name
    pub name: String,
}

impl CliCmd for Commands {
    fn execute(&self, conf: Conf) -> Result<(), i32> {
        match self {
            Commands::Get(a) => a.execute(conf),
            Commands::Download(a) => a.execute(conf),
            Commands::Build(a) => a.execute(conf),
            Commands::Update(a) => a.execute(conf),
            Commands::Status(a) => a.execute(conf),
            Commands::Patch(a) => a.execute(conf),
        }
    }
}

impl Cli {
    pub fn get() -> Self {
        Self::parse()
    }
}

impl CliCmd for Cli {
    fn execute(&self, conf: crate::Conf) -> Result<(), i32> {
        self.command.execute(conf)
    }
}

fn main() {
    env_logger::builder().filter_level(LevelFilter::Info).init();
    let args = Cli::get();
    let conf = match Conf::new(args.confdir.as_deref()) {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to create conf: {}", e);
            std::process::exit(2);
        }
    };
    if let Err(e) = conf.init() {
        error!("Failed to init: {}", e);
        std::process::exit(2);
    }

    if let Err(e) = args.execute(conf) {
        std::process::exit(e)
    }

    /*
    info!("Downloading packages...");
    let to_build = if !args.skip_download {
        download::download_all(&conf, &conf.packages, args.force_rebuild)
    } else {
        // Only packages present on the file system
        let mut pkgs = HashSet::<PkgBuildWithMakePkg>::new();
        for (name, pkg) in conf.packages.iter() {
            if conf.pkg_dir(name.as_str()).exists() {
                match PkgBuild::new(&conf, &name) {
                    Ok(p) => {
                        pkgs.insert(PkgBuildWithMakePkg((p, pkg.makepkg.as_ref())));
                    }
                    Err(e) => {
                        error!("[{}] Failed to read pkgbuild: {}", name, e)
                    }
                }
            }
        }
        pkgs
    };
    info!("Building packages...");
    match build::build(&conf, to_build) {
        Ok(built) => {
            info!("Adding packages...");
            repo::add_all(&conf, built);
        }
        Err(e) => {
            error!("Failed to build packages: {}", e);
        }
    }
    */
}
