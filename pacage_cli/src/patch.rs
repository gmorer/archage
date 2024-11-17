use pacage::utils::copy_dir::copy_dir;
use std::{
    env,
    fs::{self, File},
    io::Write,
    path::{Component, PathBuf},
};

use clap::{Args, Subcommand};

use crate::cmd_err;
use pacage::{
    builder::Builder,
    cmd::{command, NOENV},
    conf::Conf,
    download::fetch_pkg,
    format::SrcInfo,
    patch::{find_src, get_patches, patch_dir},
};

use super::CliCmd;

#[derive(Subcommand, Debug)]
pub enum Patch {
    /// Show the diff
    Diff(Diff),
    /// Save the patch
    Save(Save),
    /// Create/Open a patch for a project
    Open(Open),
}

#[derive(Args, Debug)]
pub struct Open {
    /// Package name
    pub name: String,

    /// Overwite all previous patches
    #[arg(long)]
    pub overwrite: bool,
}
impl CliCmd for Open {
    fn execute(&self, mut conf: Conf) -> Result<(), i32> {
        let name = self.name.clone();
        conf.ensure_pkg(&name);
        let pkg = conf.get(name.as_str());
        let pkgsdir = conf.pkgs_dir();
        let srcinfo = if !conf.pkg_dir(&pkg.name).exists() {
            fetch_pkg(&pkgsdir, &pkg.name, &pkg.repo).map_err(cmd_err)?
        } else {
            SrcInfo::new(&pkgsdir, &pkg.name, false).map_err(cmd_err)?
        };
        if srcinfo.src == false {
            eprintln!("The package doesnt contain sources");
            return Err(2);
        }
        // TODO: maybe if orig and patched dir exist we dont download/cleanup and just cd into it
        let builder = Builder::new(
            &conf.server_dir,
            conf.container_runner.clone(),
            &conf.host_server_dir,
            &conf.build_log_dir,
        )
        .map_err(cmd_err)?;
        let srcinfo = builder
            .download_src(&conf, srcinfo, pkg.makepkg.as_ref())
            .map_err(cmd_err)?;
        drop(builder);
        let Some(orig) = find_src(&conf, &srcinfo) else {
            eprintln!("Failed to find packages sources for {}", pkg.name);
            return Err(2);
        };
        let new = get_patched_dir(&orig)?;
        if let Ok(current_dir) = env::current_dir() {
            if current_dir.starts_with(&new) {
                eprintln!("The user is in the patched directory, cannot clean it");
                return Err(2);
            }
        }
        if new.exists() {
            if let Err(e) = fs::remove_dir_all(&new) {
                eprintln!("Failed to cleaning up old patch dir: {}", e);
                return Err(2);
            }
        }
        // use cp ?
        if let Err(e) = copy_dir(orig, &new) {
            eprintln!("Failed to copying package sources: {}", e);
            return Err(2);
        }
        if !self.overwrite {
            if let Some(patches) = get_patches(&conf, &pkg.name).map_err(cmd_err)? {
                if !patches.is_empty() {
                    if let Err(e) = patch_dir(&conf, &new, &pkg.name, patches) {
                        eprintln!("Failed to use previous patch: {}", e);
                        return Err(2);
                    }
                }
            }
        }
        Ok(())
    }
}

#[derive(Args, Debug)]
pub struct Save {
    /// Package name
    pub name: Option<String>,
}
impl CliCmd for Save {
    fn execute(&self, conf: Conf) -> Result<(), i32> {
        let (diff, name) = get_diff(&conf, self.name.as_ref())?;
        let patch_path = conf.conf_dir.join("patchs").join(&name);
        // TODO(improvment): write the patch then delete file that were there (atomic)
        if patch_path.exists() {
            if let Err(e) = fs::remove_dir_all(&patch_path) {
                eprintln!("Failed to remove old patches dir: {}", e);
                return Err(2);
            }
        }
        if let Err(e) = fs::create_dir_all(&patch_path) {
            eprintln!("Failed to create the new patch dir: {}", e);
            return Err(2);
        }

        let patch_path = patch_path.join("pacage.patch");
        let mut file = File::create(&patch_path).map_err(cmd_err)?;

        file.write_all(&diff.as_bytes()).map_err(cmd_err)?;
        // TODO: maybe dont do that
        // if let Err(e) = fs::remove_dir_all(conf.server_dir.join("srcs").join(name)) {
        // eprintln!("Fail to remove source directory: {}", e);
        // }
        println!("Patch write to {}", patch_path.to_string_lossy());
        Ok(())
    }
}

#[derive(Args, Debug)]
pub struct Diff {
    /// Package name
    pub name: Option<String>,
}
impl CliCmd for Diff {
    fn execute(&self, conf: Conf) -> Result<(), i32> {
        let (diff, _) = get_diff(&conf, self.name.as_ref())?;
        println!("{}", diff);
        Ok(())
    }
}

impl CliCmd for Patch {
    fn execute(&self, conf: Conf) -> Result<(), i32> {
        match self {
            Self::Diff(s) => s.execute(conf),
            Self::Save(s) => s.execute(conf),
            Self::Open(o) => o.execute(conf),
        }
    }
}

fn get_diff(
    conf: &Conf,
    name: Option<&String>,
) -> Result<(String /* diff */, String /* pkg name */), i32> {
    let name = if let Some(name) = name {
        name.clone()
    } else {
        get_pwd_pkg(&conf)?
    };
    let srcinfo = SrcInfo::new(&conf.pkgs_dir(), &name, false).map_err(cmd_err)?;
    let Some(mut orig_path) = find_src(&conf, &srcinfo) else {
        eprintln!("Failed to find packages sources for {}", name);
        return Err(2);
    };
    let new_path = get_patched_dir(&orig_path)?;
    let Some(new_dir) = new_path.file_name() else {
        eprintln!(
            "Failed to find patched src dir name for {}.patched",
            orig_path.to_string_lossy()
        );
        return Err(2);
    };
    let Some(orig_dir) = orig_path.file_name() else {
        eprintln!(
            "Failed to find src dir name for {}",
            orig_path.to_string_lossy()
        );
        return Err(2);
    };
    let orig_dir = orig_dir.to_string_lossy().to_string();
    let new = new_dir.to_string_lossy();
    orig_path.pop();
    println!("dir: {:?}", &orig_path);
    let (status, cmd, _) = match command(
        &["diff", "--no-dereference", "-ruN", &orig_dir, &new],
        &orig_path,
        NOENV,
    ) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to execute the diff command: {}", e);
            return Err(2);
        }
    };
    // Diff return 1 if it differ and 2 if error
    match status.code() {
        Some(0) | Some(1) => Ok((cmd.join("\n"), name)),
        _ => {
            eprintln!("Failed to excute the diff:\n{} ", cmd.join("\n"));
            Err(2)
        }
    }
}

fn get_pwd_pkg(conf: &Conf) -> Result<String, i32> {
    let current_dir = match env::current_dir() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Fail to get current directory: {}", e);
            return Err(2);
        }
    };
    let srcs = conf.server_dir.join("srcs");
    let Ok(path) = current_dir.strip_prefix(srcs) else {
        eprintln!("You should be in the patched directory to use this command, or spicify <NAME>");
        return Err(2);
    };
    let Some(name) = path.components().next() else {
        eprintln!("You should be in the patched directory to use this command, or spicify <NAME>");
        return Err(2);
    };
    let Component::Normal(name) = name else {
        eprintln!("You should be in the patched directory to use this command, or spicify <NAME>");
        return Err(2);
    };
    Ok(name.to_string_lossy().to_string())
}

fn get_patched_dir(orig: &PathBuf) -> Result<PathBuf, i32> {
    let Some(dirname) = orig.file_name() else {
        eprintln!("Invalid src dir name: {}", orig.to_string_lossy());
        return Err(2);
    };
    let mut dest = orig.clone();
    dest.pop();
    Ok(dest.join(format!("{}.patched", dirname.to_string_lossy())))
}
