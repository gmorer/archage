use clap::Parser;

/*
Get:
# Will download and build a package
$> cabage get <pkg_name>

Download
# Will download pkg sources
$> cabage download <pkg_name>

Build
# Will (re)build a pkg
$> cabage build <pkg_name>

List
# Will list built packages
$> cabage list
// Could be built or downloaded

Update
# Will download latest for every build packages and build them
$> cabage update (<pkg_name>)

Patch:
# Start the creatation of a new patch, and cd into that dir
$> cabage patch start <pkg_name>
# Check if the patched sources can build
$> cabage patch build
# Save the patch
$> cabage patch finish
*/

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// directory to load conf from, default is <DEFAULT>
    #[arg(short)]
    pub confdir: Option<String>,

    /// Rebuild packages even if there is no new versions
    #[arg(long)]
    pub force_rebuild: bool,

    /// Only build package that have been previously downloaded
    #[arg(long)]
    pub skip_download: bool,

    /// List build packages
    #[arg(long)]
    pub list_pkgs: bool,
}

impl Args {
    pub fn get() -> Self {
        Self::parse()
    }
}
