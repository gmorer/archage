use serde::Deserialize;
use std::fs::read_to_string;
use std::path::PathBuf;
use std::string::String;
use std::vec::Vec;

const DEFAULT_CONF_LOCATION: &str = "/etc/archage/conf.toml";

fn default_repo() -> String {
    "https://gitlab.archlinux.org/archlinux/packaging/packages/".to_string()
}

fn default_server() -> PathBuf {
    PathBuf::from("/tmp/archage")
}

#[derive(Deserialize, Debug, Default)]
pub struct Conf {
    #[serde(default = "default_repo")]
    pub repo: String,

    #[serde(default = "default_server")]
    pub server_dir: PathBuf,

    // Server dir seen by the container runtime (ex. usage: podman-remote)
    pub host_server_dir: Option<PathBuf>,

    pub packages: Vec<String>,
    // TODO: container_runner: (podman, docker...)
}

impl Conf {
    pub fn new(conf_file: Option<&str>) -> Self {
        let f = read_to_string(conf_file.unwrap_or(DEFAULT_CONF_LOCATION)).unwrap();
        toml::from_str(&f).unwrap()
    }

    pub fn print(&self) {
        println!("Repo: {}", self.repo);
        println!("Server: {:?}", self.server_dir);
    }

    pub fn pkg_dir(&self, pkg: &str) -> PathBuf {
        let mut path = self.server_dir.clone();
        path.push(pkg);
        path
    }
}
