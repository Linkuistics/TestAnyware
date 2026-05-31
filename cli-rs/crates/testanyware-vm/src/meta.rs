//! `VmMeta` — the private lifecycle sidecar at `<vms>/<id>.meta.json`.
//!
//! Port of `VMMeta.swift`. The CLI never consumes this; `vm stop` reads
//! it to tear the VM down. JSON keys match the Swift `CodingKeys`
//! (`clone_dir`, `viewer_window_id`) so a VM started by either CLI can be
//! stopped by the other for the parallel-tooling period.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::VmError;

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
}

impl VmMeta {
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
            id: "v".into(), tool: VmTool::Qemu, pid: 1,
            clone_dir: Some("/c".into()), viewer_window_id: Some("w".into()),
        };
        let json = serde_json::to_string(&meta).unwrap();
        assert!(json.contains("\"clone_dir\""), "snake_case clone_dir: {json}");
        assert!(json.contains("\"viewer_window_id\""), "snake_case viewer id: {json}");
    }
}
