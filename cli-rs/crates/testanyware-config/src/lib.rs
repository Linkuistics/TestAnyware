//! The evaluator of `config.rhai`, shared by the Host CLI and the TestAnyware app.
//!
//! One optional Rhai file configures both processes (ADR-0021, spec "Configuration"). The
//! script runs under an operation limit with no access to files, processes or the network.
//! Its value is a map of settings, and an unknown key is an error.
//!
//! This crate only reads and checks the file. The order of the sources (flag, environment
//! variable, file, built-in default) belongs to the caller, because the Host CLI and the app
//! order them differently.

mod evaluate;
mod location;

pub use evaluate::{evaluate, MAX_OPERATIONS};
pub use location::{load, locate, ConfigLocation, Loaded, CONFIG_ENV, DEFAULT_RELATIVE_PATH};

use std::fmt;
use std::path::{Path, PathBuf};

/// Whether a host reads the file. This is the one switch (spec "Configuration"). The Host CLI
/// reads the file on every host, a macOS host included, because the app reads it too and the two
/// never disagree about the directories. A parameter of the pure functions carries the switch, so
/// a test turns it off for any host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Activation {
    Active,
    Inactive,
}

impl Activation {
    /// The switch of the host this binary runs on. It is on for every host.
    pub const fn for_this_host() -> Self {
        Activation::Active
    }

    pub const fn is_active(self) -> bool {
        matches!(self, Activation::Active)
    }
}

/// The settings of a file that evaluated. A setting that the file does not name is `None`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Config {
    pub tools: ToolLocations,
    /// The directory that holds `vms/`, as the Host CLI and the app use it.
    pub state_dir: Option<PathBuf>,
    /// The directory that holds `golden/`, `clones/` and `cache/`.
    pub data_dir: Option<PathBuf>,
}

/// The names that the `tools` map accepts, each the file name of the tool.
pub const TOOL_NAMES: [&str; 3] = ["tart", "qemu-system-aarch64", "swtpm"];

/// The locations of the tools that the file names.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ToolLocations {
    pub tart: Option<PathBuf>,
    pub qemu_system_aarch64: Option<PathBuf>,
    pub swtpm: Option<PathBuf>,
}

impl ToolLocations {
    /// The location of the tool called `name`, one of [`TOOL_NAMES`].
    pub fn get(&self, name: &str) -> Option<&Path> {
        match name {
            "tart" => self.tart.as_deref(),
            "qemu-system-aarch64" => self.qemu_system_aarch64.as_deref(),
            "swtpm" => self.swtpm.as_deref(),
            _ => None,
        }
    }

    /// Every tool that the file names, as `(name, location)`.
    pub fn named(&self) -> Vec<(&'static str, &Path)> {
        TOOL_NAMES
            .iter()
            .filter_map(|name| self.get(name).map(|path| (*name, path)))
            .collect()
    }
}

/// Why a file failed. It carries what `CONFIG_INVALID` reports: the file, the line and the
/// message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError {
    pub file: PathBuf,
    /// The 1-based line, for an error that Rhai places. A wrong type or an unknown key has no
    /// line, because the script has already finished when the evaluator reads its value.
    pub line: Option<usize>,
    pub message: String,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(line) => write!(f, "{}:{line}: {}", self.file.display(), self.message),
            None => write!(f, "{}: {}", self.file.display(), self.message),
        }
    }
}

impl std::error::Error for ConfigError {}
