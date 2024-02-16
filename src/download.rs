use rayon::prelude::*;
use std::{fs, process::Command};

use crate::Conf;

pub fn download_all(conf: &Conf) {
    rayon::ThreadPoolBuilder::new()
        .num_threads(4)
        .build()
        .unwrap()
        .install(|| {
            conf.packages.par_iter().for_each(|pkg| {
                let pkg_dir = conf.pkg_dir(pkg);
                let pkg_dir = pkg_dir.to_str().unwrap();
                fs::remove_dir_all(pkg_dir).ok(/* Would fail if doesnt exist */);
                let pkg_url = format!("{}/{}", conf.repo, pkg);
                let output = Command::new("git")
                    .current_dir(&conf.server_dir)
                    .args(["clone", &pkg_url, pkg_dir])
                    .output()
                    .unwrap();
                if !output.status.success() {
                    eprintln!(
                        "Failed to download {}: \n{}",
                        pkg,
                        String::from_utf8_lossy(&output.stderr)
                    )
                }
            });
        });
}
