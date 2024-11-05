use thiserror::Error;

mod db_desc;
mod pkginfo;
mod srcinfo;

pub use db_desc::{DbDesc, DbDescError};
pub use pkginfo::{PkgInfo, PkgInfoError};
pub use srcinfo::{SrcInfo, SrcInfoError};

#[derive(Debug, Error)]
pub enum ParsingError {
    #[error("System error while parsing .SRCINFO : {0}")]
    SrcInfo(#[from] SrcInfoError),
    #[error("System error while parsing .PKGINFO : {0}")]
    PkgInfo(#[from] PkgInfoError),
    // #[error("System error while parsing .SRCINFO : {0}")]
    // RepoPackage(#[from] io::Error),
    // #[error("System error while parsing .SRCINFO : {0}")]
    // PkgInfo(#[from] io::Error),
}
