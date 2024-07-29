use thiserror::Error;

mod db_desc;
mod srcinfo;

pub use db_desc::{DbDesc, DbDescError};
pub use srcinfo::{SrcInfo, SrcInfoError};

#[derive(Debug, Error)]
pub enum ParsingError {
    #[error("System errro while parsing .SRCINFO : {0}")]
    SrcInfo(#[from] SrcInfoError),
    // #[error("System errro while parsing .SRCINFO : {0}")]
    // RepoPackage(#[from] io::Error),
    // #[error("System errro while parsing .SRCINFO : {0}")]
    // PkgInfo(#[from] io::Error),
}
