use std::cmp::max;
use std::collections::HashMap;
use std::fs::read_dir;

use crate::cli::CliCmd;
use crate::format::{DbDesc, SrcInfo};
use clap::Args;

use crate::db;

use super::cmd_err;

#[derive(Args, Debug)]
pub struct Status {
    /// Pull repositories to check for update
    #[arg(long)]
    pub pull: bool,
}

type StatusPkg = (Option<SrcInfo>, Option<DbDesc>);

impl CliCmd for Status {
    fn execute(&self, conf: crate::Conf) -> Result<(), i32> {
        if self.pull {
            unimplemented!();
        }
        let mut name_max_len = 0;
        let mut version_max_len = 0;
        let mut res: HashMap<String, StatusPkg> = HashMap::new();
        for file in read_dir(conf.server_dir.join("pkgs")).map_err(cmd_err)? {
            if let Ok(file) = file {
                if let Ok(typ) = file.file_type() {
                    if typ.is_dir() {
                        let name = file.file_name();
                        let name = name.to_string_lossy();
                        let pkg = SrcInfo::new(&conf, name.as_ref()).map_err(cmd_err)?;
                        name_max_len = max(name_max_len, pkg.name.len());
                        version_max_len = max(version_max_len, pkg.version.len());
                        res.insert(pkg.name.clone(), (Some(pkg), None));
                    }
                }
            }
        }
        for p in db::list(&conf).map_err(cmd_err)? {
            if let Some((_, ref mut pkg)) = res.get_mut(&p.name) {
                name_max_len = max(name_max_len, p.name.len());
                version_max_len = max(version_max_len, p.version.len());
                *pkg = Some(p);
            } else {
                res.insert(p.name.clone(), (None, Some(p)));
            }
        }
        for pkg in &conf.packages {
            name_max_len = max(name_max_len, pkg.name.len());
        }
        let max_len = name_max_len + version_max_len + 2;
        for pkg in &conf.packages {
            let name = &pkg.name;
            if let Some(pkg) = res.remove(name) {
                match pkg {
                    (Some(src), Some(db)) => {
                        if src.version != db.version {
                            println!(
                                "{:width$} outdated new version: {}",
                                format!("{}({})", name, db.version),
                                src.version,
                                width = max_len,
                            );
                        } else {
                            println!(
                                "{:width$} Built!",
                                format!("{}({})", name, db.version),
                                width = max_len
                            );
                        }
                    }
                    (Some(src), None) => {
                        println!(
                            "{:width$} Downloaded, not built",
                            format!("{}({})", name, src.version),
                            width = max_len
                        );
                        // With src not installed
                    }
                    (None, Some(db)) => {
                        println!(
                            "{:width$} Built missing src",
                            format!("{}({})", name, db.version),
                            width = max_len
                        );
                        // Installed no src
                    }
                    _ => {}
                }
            } else {
                println!("{:1$} Not downloaded/built", name, max_len);
            }
        }
        for (name, (src, repo)) in res {
            match (src, repo) {
                (Some(src), Some(db)) => {
                    if src.version != db.version {
                        println!(
                            "{:width$} outdated new version: {} (not in conf)",
                            format!("{}({})", name, db.version),
                            src.version,
                            width = max_len,
                        );
                    } else {
                        println!(
                            "{:width$} Built! (not in conf)",
                            format!("{}({})", name, db.version),
                            width = max_len
                        );
                    }
                }
                (Some(src), None) => {
                    println!(
                        "{:width$} Downloaded, not built (not in conf)",
                        format!("{}({})", name, src.version),
                        width = max_len
                    );
                    // With src not installed
                }
                (None, Some(db)) => {
                    println!(
                        "{:width$} Built missing src (not in conf)",
                        format!("{}({})", name, db.version),
                        width = max_len
                    );
                    // Installed no src
                }
                _ => {}
            }
        }
        Ok(())
    }
}
