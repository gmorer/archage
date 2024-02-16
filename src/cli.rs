use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// where to load conf from, default is <DEFAULT>
    #[arg(short)]
    pub conf: Option<String>,

    /// Only build package that have been previously downloaded
    #[arg(long)]
    pub skip_download: bool,
}

impl Args {
    pub fn get() -> Self {
        Self::parse()
    }
}
