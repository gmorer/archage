use serde::Deserialize;
use std::fs::read_to_string;
use std::path::PathBuf;
use std::string::String;
use std::vec::Vec;
use thiserror::Error;

const DEFAULT_CONF_LOCATION: &str = "/etc/pacage/conf.toml";

#[derive(Debug, Error)]
pub enum ConfError {
    #[error("System error: {0}")]
    Io(#[from] std::io::Error),
    #[error("parsing error: {0}")]
    Parse(#[from] toml::de::Error),
}

fn default_server() -> PathBuf {
    PathBuf::from("/tmp/archage")
}

#[derive(Deserialize, Debug, Default)]
pub struct Conf {
    #[serde(default = "default_server")]
    pub server_dir: PathBuf,

    // Server dir seen by the container runtime (ex. usage: podman-remote)
    pub host_server_dir: Option<PathBuf>,

    pub packages: Vec<String>,
    // TODO: container_runner: (podman, docker...)
    pub makepkg: Makepkg,

    // TODO
    pub build_log_dir: Option<PathBuf>,
}

#[derive(Deserialize, Debug, Default)]
pub struct Makepkg {
    packager: Option<String>,
    cflags: Option<String>,
    cxxflags: Option<String>,
    rustflags: Option<String>,
    makeflags: Option<String>,
    ldflags: Option<String>,
    ltoflags: Option<String>,
}

impl Makepkg {
    pub fn to_file(&self) -> Result<String, std::io::Error> {
        let mut file = std::fs::read_to_string("/etc/makepkg.conf")?;
        file.push('\n');
        if let Some(packager) = &self.packager {
            // TODO: verify packager name
            file.push_str(&format!("PACKAGER=\"{}\"\n", packager));
        }
        if let Some(cflags) = &self.cflags {
            file.push_str(&format!("CFLAGS=\"{}\"\n", cflags));
        }
        if let Some(cxxflags) = &self.cxxflags {
            file.push_str(&format!("CXXFLAGS=\"{}\"\n", cxxflags));
        }
        if let Some(rustflags) = &self.rustflags {
            file.push_str(&format!("RUSTFLAGS=\"{}\"\n", rustflags));
        }
        if let Some(makeflags) = &self.makeflags {
            file.push_str(&format!("MAKEFLAGS=\"{}\"\n", makeflags));
        }
        if let Some(ldflags) = &self.ldflags {
            file.push_str(&format!("LDFLAGS=\"{}\"\n", ldflags));
        }
        if let Some(ltoflags) = &self.ltoflags {
            file.push_str(&format!("LTOFLAGS=\"{}\"\n", ltoflags));
        }
        Ok(file)
    }
}

impl Conf {
    pub fn new(conf_file: Option<&str>) -> Result<Self, ConfError> {
        let f = read_to_string(conf_file.unwrap_or(DEFAULT_CONF_LOCATION))?;
        Ok(toml::from_str(&f)?)
    }

    pub fn print(&self) {
        println!("Server: {:?}", self.server_dir);
    }

    pub fn pkg_dir(&self, pkg: &str) -> PathBuf {
        let mut path = self.server_dir.clone();
        path.push("pkgs");
        path.push(pkg);
        path
    }
}
