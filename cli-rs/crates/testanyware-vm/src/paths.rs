//! XDG-compliant path helpers for VM lifecycle artefacts.
//!
//! Ports `VMPaths.swift` and `QEMURunner.sessionDir`. The clone tree
//! (qcow2, EFI vars, TPM state) lives under `$XDG_DATA_HOME`; the AF_UNIX
//! sockets live under `$TMPDIR` so the path fits the 104-byte `sun_path`
//! limit (see decision log 2026-04-20).

use std::collections::HashMap;
use std::path::PathBuf;

/// The directories that `config.rhai` names. A directory that the file does not name is `None`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileDirs {
    pub state_dir: Option<PathBuf>,
    pub data_dir: Option<PathBuf>,
}

/// Resolved VM-lifecycle directories.
#[derive(Debug, Clone)]
pub struct VmPaths {
    state_dir: PathBuf,
    data_dir: PathBuf,
    tmp_dir: PathBuf,
}

impl VmPaths {
    /// Directories that the caller resolved. The TestAnyware app takes them from its request
    /// headers and its arguments, never from its own environment.
    pub fn new(state_dir: PathBuf, data_dir: PathBuf, tmp_dir: PathBuf) -> Self {
        Self { state_dir, data_dir, tmp_dir }
    }

    /// The built-in directories of the TestAnyware app: the host default under `home`, with no
    /// environment variable. The app has no shell environment, and `config.rhai` is the one way
    /// to move its directories (spec "Configuration"). The app and the directory check of golden
    /// creation both take their defaults from here, so the two cannot disagree about them.
    pub fn builtin_for_home(home: &std::path::Path) -> Self {
        let env = HashMap::from([("HOME".to_string(), home.to_string_lossy().into_owned())]);
        Self::from_env(&env)
    }

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
        Self::from_env_and_file(env, &FileDirs::default())
    }

    /// [`Self::from_env`] with the directories of `config.rhai` (ADR-0021). The environment
    /// variable beats the file, and the file beats the host default. A directory of the file is
    /// the directory itself, so no `testanyware` is appended to it.
    pub fn from_env_and_file(env: &HashMap<String, String>, file: &FileDirs) -> Self {
        let get = |k: &str| env.get(k).filter(|v| !v.is_empty()).cloned();

        let state_dir = match (get("XDG_STATE_HOME"), &file.state_dir) {
            (Some(x), _) => PathBuf::from(x).join("testanyware"),
            (None, Some(dir)) => dir.clone(),
            (None, None) => host_default_dir(&get, "state").join("testanyware"),
        };
        let data_dir = match (get("XDG_DATA_HOME"), &file.data_dir) {
            (Some(x), _) => PathBuf::from(x).join("testanyware"),
            (None, Some(dir)) => dir.clone(),
            (None, None) => host_default_dir(&get, "share").join("testanyware"),
        };
        let tmp_dir = host_tmp_dir(&get);
        Self { state_dir, data_dir, tmp_dir }
    }

    /// The directory that holds `vms/`: `$XDG_STATE_HOME/testanyware`.
    pub fn state_dir(&self) -> &std::path::Path { &self.state_dir }
    /// The directory that holds `golden/` and `clones/`: `$XDG_DATA_HOME/testanyware`.
    pub fn data_dir(&self) -> &std::path::Path { &self.data_dir }

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
    fn the_builtin_directories_of_the_app_come_from_home_alone() {
        let p = VmPaths::builtin_for_home(std::path::Path::new("/Users/alice"));
        assert_eq!(p.state_dir(), std::path::Path::new("/Users/alice/.local/state/testanyware"));
        assert_eq!(p.data_dir(), std::path::Path::new("/Users/alice/.local/share/testanyware"));
    }

    #[test]
    fn empty_xdg_is_treated_as_unset() {
        let p = VmPaths::from_env(&env(&[("XDG_STATE_HOME", ""), ("HOME", "/h")]));
        assert_eq!(p.vms_dir(), PathBuf::from("/h/.local/state/testanyware/vms"));
    }

    #[test]
    fn the_environment_beats_the_file_and_the_file_beats_the_default() {
        let file = FileDirs {
            state_dir: Some(PathBuf::from("/f/state")),
            data_dir: Some(PathBuf::from("/f/data")),
        };
        // Neither variable: the file, with nothing appended.
        let p = VmPaths::from_env_and_file(&env(&[("HOME", "/h")]), &file);
        assert_eq!(p.state_dir(), std::path::Path::new("/f/state"));
        assert_eq!(p.data_dir(), std::path::Path::new("/f/data"));
        assert_eq!(p.vms_dir(), PathBuf::from("/f/state/vms"));
        // One variable beats its own setting only.
        let p = VmPaths::from_env_and_file(&env(&[("XDG_STATE_HOME", "/s"), ("HOME", "/h")]), &file);
        assert_eq!(p.state_dir(), std::path::Path::new("/s/testanyware"));
        assert_eq!(p.data_dir(), std::path::Path::new("/f/data"));
        // An empty variable counts as unset.
        let p = VmPaths::from_env_and_file(&env(&[("XDG_DATA_HOME", "")]), &file);
        assert_eq!(p.data_dir(), std::path::Path::new("/f/data"));
        // A file that names one directory leaves the other to the default.
        let state_only = FileDirs { state_dir: Some(PathBuf::from("/f/state")), data_dir: None };
        let p = VmPaths::from_env_and_file(&env(&[("HOME", "/h")]), &state_only);
        assert_eq!(p.data_dir(), std::path::Path::new("/h/.local/share/testanyware"));
    }

    #[test]
    fn no_file_resolves_as_before() {
        let e = env(&[("XDG_STATE_HOME", "/s"), ("HOME", "/h")]);
        let (a, b) = (VmPaths::from_env(&e), VmPaths::from_env_and_file(&e, &FileDirs::default()));
        assert_eq!((a.state_dir(), a.data_dir()), (b.state_dir(), b.data_dir()));
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
