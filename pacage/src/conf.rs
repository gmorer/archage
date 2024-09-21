#[cfg(test)]
use bon::bon;
use log::{error, warn};
use serde::Deserialize;
use std::fs::{self, create_dir_all};
use std::path::{Path, PathBuf};
use std::{
    collections::{HashMap, HashSet},
    fs::read_to_string,
};
use thiserror::Error;
use toml::{Table, Value};

const DEFAULT_CONF_DIR: &str = "/etc/pacage";
const BUILD_SCRIPT_CONTENT: &str = std::include_str!("../../resources/build_pkg.sh");
pub(crate) const BUILD_SCRIPT_FILE: &str = "pacage_build.sh";

// pub const fn default_bool<const V: bool>() -> bool {
//     V
// }

fn default_name() -> String {
    "/".to_string()
}

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

#[derive(Deserialize, Debug)]
#[serde(try_from = "String")]
pub enum Repo {
    None,
    Aur,
    Git(String),
    File(String),
}

impl TryFrom<String> for Repo {
    type Error = String;

    fn try_from(mut value: String) -> Result<Self, Self::Error> {
        if value == "aur" {
            Ok(Self::Aur)
        } else if value.starts_with("https://") {
            Ok(Self::Git(value))
        } else if value.starts_with("file://") {
            value.drain(..7);
            Ok(Self::File(value))
        } else {
            Err("Invalid Repo should be \"aur\", \"https://...\" or \"file://...\"".to_string())
        }
    }
}
impl std::default::Default for Repo {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Deserialize, Debug)]
pub struct Package {
    // Name is set just after serialization
    #[serde(default = "default_name")]
    pub name: String,
    pub makepkg: Option<Makepkg>,
    pub deps: Option<bool>,
    #[serde(default)]
    pub repo: Repo,
}

impl std::hash::Hash for Package {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

impl std::cmp::PartialEq for Package {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}
impl std::cmp::Eq for Package {}

fn write_value(file: &mut String, key: &str, value: Option<&String>, def: Option<&String>) {
    if let Some(v) = value.or(def) {
        file.push_str(&format!("{}=\"{}\"\n", key, v));
    }
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

impl Makepkg {
    pub fn get_conf_file(
        conf: &Conf,
        makepkg: Option<&Makepkg>,
        name: &str,
    ) -> Result<String, std::io::Error> {
        // TODO: use env instead
        let mut file = std::fs::read_to_string("/etc/makepkg.conf")?;
        let def = conf.makepkg.as_ref();
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
        if makepkg
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

#[derive(Debug)]
pub struct Conf {
    pub container_runner: String,
    pub server_dir: PathBuf,
    pub host_server_dir: Option<PathBuf>,
    pub build_log_dir: Option<PathBuf>,
    // pub log_on_error: Option<bool>
    pub deps: bool,

    pub conf_dir: PathBuf,
    // Server dir seen by the container runtime (ex. usage: podman-remote)
    pub packages: HashSet<Package>,
    // TODO: container_runner: (podman, docker...)
    pub makepkg: Option<Makepkg>,

    // Never serialized.
    pub resolver: HashMap<String, String>,
}

#[cfg_attr(test, bon)]
impl Conf {
    const RESOLVE_FILE: &'static str = "resolve.toml";

    pub fn parse_resolver(conf_dir: &PathBuf) -> HashMap<String, String> {
        let mut res = HashMap::new();
        let resolver_path = conf_dir.join("resolve.toml");
        if !resolver_path.exists() {
            return res;
        }
        let f = match read_to_string(conf_dir.join(Self::RESOLVE_FILE)) {
            Ok(f) => f,
            Err(e) => {
                error!("Failed to read {}: {}", Self::RESOLVE_FILE, e);
                return res;
            }
        };
        let g = match f.parse::<Table>() {
            Ok(g) => g,
            Err(e) => {
                error!("Failed to parse {}: {}", Self::RESOLVE_FILE, e);
                return res;
            }
        };
        for (k, v) in g.into_iter() {
            if let Some(v) = v.as_str() {
                res.insert(k.to_string(), v.to_string());
            } else {
                warn!("Invalid value in {}, {} -> {:?}", Self::RESOLVE_FILE, k, v);
            }
        }
        res
    }
    pub fn new(conf_dir: Option<&str>) -> Result<Self, ConfError> {
        // TODO: full dir from root
        let conf_dir = match fs::canonicalize(conf_dir.unwrap_or(DEFAULT_CONF_DIR)) {
            Ok(p) => p,
            Err(e) => Err(ConfError::Format(format!(
                "Failed to parse conf dir: {}",
                e
            )))?,
        };
        let f = read_to_string(conf_dir.join("pacage.toml"))?;
        let g = f.parse::<Table>()?;
        let mut packages = HashSet::new();
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
        let deps = match g.get("deps") {
            None => false,
            Some(Value::Boolean(deps)) => *deps,
            Some(a) => Err(ConfError::Format(format!("Invalid \"deps\": {:?}", a)))?,
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
                        Ok(mut p) => {
                            p.name = name.to_string();
                            packages.insert(p)
                        }
                        Err(e) => Err(e)?,
                    };
                }
            }
        }
        let resolver = Self::parse_resolver(&conf_dir);
        Ok(Self {
            resolver,
            container_runner,
            server_dir,
            conf_dir,
            host_server_dir,
            makepkg,
            build_log_dir,
            deps,
            packages,
        })
    }

    // Directory containing the pkgbuild
    pub fn pkg_dir(&self, pkg: &str) -> PathBuf {
        self.server_dir.join("pkgs").join(pkg)
    }

    // Directory containing the package sources
    pub fn pkg_src(&self, pkg: &str) -> PathBuf {
        self.server_dir.join("srcs").join(pkg)
    }

    pub fn get_repo_db(&self) -> PathBuf {
        self.server_dir.join("repo").join("pacage.db.tar.gz")
    }

    pub fn get_repo_files_db(&self) -> PathBuf {
        self.server_dir.join("repo").join("pacage.files.tar.gz")
    }

    pub fn remove_src(&self, pkg: &str) {
        if let Err(e) = fs::remove_dir_all(self.pkg_src(pkg)) {
            error!("[{}] could not remove src dir: {}", pkg, e);
        }
    }

    pub fn need_deps(&self, pkg: &Package) -> bool {
        return pkg.deps.unwrap_or(self.deps);
    }

    pub fn ensure_pkg(&mut self, name: &str) {
        let name = self.resolver.get(name).map(|a| a.as_str()).unwrap_or(name);
        if self.packages.iter().find(|p| p.name == name).is_some() {
            return;
        }
        let new = Package {
            name: name.to_string(),
            makepkg: None,
            deps: None,
            repo: Repo::None,
        };
        self.packages.insert(new);
    }

    // Name should not be used after this call, but pkg.name
    pub fn get(&self, name: String) -> &Package {
        let name = self
            .resolver
            .get(name.as_str())
            .map(|a| a.as_str())
            .unwrap_or(name.as_str());
        self.packages.iter().find(|p| p.name == name).expect("aa")
    }

    pub fn init(&self) -> Result<(), String> {
        create_dir_all(&self.server_dir)
            .map_err(|e| format!("Failed to create server dir: {}", e))?;
        let pkgs_dir = self.server_dir.join("pkgs");
        create_dir_all(&pkgs_dir).map_err(|e| format!("Failed to create pkgs dir: {}", e))?;
        let srcs_dir = self.server_dir.join("srcs");
        create_dir_all(&srcs_dir).map_err(|e| format!("Failed to create srcs dir: {}", e))?;
        if let Some(build_log_dir) = &self.build_log_dir {
            create_dir_all(build_log_dir)
                .map_err(|e| format!("Failed to create log dir: {}", e))?;
        }
        create_dir_all(self.server_dir.join("repo"))
            .map_err(|e| format!("Failed to create repo dir: {}", e))?;
        create_dir_all(self.server_dir.join("cache").join("pacman"))
            .map_err(|e| format!("Failed to create cache dir: {}", e))?;
        if self
            .makepkg
            .as_ref()
            .is_some_and(|makepkg| makepkg.ccache.is_some_and(|a| a))
        {
            create_dir_all(self.server_dir.join("cache").join("ccache"))
                .map_err(|e| format!("Failed to create ccache dir: {}", e))?;
        }
        fs::write(
            Path::new(&self.server_dir).join(BUILD_SCRIPT_FILE),
            BUILD_SCRIPT_CONTENT,
        )
        .map_err(|e| format!("Failed to write build script: {}", e))?;
        Ok(())
    }

    #[cfg(test)]
    #[builder]
    pub fn _test_builder(
        server_dir: PathBuf,
        conf_dir: Option<PathBuf>,
        deps: Option<bool>,
        resolver: Option<HashMap<String, String>>,
    ) -> Self {
        use crate::utils::copy_dir::copy_dir;

        let _ = env_logger::builder().is_test(true).try_init();
        let tmp_server_dir = tests::mktemp();
        copy_dir(server_dir, &tmp_server_dir).unwrap();
        Self {
            container_runner: "podman-remote".to_string(),
            server_dir: tmp_server_dir,
            host_server_dir: None,
            build_log_dir: None,
            // pub log_on_error: Option<bool>
            deps: deps.unwrap_or(false),
            conf_dir: conf_dir.unwrap_or("".into()),
            // Server dir seen by the container runtime (ex. usage: podman-remote)
            packages: HashSet::new(),
            // TODO: container_runner: (podman, docker...)
            makepkg: None,

            // Never serialized.
            resolver: resolver.unwrap_or(HashMap::new()),
        }
    }

    // TODO: fix rust polonius
    // pub fn get_or_insert(&mut self, name: &str) -> &Package {
    //     if let Some(p) = self.packages.iter().find(|p| p.name == name) {
    //         return p;
    //     }
    //     let new = Package {
    //         name: name.to_string(),
    //         makepkg: None,
    //         deps: None,
    //         repo: Repo::None,
    //     };
    //     // TODO(nightly): https://doc.rust-lang.org/stable/std/collections/struct.HashSet.html#method.get_or_insert
    //     self.packages.insert(new);
    //     return self.packages.iter().find(|p| p.name == name).unwrap();
    // }

    #[cfg(test)]
    pub fn rand() -> Self {
        use std::env;

        Self {
            container_runner: "dunno".to_string(),
            server_dir: env::temp_dir(),
            host_server_dir: None,
            build_log_dir: None,
            deps: false,
            conf_dir: PathBuf::from("."),
            packages: HashSet::new(),
            makepkg: None,
            resolver: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    use rand::{self, distributions::Alphanumeric, Rng};

    pub fn mktemp() -> PathBuf {
        const BASE: &str = "pacage";
        let key: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(7)
            .map(char::from)
            .collect();
        let tmp_server_dir = env::temp_dir().join(format!("{}-{}", BASE, key));
        if tmp_server_dir.exists() {
            mktemp()
        } else {
            fs::create_dir(&tmp_server_dir).unwrap();
            tmp_server_dir
        }
    }
}
