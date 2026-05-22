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

/// Public per-VM spec. `platform` is a plain string (`macos`/`linux`/
/// `windows`) so it round-trips with `ConnectionSpec.platform`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VmSpec {
    pub vnc: VncEndpoint,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<AgentEndpoint>,
    pub platform: String,
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
        };
        spec.write_atomic(&path).unwrap();
        assert_eq!(VmSpec::load(&path).unwrap(), spec);
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
    }

    #[test]
    fn agentless_spec_round_trips() {
        let spec = VmSpec {
            vnc: VncEndpoint { host: "localhost".into(), port: 5900, password: None },
            agent: None,
            platform: "windows".into(),
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
        };
        spec.write_atomic(&path).unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
