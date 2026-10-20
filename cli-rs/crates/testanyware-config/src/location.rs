//! Where the file is, and reading it.

use std::io;
use std::path::{Path, PathBuf};

use crate::{evaluate, Config, ConfigError};

/// The variable that names another file, or `none`. A test uses it. Nothing else does.
pub const CONFIG_ENV: &str = "TESTANYWARE_CONFIG";

/// The default file, relative to the home directory.
pub const DEFAULT_RELATIVE_PATH: &str = ".config/testanyware/config.rhai";

/// Which file a process reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigLocation {
    /// `TESTANYWARE_CONFIG=none`, or no home directory to find the default in.
    Disabled,
    File(PathBuf),
}

/// What reading the location gave.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Loaded {
    /// The location names no file.
    Disabled,
    /// The file does not exist. The file is optional, so this is no error.
    Absent(PathBuf),
    Present {
        file: PathBuf,
        config: Config,
    },
}

impl Loaded {
    /// The settings of the file, or none when it is disabled or absent.
    pub fn config(&self) -> Option<&Config> {
        match self {
            Loaded::Present { config, .. } => Some(config),
            _ => None,
        }
    }
}

/// The file for the environment `get`. An empty variable counts as unset, as the other
/// `TESTANYWARE_*` variables do. The default is `~/.config/testanyware/config.rhai` on every
/// host, with `USERPROFILE` as the home directory where `HOME` is unset.
pub fn locate(get: &dyn Fn(&str) -> Option<String>) -> ConfigLocation {
    let non_empty = |key: &str| get(key).filter(|value| !value.is_empty());
    if let Some(named) = non_empty(CONFIG_ENV) {
        return if named == "none" {
            ConfigLocation::Disabled
        } else {
            ConfigLocation::File(PathBuf::from(named))
        };
    }
    match non_empty("HOME").or_else(|| non_empty("USERPROFILE")) {
        Some(home) => ConfigLocation::File(Path::new(&home).join(DEFAULT_RELATIVE_PATH)),
        None => ConfigLocation::Disabled,
    }
}

/// Read and evaluate the file at `location`. `read` returns the text of a file, so a test
/// reads from memory.
pub fn load(
    location: &ConfigLocation,
    read: &dyn Fn(&Path) -> io::Result<String>,
) -> Result<Loaded, ConfigError> {
    let ConfigLocation::File(file) = location else {
        return Ok(Loaded::Disabled);
    };
    match read(file) {
        Ok(source) => evaluate(&source, file).map(|config| Loaded::Present {
            file: file.clone(),
            config,
        }),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Loaded::Absent(file.clone())),
        Err(error) => Err(ConfigError {
            file: file.clone(),
            line: None,
            message: format!("cannot read the file: {error}"),
        }),
    }
}
