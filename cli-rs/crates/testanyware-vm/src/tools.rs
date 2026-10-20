//! Where the lifecycle code looks for the tools it runs: `tart`, `qemu-system-*` and `swtpm`.
//!
//! The Host CLI searches `$PATH` and then the Homebrew directories. The TestAnyware app starts
//! from LaunchServices with no shell environment, so it uses the built-in search of the spec
//! ("Configuration") and reads no `PATH`. A caller passes the search to the code that runs a
//! tool, and a test passes a directory of fake tools.

use std::path::PathBuf;

/// The directories where the app looks for a tool, in order.
const BUILTIN_DIRS: [&str; 3] = ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"];

/// An ordered list of directories that may hold a tool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolSearch {
    dirs: Vec<PathBuf>,
}

impl ToolSearch {
    pub fn new(dirs: Vec<PathBuf>) -> Self {
        Self { dirs }
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
        Self::new(dirs)
    }

    pub fn dirs(&self) -> &[PathBuf] {
        &self.dirs
    }

    /// The first file called `name` in the directories, or `None`.
    pub fn find(&self, name: &str) -> Option<PathBuf> {
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
    fn the_process_search_finds_sh() {
        assert!(ToolSearch::from_process_path().find("sh").is_some());
    }
}
