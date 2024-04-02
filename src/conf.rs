use serde::Deserialize;
use std::fs::read_to_string;
use std::path::PathBuf;
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
pub struct Package {
    pub name: String,
    makepkg: Option<Makepkg>,
}

fn write_value(file: &mut String, key: &str, value: Option<&String>, def: Option<&String>) {
    if let Some(v) = value.or(def) {
        file.push_str(&format!("{}=\"{}\"\n", key, v));
    }
}

impl Package {
    pub fn get_makepkg(&self, conf: &Conf) -> Result<String, std::io::Error> {
        let mut file = std::fs::read_to_string("/etc/makepkg.conf")?;
        let def = &conf.makepkg;
        let makepkg = self.makepkg.as_ref();
        file.push('\n');
        file.push_str(&format!("SRCDEST=/build/srcs/{}\n", self.name));
        file.push_str(&format!("SRCPKGDEST==/build/srcs/{}\n", self.name));
        // file.push_str(&format!("SRCDEST=/build/srcs/{}\n", self.name));
        // file.push_str("PKGDEST=/build/repo/\n");
        write_value(
            &mut file,
            "PACKAGER",
            makepkg.map(|a| a.packager.as_ref()).flatten(),
            def.packager.as_ref(),
        );
        write_value(
            &mut file,
            "MAKEFLAGS",
            makepkg.map(|a| a.makeflags.as_ref()).flatten(),
            def.makeflags.as_ref(),
        );
        write_value(
            &mut file,
            "CFLAGS",
            makepkg.map(|a| a.cflags.as_ref()).flatten(),
            def.cflags.as_ref(),
        );
        write_value(
            &mut file,
            "CXXFLAGS",
            makepkg.map(|a| a.cxxflags.as_ref()).flatten(),
            def.cxxflags.as_ref(),
        );
        write_value(
            &mut file,
            "RUSTFLAGS",
            makepkg.map(|a| a.rustflags.as_ref()).flatten(),
            def.rustflags.as_ref(),
        );
        write_value(
            &mut file,
            "LDFLAGS",
            makepkg.map(|a| a.ldflags.as_ref()).flatten(),
            def.ldflags.as_ref(),
        );
        write_value(
            &mut file,
            "LTOFLAGS",
            makepkg.map(|a| a.ltoflags.as_ref()).flatten(),
            def.ltoflags.as_ref(),
        );
        if self
            .makepkg
            .as_ref()
            .map(|c| c.ccache)
            .unwrap_or(def.ccache)
            .is_some_and(|a| a)
        {
            file.push_str("BUILDENV=(!distcc color ccache check !sign)");
        }
        Ok(file)
    }
}

#[derive(Deserialize, Debug, Default)]
pub struct Conf {
    pub container_runner: String,
    #[serde(default = "default_server")]
    pub server_dir: PathBuf,

    // Server dir seen by the container runtime (ex. usage: podman-remote)
    pub host_server_dir: Option<PathBuf>,

    pub packages: Vec<Package>,
    // TODO: container_runner: (podman, docker...)
    pub makepkg: Makepkg,

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
    pub ccache: Option<bool>,
}

impl Makepkg {}

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
