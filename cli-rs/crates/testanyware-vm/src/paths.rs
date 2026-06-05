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
    /// set and non-empty (an explicit cross-platform override — the
    /// harness sets them), else the per-host default:
    ///   - **Unix:** `$HOME/.local/{state,share}/testanyware`,
    ///     socket root `$TMPDIR` → `/tmp`.
    ///   - **Windows:** `%LOCALAPPDATA%\testanyware{,\share}`, socket root
    ///     `%TEMP%` / `%TMP%`. Windows has no XDG/`$HOME` and no AF_UNIX
    ///     sockets — the local-QEMU path is build-verified only here
    ///     (ADR-0009) — but the resolver stays correct so `vm list` and
    ///     friends point at a real per-user dir instead of a bogus
    ///     `.local/...` under an empty home.
    pub fn from_env(env: &HashMap<String, String>) -> Self {
        let get = |k: &str| env.get(k).filter(|v| !v.is_empty()).cloned();

        let state_dir = match get("XDG_STATE_HOME") {
            Some(x) => PathBuf::from(x).join("testanyware"),
            None => host_default_dir(&get, "state").join("testanyware"),
        };
        let data_dir = match get("XDG_DATA_HOME") {
            Some(x) => PathBuf::from(x).join("testanyware"),
            None => host_default_dir(&get, "share").join("testanyware"),
        };
        let tmp_dir = host_tmp_dir(&get);
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

/// Per-host base for the `state`/`share` XDG-style dirs (the caller
/// appends `testanyware`). Unix → `$HOME/.local/<kind>`.
#[cfg(not(target_os = "windows"))]
fn host_default_dir(get: &impl Fn(&str) -> Option<String>, kind: &str) -> PathBuf {
    let home = get("HOME").unwrap_or_default();
    PathBuf::from(home).join(".local").join(kind)
}

/// Windows has no XDG split: both `state` and `share` live under
/// `%LOCALAPPDATA%` (per-user, non-roaming), so `vms`/`golden`/`clones`
/// sit side by side in one app-data root. Falls back to
/// `%USERPROFILE%\AppData\Local`, then a relative dir.
#[cfg(target_os = "windows")]
fn host_default_dir(get: &impl Fn(&str) -> Option<String>, _kind: &str) -> PathBuf {
    if let Some(local) = get("LOCALAPPDATA") {
        return PathBuf::from(local);
    }
    if let Some(profile) = get("USERPROFILE") {
        return PathBuf::from(profile).join("AppData").join("Local");
    }
    PathBuf::from(".")
}

/// Socket/scratch root. Unix → `$TMPDIR` else `/tmp`.
#[cfg(not(target_os = "windows"))]
fn host_tmp_dir(get: &impl Fn(&str) -> Option<String>) -> PathBuf {
    let raw = get("TMPDIR").unwrap_or_else(|| "/tmp".to_string());
    PathBuf::from(raw.trim_end_matches('/'))
}

/// Windows scratch root: honour `$TMPDIR` if a caller set it (the harness
/// may), else `%TEMP%` / `%TMP%`, else the conventional system temp.
#[cfg(target_os = "windows")]
fn host_tmp_dir(get: &impl Fn(&str) -> Option<String>) -> PathBuf {
    let raw = get("TMPDIR")
        .or_else(|| get("TEMP"))
        .or_else(|| get("TMP"))
        .unwrap_or_else(|| r"C:\Windows\Temp".to_string());
    PathBuf::from(raw.trim_end_matches(['/', '\\']))
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
    #[cfg(not(target_os = "windows"))]
    fn session_dir_defaults_to_tmp() {
        let p = VmPaths::from_env(&env(&[]));
        assert_eq!(p.session_dir("v"), PathBuf::from("/tmp/testanyware-v"));
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn windows_falls_back_to_localappdata() {
        // No XDG, no HOME: Windows resolves under %LOCALAPPDATA% (state and
        // data share one app-data root) rather than a bogus `.local/...`.
        let p = VmPaths::from_env(&env(&[("LOCALAPPDATA", r"C:\Users\alice\AppData\Local")]));
        assert_eq!(p.vms_dir(), PathBuf::from(r"C:\Users\alice\AppData\Local\testanyware\vms"));
        assert_eq!(p.golden_dir(), PathBuf::from(r"C:\Users\alice\AppData\Local\testanyware\golden"));
    }
}
