//! XDG-compliant path helpers for VM lifecycle artefacts.
//!
//! Ports `VMPaths.swift` and `QEMURunner.sessionDir`. The clone tree
//! (qcow2, EFI vars, TPM state) lives under `$XDG_DATA_HOME`; the AF_UNIX
//! sockets live under `$TMPDIR` so the path fits the 104-byte `sun_path`
//! limit (see decision log 2026-04-20).

use std::collections::HashMap;
use std::path::PathBuf;

/// Resolved VM-lifecycle directories.
#[derive(Debug, Clone)]
pub struct VmPaths {
    state_dir: PathBuf,
    data_dir: PathBuf,
    tmp_dir: PathBuf,
}

impl VmPaths {
    /// Resolve from the process environment.
    pub fn from_process_env() -> Self {
        let env: HashMap<String, String> = std::env::vars().collect();
        Self::from_env(&env)
    }

    /// Resolve from an explicit environment map (test-friendly). Mirrors
    /// `VMPaths.init(env:)`: `$XDG_STATE_HOME` / `$XDG_DATA_HOME` win when
    /// set and non-empty, else `$HOME/.local/{state,share}`. `$TMPDIR`
    /// resolves the socket session root, falling back to `/tmp`.
    pub fn from_env(env: &HashMap<String, String>) -> Self {
        let get = |k: &str| env.get(k).filter(|v| !v.is_empty()).cloned();
        let home = get("HOME").unwrap_or_default();
        let state_dir = match get("XDG_STATE_HOME") {
            Some(x) => PathBuf::from(x).join("testanyware"),
            None => PathBuf::from(&home).join(".local/state/testanyware"),
        };
        let data_dir = match get("XDG_DATA_HOME") {
            Some(x) => PathBuf::from(x).join("testanyware"),
            None => PathBuf::from(&home).join(".local/share/testanyware"),
        };
        let raw_tmp = get("TMPDIR").unwrap_or_else(|| "/tmp".to_string());
        let tmp_dir = PathBuf::from(raw_tmp.trim_end_matches('/'));
        Self { state_dir, data_dir, tmp_dir }
    }

    pub fn vms_dir(&self) -> PathBuf { self.state_dir.join("vms") }
    pub fn golden_dir(&self) -> PathBuf { self.data_dir.join("golden") }
    pub fn clones_dir(&self) -> PathBuf { self.data_dir.join("clones") }
    /// Persistent cache: install ISOs, virtio-win drivers, and the throwaway
    /// setup-VM disk during golden creation. Ports the script's
    /// `$_DATA_DIR/cache`.
    pub fn cache_dir(&self) -> PathBuf { self.data_dir.join("cache") }

    pub fn spec_path(&self, id: &str) -> PathBuf { self.vms_dir().join(format!("{id}.json")) }
    pub fn meta_path(&self, id: &str) -> PathBuf { self.vms_dir().join(format!("{id}.meta.json")) }
    pub fn clone_dir(&self, id: &str) -> PathBuf { self.clones_dir().join(id) }

    /// Per-VM short-path session dir under `$TMPDIR` for AF_UNIX sockets.
    /// Ports `QEMURunner.sessionDir(forID:)`.
    pub fn session_dir(&self, id: &str) -> PathBuf {
        self.tmp_dir.join(format!("testanyware-{id}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn xdg_dirs_win_when_set() {
        let p = VmPaths::from_env(&env(&[
            ("XDG_STATE_HOME", "/s"),
            ("XDG_DATA_HOME", "/d"),
        ]));
        assert_eq!(p.vms_dir(), PathBuf::from("/s/testanyware/vms"));
        assert_eq!(p.golden_dir(), PathBuf::from("/d/testanyware/golden"));
        assert_eq!(p.clones_dir(), PathBuf::from("/d/testanyware/clones"));
    }

    #[test]
    fn falls_back_to_home_dot_local() {
        let p = VmPaths::from_env(&env(&[("HOME", "/Users/alice")]));
        assert_eq!(p.vms_dir(), PathBuf::from("/Users/alice/.local/state/testanyware/vms"));
        assert_eq!(p.golden_dir(), PathBuf::from("/Users/alice/.local/share/testanyware/golden"));
    }

    #[test]
    fn empty_xdg_is_treated_as_unset() {
        let p = VmPaths::from_env(&env(&[("XDG_STATE_HOME", ""), ("HOME", "/h")]));
        assert_eq!(p.vms_dir(), PathBuf::from("/h/.local/state/testanyware/vms"));
    }

    #[test]
    fn spec_and_meta_paths() {
        let p = VmPaths::from_env(&env(&[("XDG_STATE_HOME", "/s")]));
        assert_eq!(p.spec_path("testanyware-abcd1234"),
            PathBuf::from("/s/testanyware/vms/testanyware-abcd1234.json"));
        assert_eq!(p.meta_path("testanyware-abcd1234"),
            PathBuf::from("/s/testanyware/vms/testanyware-abcd1234.meta.json"));
    }

    #[test]
    fn session_dir_strips_trailing_slash_from_tmpdir() {
        let p = VmPaths::from_env(&env(&[("TMPDIR", "/var/folders/x/T/")]));
        assert_eq!(p.session_dir("testanyware-abcd1234"),
            PathBuf::from("/var/folders/x/T/testanyware-testanyware-abcd1234"));
    }

    #[test]
    fn session_dir_defaults_to_tmp() {
        let p = VmPaths::from_env(&env(&[]));
        assert_eq!(p.session_dir("v"), PathBuf::from("/tmp/testanyware-v"));
    }
}
