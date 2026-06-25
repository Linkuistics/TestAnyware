//! `VmSpec` — the public per-VM spec sidecar at `<vms>/<id>.json`.
//!
//! Port of `VMSpec.swift`. Written by `vm start`, read by the CLI's
//! connection-resolution chain (`resolve.rs::ConnectionSpec`).

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::VmError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VncEndpoint {
    pub host: String,
    pub port: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentEndpoint {
    pub host: String,
    pub port: u16,
}

/// The HiDPI **logical** surface a `@2x` VM presents to consumers (ADR-0016
/// D2): the point dimensions that `screen *`, vision, the viewer, and `input`
/// operate in, while the guest renders at the physical 2× framebuffer. Persisted
/// here by `vm start --display WxH@2x` so each later, short-lived command
/// connection (ADR-0004) re-reads it and calls k5's `set_logical_target`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogicalSize {
    pub width: u32,
    pub height: u32,
}

/// Public per-VM spec. `platform` is a plain string (`macos`/`linux`/
/// `windows`) so it round-trips with `ConnectionSpec.platform`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VmSpec {
    pub vnc: VncEndpoint,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<AgentEndpoint>,
    pub platform: String,
    /// The HiDPI logical surface, present only for a `@2x` VM (ADR-0016 D2).
    /// Absent (and omitted from the JSON) on the default 1× path, so a 1× spec
    /// is byte-identical to before this field existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logical: Option<LogicalSize>,
}

impl VmSpec {
    pub fn load(path: &Path) -> Result<Self, VmError> {
        let bytes = std::fs::read(path).map_err(|e| VmError::Io(format!("{}: {e}", path.display())))?;
        serde_json::from_slice(&bytes).map_err(|e| VmError::Io(format!("{}: {e}", path.display())))
    }

    /// Atomically write `self` to `path`, mode 0600. Ports
    /// `VMSpec.writeAtomic`.
    pub fn write_atomic(&self, path: &Path) -> Result<(), VmError> {
        let json = serde_json::to_vec_pretty(self)
            .map_err(|e| VmError::Io(format!("serialize spec: {e}")))?;
        crate::spec::write_atomic_0600(path, &json)
    }
}

/// Shared atomic-write helper used by both sidecars.
pub(crate) fn write_atomic_0600(path: &Path, bytes: &[u8]) -> Result<(), VmError> {
    let tmp = std::path::PathBuf::from(format!("{}.tmp", path.display()));
    std::fs::write(&tmp, bytes).map_err(|e| VmError::Io(format!("{}: {e}", tmp.display())))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| VmError::Io(format!("chmod {}: {e}", tmp.display())))?;
    }
    std::fs::rename(&tmp, path).map_err(|e| VmError::Io(format!("rename into {}: {e}", path.display())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("testanyware-deadbeef.json");
        let spec = VmSpec {
            vnc: VncEndpoint { host: "localhost".into(), port: 5901, password: Some("testanyware".into()) },
            agent: Some(AgentEndpoint { host: "localhost".into(), port: 51234 }),
            platform: "windows".into(),
            logical: None,
        };
        spec.write_atomic(&path).unwrap();
        assert_eq!(VmSpec::load(&path).unwrap(), spec);
    }

    #[test]
    fn hidpi_logical_round_trips_and_a_1x_spec_omits_it() {
        let dir = tempfile::tempdir().unwrap();
        // A @2x VM persists its logical surface (ADR-0016 D2).
        let hidpi = VmSpec {
            vnc: VncEndpoint { host: "10.0.0.5".into(), port: 5900, password: None },
            agent: Some(AgentEndpoint { host: "10.0.0.5".into(), port: 8648 }),
            platform: "macos".into(),
            logical: Some(LogicalSize { width: 1920, height: 1080 }),
        };
        let path = dir.path().join("hidpi.json");
        hidpi.write_atomic(&path).unwrap();
        assert_eq!(VmSpec::load(&path).unwrap(), hidpi);

        // The default 1× path omits `logical` entirely — byte-identical to a
        // pre-HiDPI spec, and an old spec with no `logical` key still loads.
        let one_x = VmSpec {
            vnc: VncEndpoint { host: "localhost".into(), port: 5900, password: None },
            agent: None,
            platform: "linux".into(),
            logical: None,
        };
        let json = serde_json::to_string(&one_x).unwrap();
        assert!(!json.contains("logical"), "1× spec must not serialize logical: {json}");
        assert_eq!(serde_json::from_str::<VmSpec>(&json).unwrap(), one_x);
    }

    #[test]
    fn deserializes_a_swift_shaped_spec() {
        // Exactly the shape `VMSpec.swift` emits (sorted, pretty).
        let json = r#"{
          "agent": { "host": "localhost", "port": 51234 },
          "platform": "linux",
          "vnc": { "host": "localhost", "password": "testanyware", "port": 5900 }
        }"#;
        let spec: VmSpec = serde_json::from_str(json).unwrap();
        assert_eq!(spec.platform, "linux");
        assert_eq!(spec.vnc.port, 5900);
        assert_eq!(spec.agent.unwrap().port, 51234);
        // A Swift-shaped spec predates the HiDPI field; it loads as 1×.
        assert_eq!(spec.logical, None);
    }

    #[test]
    fn agentless_spec_round_trips() {
        let spec = VmSpec {
            vnc: VncEndpoint { host: "localhost".into(), port: 5900, password: None },
            agent: None,
            platform: "windows".into(),
            logical: None,
        };
        let json = serde_json::to_string(&spec).unwrap();
        assert!(!json.contains("agent"), "absent agent must not serialize: {json}");
        assert_eq!(serde_json::from_str::<VmSpec>(&json).unwrap(), spec);
    }

    #[cfg(unix)]
    #[test]
    fn written_file_is_mode_0600() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("v.json");
        let spec = VmSpec {
            vnc: VncEndpoint { host: "h".into(), port: 1, password: None },
            agent: None,
            platform: "linux".into(),
            logical: None,
        };
        spec.write_atomic(&path).unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
