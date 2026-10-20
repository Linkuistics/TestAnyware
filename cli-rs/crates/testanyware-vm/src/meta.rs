//! `VmMeta` — the private lifecycle sidecar at `<vms>/<id>.meta.json`.
//!
//! Port of `VMMeta.swift`. The CLI never consumes this; `vm stop` reads
//! it to tear the VM down. JSON keys match the Swift `CodingKeys`
//! (`clone_dir`, `viewer_window_id`) so a VM started by either CLI can be
//! stopped by the other for the parallel-tooling period.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::VmError;
use crate::identity::ProcessIdentity;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VmTool {
    Tart,
    Qemu,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VmMeta {
    pub id: String,
    pub tool: VmTool,
    pub pid: i32,
    #[serde(rename = "clone_dir", default, skip_serializing_if = "Option::is_none")]
    pub clone_dir: Option<String>,
    #[serde(rename = "viewer_window_id", default, skip_serializing_if = "Option::is_none")]
    pub viewer_window_id: Option<String>,
    /// Pid of the VNC keeper that `vm start` spawned for a tart guest
    /// (ADR-0018). Absent for QEMU guests and for a meta written before the
    /// keeper existed.
    #[serde(rename = "keeper_pid", default, skip_serializing_if = "Option::is_none")]
    pub keeper_pid: Option<i32>,
    /// The identity of the process at `pid`, read when it started. Absent in a meta written
    /// before identities were recorded: such a VM gets streams only, and no signal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity: Option<ProcessIdentity>,
    /// The identity of the process at `keeper_pid`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keeper_identity: Option<ProcessIdentity>,
    /// Pid of the swtpm of a QEMU guest that has a TPM. swtpm ends when its QEMU ends.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub swtpm_pid: Option<i32>,
    /// The identity of the process at `swtpm_pid`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub swtpm_identity: Option<ProcessIdentity>,
    /// The directory of a QEMU guest's monitor socket and swtpm control socket. It sits under
    /// the `$TMPDIR` of the starter, so nobody can derive it from the VM id alone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_dir: Option<String>,
}

impl VmMeta {
    /// A meta that records the id, the tool and the pid, and no identity or session directory.
    pub fn new(id: impl Into<String>, tool: VmTool, pid: i32) -> Self {
        Self {
            id: id.into(),
            tool,
            pid,
            clone_dir: None,
            viewer_window_id: None,
            keeper_pid: None,
            identity: None,
            keeper_identity: None,
            swtpm_pid: None,
            swtpm_identity: None,
            session_dir: None,
        }
    }

    pub fn load(path: &Path) -> Result<Self, VmError> {
        let bytes = std::fs::read(path).map_err(|e| VmError::Io(format!("{}: {e}", path.display())))?;
        serde_json::from_slice(&bytes).map_err(|e| VmError::Io(format!("{}: {e}", path.display())))
    }

    pub fn write_atomic(&self, path: &Path) -> Result<(), VmError> {
        let json = serde_json::to_vec_pretty(self)
            .map_err(|e| VmError::Io(format!("serialize meta: {e}")))?;
        crate::spec::write_atomic_0600(path, &json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("v.meta.json");
        let meta = VmMeta {
            id: "testanyware-deadbeef".into(),
            tool: VmTool::Qemu,
            pid: 4242,
            clone_dir: Some("/d/clones/testanyware-deadbeef".into()),
            viewer_window_id: None,
            keeper_pid: None,
            identity: None,
            keeper_identity: None,
            swtpm_pid: None,
            swtpm_identity: None,
            session_dir: None,
        };
        meta.write_atomic(&path).unwrap();
        assert_eq!(VmMeta::load(&path).unwrap(), meta);
    }

    #[test]
    fn deserializes_a_swift_shaped_qemu_meta() {
        // Exactly the shape `vm-start.sh` / `VMMeta.swift` emits.
        let json = r#"{
          "clone_dir": "/Users/x/.local/share/testanyware/clones/testanyware-abcd1234",
          "id": "testanyware-abcd1234",
          "pid": 9876,
          "tool": "qemu"
        }"#;
        let meta: VmMeta = serde_json::from_str(json).unwrap();
        assert_eq!(meta.tool, VmTool::Qemu);
        assert_eq!(meta.pid, 9876);
        assert_eq!(meta.clone_dir.as_deref(),
            Some("/Users/x/.local/share/testanyware/clones/testanyware-abcd1234"));
        assert_eq!(meta.viewer_window_id, None);
    }

    #[test]
    fn tool_serializes_lowercase() {
        let json = serde_json::to_string(&VmTool::Qemu).unwrap();
        assert_eq!(json, "\"qemu\"");
        assert_eq!(serde_json::to_string(&VmTool::Tart).unwrap(), "\"tart\"");
    }

    #[test]
    fn tart_meta_round_trips_without_a_clone_dir() {
        // tart manages its own storage under ~/.tart, so a tart meta
        // carries no `clone_dir` — `vm start --platform macos` writes this
        // shape and `vm stop` must read it back. (Regression guard for the
        // 010-tart-runner leaf, which makes `tool: "tart"` writable.)
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("v.meta.json");
        let meta = VmMeta {
            id: "testanyware-abcd1234".into(),
            tool: VmTool::Tart,
            pid: 5151,
            clone_dir: None,
            viewer_window_id: None,
            keeper_pid: None,
            identity: None,
            keeper_identity: None,
            swtpm_pid: None,
            swtpm_identity: None,
            session_dir: None,
        };
        meta.write_atomic(&path).unwrap();
        let loaded = VmMeta::load(&path).unwrap();
        assert_eq!(loaded, meta);
        assert_eq!(loaded.tool, VmTool::Tart);
        // An absent clone_dir must not serialize (skip_serializing_if).
        let json = serde_json::to_string(&meta).unwrap();
        assert!(!json.contains("clone_dir"), "tart meta must omit clone_dir: {json}");
    }

    #[test]
    fn key_names_match_swift_coding_keys() {
        let meta = VmMeta {
            clone_dir: Some("/c".into()),
            viewer_window_id: Some("w".into()),
            keeper_pid: Some(7),
            ..VmMeta::new("v", VmTool::Qemu, 1)
        };
        let json = serde_json::to_string(&meta).unwrap();
        assert!(json.contains("\"clone_dir\""), "snake_case clone_dir: {json}");
        assert!(json.contains("\"viewer_window_id\""), "snake_case viewer id: {json}");
        assert!(json.contains("\"keeper_pid\":7"), "snake_case keeper pid: {json}");
    }

    #[test]
    fn keeper_pid_round_trips_and_is_omitted_when_absent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("v.meta.json");
        let mut meta = VmMeta {
            id: "testanyware-abcd1234".into(),
            tool: VmTool::Tart,
            pid: 5151,
            clone_dir: None,
            viewer_window_id: None,
            keeper_pid: Some(6262),
            identity: None,
            keeper_identity: None,
            swtpm_pid: None,
            swtpm_identity: None,
            session_dir: None,
        };
        meta.write_atomic(&path).unwrap();
        assert_eq!(VmMeta::load(&path).unwrap().keeper_pid, Some(6262));

        meta.keeper_pid = None;
        let json = serde_json::to_string(&meta).unwrap();
        assert!(!json.contains("keeper_pid"), "an absent keeper pid must not serialize: {json}");
    }

    #[test]
    fn a_meta_written_before_the_keeper_still_loads() {
        // The shape `vm start --platform macos` wrote before ADR-0018.
        let json = r#"{"id":"testanyware-abcd1234","pid":5151,"tool":"tart"}"#;
        let meta: VmMeta = serde_json::from_str(json).unwrap();
        assert_eq!(meta.tool, VmTool::Tart);
        assert_eq!(meta.keeper_pid, None);
    }

    fn identity(start_time: u64) -> ProcessIdentity {
        ProcessIdentity { start_time, boot_session: "6f1c2a4e-boot".into() }
    }

    #[test]
    fn the_identity_of_every_process_round_trips_with_the_session_dir() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("v.meta.json");
        let meta = VmMeta {
            clone_dir: Some("/d/clones/v".into()),
            keeper_pid: Some(7),
            identity: Some(identity(100)),
            keeper_identity: Some(identity(101)),
            swtpm_pid: Some(8),
            swtpm_identity: Some(identity(102)),
            session_dir: Some("/tmp/testanyware-v".into()),
            ..VmMeta::new("v", VmTool::Qemu, 1)
        };
        meta.write_atomic(&path).unwrap();
        assert_eq!(VmMeta::load(&path).unwrap(), meta);

        let json = serde_json::to_string(&meta).unwrap();
        for key in [
            "\"identity\"", "\"keeper_identity\"", "\"swtpm_pid\"", "\"swtpm_identity\"",
            "\"session_dir\"", "\"start_time\"", "\"boot_session\"",
        ] {
            assert!(json.contains(key), "{key} is in the meta: {json}");
        }
    }

    #[test]
    fn a_meta_without_an_identity_serializes_none_of_the_new_keys() {
        let json = serde_json::to_string(&VmMeta::new("v", VmTool::Tart, 1)).unwrap();
        for key in ["identity", "swtpm", "session_dir", "start_time", "boot_session"] {
            assert!(!json.contains(key), "an absent {key} must not serialize: {json}");
        }
    }

    #[test]
    fn a_meta_written_before_identities_still_loads_with_none() {
        // The shape of a QEMU guest that the Host CLI started before this field existed.
        let json = r#"{"clone_dir":"/d/clones/v","id":"v","pid":9876,"tool":"qemu"}"#;
        let meta: VmMeta = serde_json::from_str(json).unwrap();
        assert_eq!(meta.pid, 9876);
        assert_eq!(meta.identity, None);
        assert_eq!(meta.keeper_identity, None);
        assert_eq!(meta.swtpm_pid, None);
        assert_eq!(meta.swtpm_identity, None);
        assert_eq!(meta.session_dir, None);
    }
}
