//! Connection resolution: turn the user's `--connect`/`--vm`/`--agent`
//! flags and `TESTANYWARE_*` env vars into an addressable agent endpoint.
//!
//! This mirrors Swift's `ConnectionOptions.resolveAgent` plus
//! `VMPaths.specPath(forID:)`. The chain is:
//!
//!   1. `--connect <path>` — explicit spec file
//!   2. `--vm <id>` — per-VM spec under `<state>/testanyware/vms/<id>.json`
//!   3. `--agent <host:port>` — explicit endpoint (skips VNC)
//!   4. error (`NO_CONNECTION_SPECIFIED`)
//!
//! Per Swift parity, `<state>` resolves to `$XDG_STATE_HOME` if set and
//! non-empty, otherwise `$HOME/.local/state`. On Windows
//! (`cfg(target_os = "windows")`), we fall back to `%LOCALAPPDATA%` per
//! the task spec.

use std::path::PathBuf;

use serde::Deserialize;
use thiserror::Error;

/// User-specified connection options. Mirrors clap's `ConnectionOptions`
/// flattened struct in `main.rs`. Kept here as a plain data carrier so
/// command handlers do not need to depend on clap.
#[derive(Debug, Clone, Default)]
pub struct ConnectionOptions {
    pub connect: Option<String>,
    pub vm: Option<String>,
    pub agent: Option<String>,
    pub vnc: Option<String>,
    pub platform: Option<String>,
}

/// Resolved agent endpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedAgent {
    pub host: String,
    pub port: u16,
}

impl ResolvedAgent {
    pub const DEFAULT_PORT: u16 = 8648;
}

/// Resolved VNC endpoint. Surfaced for `screen size`, `screen capture`,
/// and the `input` family of commands once they land.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedVnc {
    pub host: String,
    pub port: u16,
    pub password: Option<String>,
}

impl ResolvedVnc {
    pub const DEFAULT_PORT: u16 = 5900;
}

/// On-disk per-VM spec file. Mirrors Swift's `ConnectionSpec` JSON layout.
#[derive(Debug, Clone, Deserialize)]
pub struct ConnectionSpec {
    #[serde(default)]
    #[allow(dead_code)] // surfaced once VNC commands land
    pub vnc: Option<VncSpec>,
    #[serde(default)]
    pub agent: Option<AgentSpec>,
    #[serde(default)]
    pub platform: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VncSpec {
    pub host: String,
    pub port: u16,
    #[serde(default)]
    pub password: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AgentSpec {
    pub host: String,
    pub port: u16,
}

/// Errors produced by the resolution chain. Each variant maps to a §4
/// error code via `code()` so the CLI can emit a stable JSON envelope.
#[derive(Debug, Error)]
pub enum ResolveError {
    #[error(
        "No connection specified. Provide --connect <path>, --vm <id>, --agent <host:port>, \
         or set TESTANYWARE_VM_ID / TESTANYWARE_AGENT. \
         Start a VM with `testanyware vm start` to create a spec."
    )]
    NoConnectionSpecified,

    #[error("No spec found for VM id '{id}' at {path}")]
    VmNotFound { id: String, path: PathBuf },

    #[error("Connection spec at {path} has no `agent` section; this command requires the in-VM agent")]
    NoAgentInSpec { path: PathBuf },

    #[error("Connection spec at {path} has no `vnc` section; this command requires a VNC endpoint")]
    NoVncInSpec { path: PathBuf },

    #[error("Failed to read spec file {path}: {source}")]
    SpecRead {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to parse spec file {path}: {source}")]
    SpecParse {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error("Invalid agent endpoint '{value}': {reason}")]
    InvalidEndpoint { value: String, reason: String },

    #[error("HOME environment variable is not set; cannot resolve XDG state path")]
    NoHome,
}

impl ResolveError {
    /// Stable §4 code surfaced in `--json` output.
    pub fn code(&self) -> &'static str {
        match self {
            ResolveError::NoConnectionSpecified => "NO_CONNECTION_SPECIFIED",
            ResolveError::VmNotFound { .. } => "VM_NOT_FOUND",
            ResolveError::NoAgentInSpec { .. } => "NO_CONNECTION_SPECIFIED",
            ResolveError::NoVncInSpec { .. } => "NO_CONNECTION_SPECIFIED",
            ResolveError::SpecRead { .. } => "IO_ERROR",
            ResolveError::SpecParse { .. } => "IO_ERROR",
            ResolveError::InvalidEndpoint { .. } => "INVALID_ENDPOINT",
            ResolveError::NoHome => "INTERNAL",
        }
    }

    /// §5 process exit code.
    pub fn exit_code(&self) -> i32 {
        match self {
            ResolveError::VmNotFound { .. } => 3,
            ResolveError::NoConnectionSpecified
            | ResolveError::NoAgentInSpec { .. }
            | ResolveError::NoVncInSpec { .. }
            | ResolveError::InvalidEndpoint { .. } => 2,
            _ => 1,
        }
    }

    /// `details` payload for the §3.4 JSON error envelope.
    pub fn details(&self) -> serde_json::Value {
        match self {
            ResolveError::VmNotFound { id, path } => serde_json::json!({
                "vm_id": id,
                "spec_path": path.display().to_string(),
            }),
            ResolveError::NoAgentInSpec { path } => serde_json::json!({
                "spec_path": path.display().to_string(),
            }),
            ResolveError::NoVncInSpec { path } => serde_json::json!({
                "spec_path": path.display().to_string(),
            }),
            ResolveError::SpecRead { path, .. } | ResolveError::SpecParse { path, .. } => {
                serde_json::json!({ "spec_path": path.display().to_string() })
            }
            ResolveError::InvalidEndpoint { value, reason } => serde_json::json!({
                "value": value,
                "reason": reason,
            }),
            _ => serde_json::Value::Null,
        }
    }
}

/// Run the resolution chain to produce an addressable VNC endpoint.
/// The chain mirrors `resolve_agent` but consumes the `vnc` section
/// from spec files and the `--vnc` / `TESTANYWARE_VNC` direct flag.
pub fn resolve_vnc(opts: &ConnectionOptions) -> Result<ResolvedVnc, ResolveError> {
    resolve_vnc_with_env(opts, &EnvProvider::process())
}

pub fn resolve_vnc_with_env(
    opts: &ConnectionOptions,
    env: &EnvProvider,
) -> Result<ResolvedVnc, ResolveError> {
    if let Some(path) = &opts.connect {
        let spec_path = expand_tilde(path, env);
        return load_vnc_from_spec(&spec_path);
    }
    if let Some(id) = &opts.vm {
        let spec_path = vms_dir(env)?.join(format!("{id}.json"));
        if !spec_path.is_file() {
            return Err(ResolveError::VmNotFound {
                id: id.clone(),
                path: spec_path,
            });
        }
        return load_vnc_from_spec(&spec_path);
    }
    if let Some(endpoint) = &opts.vnc {
        let mut resolved = parse_vnc_endpoint(endpoint)?;
        // The TESTANYWARE_VNC_PASSWORD env var may carry the password
        // when --vnc / TESTANYWARE_VNC carry only `host:port`.
        if resolved.password.is_none() {
            if let Some(pw) = env.get("TESTANYWARE_VNC_PASSWORD") {
                if !pw.is_empty() {
                    resolved.password = Some(pw);
                }
            }
        }
        return Ok(resolved);
    }
    Err(ResolveError::NoConnectionSpecified)
}

fn load_vnc_from_spec(path: &std::path::Path) -> Result<ResolvedVnc, ResolveError> {
    let bytes = std::fs::read(path).map_err(|source| ResolveError::SpecRead {
        path: path.to_path_buf(),
        source,
    })?;
    let spec: ConnectionSpec =
        serde_json::from_slice(&bytes).map_err(|source| ResolveError::SpecParse {
            path: path.to_path_buf(),
            source,
        })?;
    let vnc = spec.vnc.ok_or_else(|| ResolveError::NoVncInSpec {
        path: path.to_path_buf(),
    })?;
    Ok(ResolvedVnc {
        host: vnc.host,
        port: vnc.port,
        password: vnc.password,
    })
}

/// Parse a `host:port` endpoint with no embedded password (port
/// optional, defaults to 5900).
pub fn parse_vnc_endpoint(value: &str) -> Result<ResolvedVnc, ResolveError> {
    let (host, port) = match value.rsplit_once(':') {
        Some((h, p)) => {
            if h.is_empty() {
                return Err(ResolveError::InvalidEndpoint {
                    value: value.to_string(),
                    reason: "host is empty".into(),
                });
            }
            let port: u16 = p
                .parse()
                .map_err(|_| ResolveError::InvalidEndpoint {
                    value: value.to_string(),
                    reason: format!("invalid port '{p}'; expected 1..=65535"),
                })?;
            if port == 0 {
                return Err(ResolveError::InvalidEndpoint {
                    value: value.to_string(),
                    reason: "port must be 1..=65535".into(),
                });
            }
            (h.to_string(), port)
        }
        None => {
            if value.is_empty() {
                return Err(ResolveError::InvalidEndpoint {
                    value: value.to_string(),
                    reason: "host is empty".into(),
                });
            }
            (value.to_string(), ResolvedVnc::DEFAULT_PORT)
        }
    };
    Ok(ResolvedVnc {
        host,
        port,
        password: None,
    })
}

/// Resolve the target platform string from `--platform`/env or, when
/// neither is supplied, from the per-VM spec referenced by
/// `--connect`/`--vm`. Returns `Ok(None)` when no source was available.
///
/// Mirrors Swift's `ConnectionSpec.platform` precedence: explicit flag
/// wins, otherwise the spec's `platform` field is consulted, otherwise
/// the caller decides the default.
pub fn resolve_platform(opts: &ConnectionOptions) -> Result<Option<String>, ResolveError> {
    resolve_platform_with_env(opts, &EnvProvider::process())
}

pub fn resolve_platform_with_env(
    opts: &ConnectionOptions,
    env: &EnvProvider,
) -> Result<Option<String>, ResolveError> {
    if let Some(p) = &opts.platform {
        if !p.is_empty() {
            return Ok(Some(p.clone()));
        }
    }
    if let Some(path) = &opts.connect {
        let spec_path = expand_tilde(path, env);
        return Ok(load_spec(&spec_path)?.platform);
    }
    if let Some(id) = &opts.vm {
        let spec_path = vms_dir(env)?.join(format!("{id}.json"));
        if !spec_path.is_file() {
            return Err(ResolveError::VmNotFound {
                id: id.clone(),
                path: spec_path,
            });
        }
        return Ok(load_spec(&spec_path)?.platform);
    }
    Ok(None)
}

fn load_spec(path: &std::path::Path) -> Result<ConnectionSpec, ResolveError> {
    let bytes = std::fs::read(path).map_err(|source| ResolveError::SpecRead {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_slice(&bytes).map_err(|source| ResolveError::SpecParse {
        path: path.to_path_buf(),
        source,
    })
}

/// Run the resolution chain to produce an addressable agent endpoint.
pub fn resolve_agent(opts: &ConnectionOptions) -> Result<ResolvedAgent, ResolveError> {
    resolve_agent_with_env(opts, &EnvProvider::process())
}

/// Test-friendly variant: the env provider is injected so the test suite
/// can exercise the chain without poking at process-wide env vars.
pub fn resolve_agent_with_env(
    opts: &ConnectionOptions,
    env: &EnvProvider,
) -> Result<ResolvedAgent, ResolveError> {
    if let Some(path) = &opts.connect {
        let spec_path = expand_tilde(path, env);
        return load_agent_from_spec(&spec_path);
    }
    if let Some(id) = &opts.vm {
        let spec_path = vms_dir(env)?.join(format!("{id}.json"));
        if !spec_path.is_file() {
            return Err(ResolveError::VmNotFound {
                id: id.clone(),
                path: spec_path,
            });
        }
        return load_agent_from_spec(&spec_path);
    }
    if let Some(endpoint) = &opts.agent {
        return parse_agent_endpoint(endpoint);
    }
    Err(ResolveError::NoConnectionSpecified)
}

fn load_agent_from_spec(path: &std::path::Path) -> Result<ResolvedAgent, ResolveError> {
    let bytes = std::fs::read(path).map_err(|source| ResolveError::SpecRead {
        path: path.to_path_buf(),
        source,
    })?;
    let spec: ConnectionSpec =
        serde_json::from_slice(&bytes).map_err(|source| ResolveError::SpecParse {
            path: path.to_path_buf(),
            source,
        })?;
    let agent = spec.agent.ok_or_else(|| ResolveError::NoAgentInSpec {
        path: path.to_path_buf(),
    })?;
    Ok(ResolvedAgent {
        host: agent.host,
        port: agent.port,
    })
}

/// Parse `host:port` (port optional, defaults to 8648). Mirrors Swift's
/// `parseAgentEndpoint` byte-for-byte.
pub fn parse_agent_endpoint(value: &str) -> Result<ResolvedAgent, ResolveError> {
    let (host, port) = match value.rsplit_once(':') {
        Some((h, p)) => {
            if h.is_empty() {
                return Err(ResolveError::InvalidEndpoint {
                    value: value.to_string(),
                    reason: "host is empty".into(),
                });
            }
            let port: u16 = p
                .parse()
                .map_err(|_| ResolveError::InvalidEndpoint {
                    value: value.to_string(),
                    reason: format!("invalid port '{p}'; expected 1..=65535"),
                })?;
            if port == 0 {
                return Err(ResolveError::InvalidEndpoint {
                    value: value.to_string(),
                    reason: "port must be 1..=65535".into(),
                });
            }
            (h.to_string(), port)
        }
        None => {
            if value.is_empty() {
                return Err(ResolveError::InvalidEndpoint {
                    value: value.to_string(),
                    reason: "host is empty".into(),
                });
            }
            (value.to_string(), ResolvedAgent::DEFAULT_PORT)
        }
    };
    Ok(ResolvedAgent { host, port })
}

fn vms_dir(env: &EnvProvider) -> Result<PathBuf, ResolveError> {
    Ok(state_dir(env)?.join("testanyware").join("vms"))
}

fn state_dir(env: &EnvProvider) -> Result<PathBuf, ResolveError> {
    if let Some(value) = env.get("XDG_STATE_HOME") {
        if !value.is_empty() {
            return Ok(PathBuf::from(value));
        }
    }
    #[cfg(target_os = "windows")]
    {
        if let Some(value) = env.get("LOCALAPPDATA") {
            if !value.is_empty() {
                return Ok(PathBuf::from(value));
            }
        }
        // Windows has no `$HOME`; `%USERPROFILE%` is the per-user root.
        // Mirror the XDG layout under it so the spec path is stable.
        if let Some(profile) = env.get("USERPROFILE") {
            if !profile.is_empty() {
                return Ok(PathBuf::from(profile).join(".local").join("state"));
            }
        }
    }
    let home = env.get("HOME").ok_or(ResolveError::NoHome)?;
    Ok(PathBuf::from(home).join(".local").join("state"))
}

fn expand_tilde(path: &str, env: &EnvProvider) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = env.get("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    if path == "~" {
        if let Some(home) = env.get("HOME") {
            return PathBuf::from(home);
        }
    }
    PathBuf::from(path)
}

/// Indirection over `std::env::var` so the resolution chain can be tested
/// with a synthetic environment.
type EnvLookup = Box<dyn Fn(&str) -> Option<String> + Send + Sync>;

pub struct EnvProvider {
    inner: EnvLookup,
}

impl EnvProvider {
    pub fn process() -> Self {
        Self {
            inner: Box::new(|key| std::env::var(key).ok()),
        }
    }

    pub fn from_map(map: std::collections::HashMap<String, String>) -> Self {
        Self {
            inner: Box::new(move |key| map.get(key).cloned()),
        }
    }

    fn get(&self, key: &str) -> Option<String> {
        (self.inner)(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::io::Write;

    fn env_with(pairs: &[(&str, &str)]) -> EnvProvider {
        let mut map = HashMap::new();
        for (k, v) in pairs {
            map.insert((*k).to_string(), (*v).to_string());
        }
        EnvProvider::from_map(map)
    }

    #[test]
    fn parse_endpoint_with_explicit_port() {
        let r = parse_agent_endpoint("192.168.64.2:9000").expect("ok");
        assert_eq!(r.host, "192.168.64.2");
        assert_eq!(r.port, 9000);
    }

    #[test]
    fn parse_endpoint_defaults_port_to_8648() {
        let r = parse_agent_endpoint("agent.local").expect("ok");
        assert_eq!(r.host, "agent.local");
        assert_eq!(r.port, ResolvedAgent::DEFAULT_PORT);
    }

    #[test]
    fn parse_endpoint_rejects_empty() {
        let err = parse_agent_endpoint("").expect_err("should reject empty");
        assert_eq!(err.code(), "INVALID_ENDPOINT");
    }

    #[test]
    fn parse_endpoint_rejects_bad_port() {
        let err = parse_agent_endpoint("host:abc").expect_err("bad port");
        assert_eq!(err.code(), "INVALID_ENDPOINT");
    }

    #[test]
    fn resolve_returns_error_with_no_input() {
        let opts = ConnectionOptions::default();
        let err = resolve_agent_with_env(&opts, &env_with(&[])).expect_err("no input");
        assert!(matches!(err, ResolveError::NoConnectionSpecified));
        assert_eq!(err.code(), "NO_CONNECTION_SPECIFIED");
        assert_eq!(err.exit_code(), 2);
    }

    #[test]
    fn resolve_uses_explicit_agent_flag() {
        let opts = ConnectionOptions {
            agent: Some("10.0.0.1:1234".into()),
            ..Default::default()
        };
        let r = resolve_agent_with_env(&opts, &env_with(&[])).expect("ok");
        assert_eq!(r.host, "10.0.0.1");
        assert_eq!(r.port, 1234);
    }

    #[test]
    fn resolve_loads_per_vm_spec() {
        let dir = tempdir();
        let vms = dir.path().join("testanyware").join("vms");
        std::fs::create_dir_all(&vms).unwrap();
        let id = "testanyware-deadbeef";
        let spec_path = vms.join(format!("{id}.json"));
        std::fs::write(
            &spec_path,
            serde_json::to_vec(&serde_json::json!({
                "vnc": { "host": "127.0.0.1", "port": 5900 },
                "agent": { "host": "192.168.64.5", "port": 8648 },
                "platform": "linux"
            }))
            .unwrap(),
        )
        .unwrap();

        let env = env_with(&[("XDG_STATE_HOME", dir.path().to_str().unwrap())]);
        let opts = ConnectionOptions {
            vm: Some(id.into()),
            ..Default::default()
        };
        let r = resolve_agent_with_env(&opts, &env).expect("ok");
        assert_eq!(r.host, "192.168.64.5");
        assert_eq!(r.port, 8648);
    }

    #[test]
    fn resolve_reports_vm_not_found_when_spec_absent() {
        let dir = tempdir();
        let env = env_with(&[("XDG_STATE_HOME", dir.path().to_str().unwrap())]);
        let opts = ConnectionOptions {
            vm: Some("testanyware-missing".into()),
            ..Default::default()
        };
        let err = resolve_agent_with_env(&opts, &env).expect_err("missing");
        match &err {
            ResolveError::VmNotFound { id, .. } => assert_eq!(id, "testanyware-missing"),
            other => panic!("expected VmNotFound, got {other:?}"),
        }
        assert_eq!(err.code(), "VM_NOT_FOUND");
        assert_eq!(err.exit_code(), 3);
    }

    #[test]
    fn resolve_reports_no_agent_in_spec_when_field_missing() {
        let dir = tempdir();
        let vms = dir.path().join("testanyware").join("vms");
        std::fs::create_dir_all(&vms).unwrap();
        let id = "testanyware-novirt";
        let spec_path = vms.join(format!("{id}.json"));
        std::fs::write(
            &spec_path,
            br#"{"vnc":{"host":"127.0.0.1","port":5900}}"#,
        )
        .unwrap();

        let env = env_with(&[("XDG_STATE_HOME", dir.path().to_str().unwrap())]);
        let opts = ConnectionOptions {
            vm: Some(id.into()),
            ..Default::default()
        };
        let err = resolve_agent_with_env(&opts, &env).expect_err("no agent");
        assert!(matches!(err, ResolveError::NoAgentInSpec { .. }));
    }

    #[test]
    fn resolve_loads_explicit_connect_path() {
        let dir = tempdir();
        let path = dir.path().join("custom-spec.json");
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(br#"{"vnc":{"host":"a","port":5900},"agent":{"host":"b","port":7777}}"#)
            .unwrap();

        let opts = ConnectionOptions {
            connect: Some(path.to_str().unwrap().to_string()),
            ..Default::default()
        };
        let r = resolve_agent_with_env(&opts, &env_with(&[])).expect("ok");
        assert_eq!(r.host, "b");
        assert_eq!(r.port, 7777);
    }

    #[test]
    fn state_dir_uses_xdg_state_home_when_set() {
        let env = env_with(&[("XDG_STATE_HOME", "/custom/state")]);
        assert_eq!(state_dir(&env).unwrap(), PathBuf::from("/custom/state"));
    }

    #[test]
    fn state_dir_falls_back_to_home_dot_local_state() {
        let env = env_with(&[("HOME", "/Users/alice")]);
        assert_eq!(
            state_dir(&env).unwrap(),
            PathBuf::from("/Users/alice/.local/state")
        );
    }

    #[test]
    fn state_dir_treats_empty_xdg_as_unset() {
        let env = env_with(&[("XDG_STATE_HOME", ""), ("HOME", "/Users/alice")]);
        assert_eq!(
            state_dir(&env).unwrap(),
            PathBuf::from("/Users/alice/.local/state")
        );
    }

    fn tempdir() -> tempfile::TempDir {
        tempfile::tempdir().expect("tempdir")
    }
}
