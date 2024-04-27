use serde::Deserialize;
use std::path::PathBuf;
use std::{collections::HashMap, fs::read_to_string};
use thiserror::Error;
use toml::{Table, Value};

const DEFAULT_CONF_LOCATION: &str = "/etc/pacage/conf.toml";

#[derive(Debug, Error)]
pub enum ConfError {
    #[error("System error: {0}")]
    Io(#[from] std::io::Error),
    #[error("parsing error: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("format error: {0}")]
    Format(String),
}

fn default_server() -> PathBuf {
    PathBuf::from("/tmp/archage")
}

#[derive(Deserialize, Debug, Default)]
pub struct Package {
    makepkg: Option<Makepkg>,
}

fn write_value(file: &mut String, key: &str, value: Option<&String>, def: Option<&String>) {
    if let Some(v) = value.or(def) {
        file.push_str(&format!("{}=\"{}\"\n", key, v));
    }
}

impl Package {
    pub fn get_makepkg(&self, conf: &Conf, name: &String) -> Result<String, std::io::Error> {
        let mut file = std::fs::read_to_string("/etc/makepkg.conf")?;
        let def = conf.makepkg.as_ref();
        let makepkg = self.makepkg.as_ref();
        file.push('\n');
        file.push_str(&format!("SRCDEST=/build/srcs/{}\n", name));
        file.push_str(&format!("SRCPKGDEST==/build/srcs/{}\n", name));
        // file.push_str(&format!("SRCDEST=/build/srcs/{}\n", name));
        // file.push_str("PKGDEST=/build/repo/\n");
        write_value(
            &mut file,
            "PACKAGER",
            makepkg.map(|a| a.packager.as_ref()).flatten(),
            def.map(|a| a.packager.as_ref()).flatten(),
        );
        write_value(
            &mut file,
            "MAKEFLAGS",
            makepkg.map(|a| a.makeflags.as_ref()).flatten(),
            def.map(|a| a.makeflags.as_ref()).flatten(),
        );
        write_value(
            &mut file,
            "CFLAGS",
            makepkg.map(|a| a.cflags.as_ref()).flatten(),
            def.map(|a| a.cflags.as_ref()).flatten(),
        );
        write_value(
            &mut file,
            "CXXFLAGS",
            makepkg.map(|a| a.cxxflags.as_ref()).flatten(),
            def.map(|a| a.cxxflags.as_ref()).flatten(),
        );
        write_value(
            &mut file,
            "RUSTFLAGS",
            makepkg.map(|a| a.rustflags.as_ref()).flatten(),
            def.map(|a| a.rustflags.as_ref()).flatten(),
        );
        write_value(
            &mut file,
            "LDFLAGS",
            makepkg.map(|a| a.ldflags.as_ref()).flatten(),
            def.map(|a| a.ldflags.as_ref()).flatten(),
        );
        write_value(
            &mut file,
            "LTOFLAGS",
            makepkg.map(|a| a.ltoflags.as_ref()).flatten(),
            def.map(|a| a.ltoflags.as_ref()).flatten(),
        );
        if self
            .makepkg
            .as_ref()
            .map(|c| c.ccache)
            .unwrap_or(Some(def.is_some_and(|d| d.ccache.is_some_and(|d| d))))
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

    pub packages: HashMap<String, Package>,
    // TODO: container_runner: (podman, docker...)
    pub makepkg: Option<Makepkg>,

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
        let g = f.parse::<Table>()?;
        let mut packages = HashMap::new();
        let container_runner = match g.get("container_runner") {
            None => "docker".to_string(),
            Some(Value::String(runner)) => runner.clone(),
            Some(a) => Err(ConfError::Format(format!(
                "Invalid \"container_runner\": {:?}",
                a
            )))?,
        };
        let server_dir = match g.get("server_dir") {
            None => default_server(),
            Some(Value::String(dir)) => PathBuf::from(dir),
            Some(a) => Err(ConfError::Format(format!(
                "Invalid \"server_dir\": {:?}",
                a
            )))?,
        };
        let host_server_dir = match g.get("host_server_dir") {
            None => None,
            Some(Value::String(dir)) => Some(PathBuf::from(dir)),
            Some(a) => Err(ConfError::Format(format!(
                "Invalid \"host_server_dir\": {:?}",
                a
            )))?,
        };
        let build_log_dir = match g.get("build_log_dir") {
            None => None,
            Some(Value::String(dir)) => Some(PathBuf::from(dir)),
            Some(a) => Err(ConfError::Format(format!(
                "Invalid \"build_log_dir\": {:?}",
                a
            )))?,
        };
        let makepkg: Option<Makepkg> = match g.get("makepkg") {
            None => None,
            Some(Value::Table(makepkg)) => Some(
                Value::Table(makepkg.clone())
                    .try_into()
                    .map_err(|e| ConfError::Format(format!("Failed to parse, makepkg: {}", e)))?,
            ),
            Some(a) => Err(ConfError::Format(format!("Invalid \"makepkg\": {:?}", a)))?,
        };
        for (name, v) in g {
            if name.as_str() != "makepkg" {
                if let Value::Table(t) = v {
                    match t.try_into::<Package>() {
                        Ok(p) => packages.insert(name, p),
                        Err(e) => Err(e)?,
                    };
                }
            }
        }
        Ok(Self {
            container_runner,
            server_dir,
            host_server_dir,
            makepkg,
            build_log_dir,
            packages,
        })
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

    pub fn get_repo(&self) -> PathBuf {
        let mut path = self.server_dir.clone();
        path.push("pacage.db.tar.gz");
        path
    }
}
