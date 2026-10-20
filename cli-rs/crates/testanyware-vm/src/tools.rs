//! Where the lifecycle code looks for the tools it runs: `tart`, `qemu-system-*` and `swtpm`.
//!
//! The Host CLI searches `$PATH` and then the Homebrew directories. The TestAnyware app starts
//! from LaunchServices with no shell environment, so it uses the built-in search of the spec
//! ("Configuration") and reads no `PATH`. A caller passes the search to the code that runs a
//! tool, and a test passes a directory of fake tools.

use std::path::PathBuf;
use std::sync::OnceLock;

/// The directories where the app looks for a tool, in order.
const BUILTIN_DIRS: [&str; 3] = ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"];

/// The tool locations of `config.rhai`, which the Host CLI installs once at its start. Every
/// `ToolSearch::from_process_path()` of this process carries them, so each place that runs a
/// tool honours the file, golden creation included, without a parameter through the lifecycle
/// code. It is empty unless the Host CLI installs a file, so a test and the app are unaffected.
static CONFIGURED: OnceLock<Vec<(String, PathBuf)>> = OnceLock::new();

/// Install the tool locations of the file for this process. The first call wins, and it
/// returns `false` for a later one.
pub fn install_configured(tools: Vec<(String, PathBuf)>) -> bool {
    CONFIGURED.set(tools).is_ok()
}

/// An ordered list of directories that may hold a tool, and the locations that the file names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolSearch {
    dirs: Vec<PathBuf>,
    configured: Vec<(String, PathBuf)>,
}

impl ToolSearch {
    pub fn new(dirs: Vec<PathBuf>) -> Self {
        Self { dirs, configured: Vec::new() }
    }

    /// This search, with the tools that the file locates. A configured tool is found at its
    /// location or not at all: a wrong location is a mistake to report, so no directory of the
    /// search stands in for it.
    pub fn with_configured(mut self, configured: Vec<(String, PathBuf)>) -> Self {
        self.configured = configured;
        self
    }

    /// The search of the TestAnyware app: `/opt/homebrew/bin`, `/usr/local/bin` and `/usr/bin`.
    pub fn builtin() -> Self {
        Self::new(BUILTIN_DIRS.iter().map(PathBuf::from).collect())
    }

    /// The search of the Host CLI: `$PATH` of this process. On macOS, `/opt/homebrew/bin` and
    /// `/usr/local/bin` follow it, so the qemu toolchain resolves from a scrubbed environment.
    pub fn from_process_path() -> Self {
        // `mut` is consumed only by the macOS Homebrew pushes below.
        #[cfg_attr(not(target_os = "macos"), allow(unused_mut))]
        let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
            .map(|p| std::env::split_paths(&p).collect())
            .unwrap_or_default();
        #[cfg(target_os = "macos")]
        {
            dirs.push(PathBuf::from("/opt/homebrew/bin"));
            dirs.push(PathBuf::from("/usr/local/bin"));
        }
        Self::new(dirs).with_configured(CONFIGURED.get().cloned().unwrap_or_default())
    }

    pub fn dirs(&self) -> &[PathBuf] {
        &self.dirs
    }

    /// The location that the file names for `name`, else the first file called `name` in the
    /// directories, or `None`.
    pub fn find(&self, name: &str) -> Option<PathBuf> {
        if let Some((_, location)) = self.configured.iter().find(|(tool, _)| tool == name) {
            return location.is_file().then(|| location.clone());
        }
        self.dirs
            .iter()
            .map(|dir| dir.join(name))
            .find(|candidate| candidate.is_file())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_builtin_search_is_the_three_directories_in_order() {
        assert_eq!(
            ToolSearch::builtin().dirs(),
            [
                PathBuf::from("/opt/homebrew/bin"),
                PathBuf::from("/usr/local/bin"),
                PathBuf::from("/usr/bin"),
            ]
        );
    }

    #[test]
    fn find_returns_the_first_directory_that_holds_the_tool() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        std::fs::write(second.path().join("tart"), b"").unwrap();
        let search = ToolSearch::new(vec![first.path().into(), second.path().into()]);
        assert_eq!(search.find("tart"), Some(second.path().join("tart")));

        std::fs::write(first.path().join("tart"), b"").unwrap();
        assert_eq!(search.find("tart"), Some(first.path().join("tart")));
    }

    #[test]
    fn find_is_none_for_a_tool_that_no_directory_holds() {
        let dir = tempfile::tempdir().unwrap();
        let search = ToolSearch::new(vec![dir.path().into()]);
        assert_eq!(search.find("tart"), None);
        // A directory is no tool.
        std::fs::create_dir(dir.path().join("swtpm")).unwrap();
        assert_eq!(search.find("swtpm"), None);
    }

    #[test]
    fn a_configured_location_beats_the_directories() {
        let dir = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("tart"), b"").unwrap();
        std::fs::write(other.path().join("tart"), b"").unwrap();
        let search = ToolSearch::new(vec![dir.path().into()])
            .with_configured(vec![("tart".into(), other.path().join("tart"))]);
        assert_eq!(search.find("tart"), Some(other.path().join("tart")));
        // Another tool still comes from the directories.
        std::fs::write(dir.path().join("swtpm"), b"").unwrap();
        assert_eq!(search.find("swtpm"), Some(dir.path().join("swtpm")));
    }

    #[test]
    fn a_configured_location_that_is_no_file_is_not_replaced_by_a_directory() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("tart"), b"").unwrap();
        let search = ToolSearch::new(vec![dir.path().into()])
            .with_configured(vec![("tart".into(), dir.path().join("no-such-tart"))]);
        assert_eq!(search.find("tart"), None);
    }

    #[test]
    fn the_process_search_finds_sh() {
        assert!(ToolSearch::from_process_path().find("sh").is_some());
    }
}
