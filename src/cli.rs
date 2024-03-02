use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// where to load conf from, default is <DEFAULT>
    #[arg(short)]
    pub conffile: Option<String>,

    /// Rebuild packages even if there is no new versions
    #[arg(long)]
    pub force_rebuild: bool,

    /// Only build package that have been previously downloaded
    #[arg(long)]
    pub skip_download: bool,
}

impl Args {
    pub fn get() -> Self {
        Self::parse()
    }
}
