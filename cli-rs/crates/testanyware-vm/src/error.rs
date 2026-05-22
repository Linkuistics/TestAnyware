//! `VmError` — the crate error type, mapped to contract §4 codes.

use serde_json::{json, Value};

/// Errors produced by the VM lifecycle. Each variant maps 1:1 to a §4
/// error code and a §5 exit code.
#[derive(Debug, thiserror::Error)]
pub enum VmError {
    #[error("/dev/kvm is not readable+writable at {path}")]
    KvmPermissionDenied { path: String },

    #[error("swtpm is not installed; it is required for Windows guests")]
    SwtpmMissing,

    #[error("UEFI firmware not found at {path}")]
    UefiNotFound { path: String },

    #[error("QEMU failed: {detail}")]
    QemuFailed { detail: String },

    #[error("could not discover the agent port via the QEMU monitor")]
    MonitorDiscoveryFailed,

    #[error("failed to spawn a child process: {detail}")]
    SpawnFailed { detail: String },

    #[error("golden image '{name}' not found")]
    GoldenNotFound { name: String },

    #[error("golden image '{name}' is in use by running clones (PIDs {clone_pids:?})")]
    GoldenInUse { name: String, clone_pids: Vec<i32> },

    #[error("no VM found for id '{id}'")]
    VmNotFound { id: String },

    #[error("VM '{id}' did not stop cleanly")]
    VmStopFailed { id: String },

    #[error("no backend can serve platform '{platform}' (tart support is a later task)")]
    BackendUnsupported { platform: String },

    #[error("unknown platform '{value}' (expected macos, linux, or windows)")]
    InvalidPlatform { value: String },

    /// A local filesystem error. The offending path (when there is a
    /// single one) is embedded in the message string rather than carried
    /// in `details.path`: many I/O failure sites here — serialization,
    /// fd duplication — have no single path, so a structured `path`
    /// field would be absent or misleading. `details()` returns null for
    /// this variant by design. (Contract §4.6 documents `details.path`
    /// "where useful"; §3.4 makes per-code detail keys optional.)
    #[error("I/O error: {0}")]
    Io(String),
}

impl VmError {
    /// Stable contract §4 code surfaced in `--json` output.
    pub fn code(&self) -> &'static str {
        match self {
            VmError::KvmPermissionDenied { .. } => "KVM_PERMISSION_DENIED",
            VmError::SwtpmMissing => "SWTPM_MISSING",
            VmError::UefiNotFound { .. } => "UEFI_NOT_FOUND",
            VmError::QemuFailed { .. } | VmError::MonitorDiscoveryFailed => "QEMU_FAILED",
            VmError::SpawnFailed { .. } => "SPAWN_FAILED",
            VmError::GoldenNotFound { .. } => "GOLDEN_NOT_FOUND",
            VmError::GoldenInUse { .. } => "GOLDEN_IN_USE",
            VmError::VmNotFound { .. } => "VM_NOT_FOUND",
            VmError::VmStopFailed { .. } => "VM_STOP_FAILED",
            VmError::BackendUnsupported { .. } => "VM_BACKEND_UNSUPPORTED",
            VmError::InvalidPlatform { .. } => "INVALID_PLATFORM",
            VmError::Io(_) => "IO_ERROR",
        }
    }

    /// §5 process exit code.
    pub fn exit_code(&self) -> i32 {
        match self {
            VmError::KvmPermissionDenied { .. } => 4,
            VmError::UefiNotFound { .. }
            | VmError::GoldenNotFound { .. }
            | VmError::VmNotFound { .. } => 3,
            VmError::GoldenInUse { .. } => 5,
            VmError::InvalidPlatform { .. } => 2,
            _ => 1,
        }
    }

    /// Actionable remediation string (contract §4 / §9.5).
    pub fn remediation(&self) -> Option<String> {
        match self {
            VmError::KvmPermissionDenied { .. } => Some(
                "Add yourself to the kvm group: `sudo usermod -aG kvm $USER`, \
                 then log out and back in."
                    .into(),
            ),
            VmError::SwtpmMissing => Some(
                "Install swtpm: `apt install swtpm swtpm-tools` on Linux, \
                 `brew install swtpm` on macOS."
                    .into(),
            ),
            VmError::GoldenInUse { .. } => {
                Some("Stop the running clones first, or re-run with --force.".into())
            }
            VmError::GoldenNotFound { .. } => {
                Some("Run `testanyware vm list` to see available golden images.".into())
            }
            VmError::VmNotFound { .. } => {
                Some("Run `testanyware vm list` to see running VMs.".into())
            }
            VmError::BackendUnsupported { .. } => Some(
                "QEMU serves linux and windows guests. macOS guests use the tart \
                 backend, which is not yet ported to the Rust CLI."
                    .into(),
            ),
            _ => None,
        }
    }

    /// `details` payload for the §3.4 JSON error envelope.
    pub fn details(&self) -> Value {
        match self {
            VmError::KvmPermissionDenied { path } | VmError::UefiNotFound { path } => {
                json!({ "path": path })
            }
            VmError::GoldenNotFound { name } => json!({ "golden_name": name }),
            VmError::GoldenInUse { name, clone_pids } => {
                json!({ "golden_name": name, "clone_pids": clone_pids })
            }
            VmError::VmNotFound { id } | VmError::VmStopFailed { id } => json!({ "vm_id": id }),
            VmError::BackendUnsupported { platform } | VmError::InvalidPlatform { value: platform } => {
                json!({ "platform": platform })
            }
            _ => Value::Null,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_match_contract_section_4() {
        assert_eq!(VmError::KvmPermissionDenied { path: "/dev/kvm".into() }.code(), "KVM_PERMISSION_DENIED");
        assert_eq!(VmError::SwtpmMissing.code(), "SWTPM_MISSING");
        assert_eq!(VmError::UefiNotFound { path: "/x".into() }.code(), "UEFI_NOT_FOUND");
        assert_eq!(VmError::QemuFailed { detail: "x".into() }.code(), "QEMU_FAILED");
        assert_eq!(VmError::MonitorDiscoveryFailed.code(), "QEMU_FAILED");
        assert_eq!(VmError::SpawnFailed { detail: "x".into() }.code(), "SPAWN_FAILED");
        assert_eq!(VmError::GoldenNotFound { name: "g".into() }.code(), "GOLDEN_NOT_FOUND");
        assert_eq!(VmError::GoldenInUse { name: "g".into(), clone_pids: vec![] }.code(), "GOLDEN_IN_USE");
        assert_eq!(VmError::VmNotFound { id: "v".into() }.code(), "VM_NOT_FOUND");
        assert_eq!(VmError::VmStopFailed { id: "v".into() }.code(), "VM_STOP_FAILED");
        assert_eq!(VmError::BackendUnsupported { platform: "macos".into() }.code(), "VM_BACKEND_UNSUPPORTED");
        assert_eq!(VmError::InvalidPlatform { value: "bsd".into() }.code(), "INVALID_PLATFORM");
        assert_eq!(VmError::Io("disk full".into()).code(), "IO_ERROR");
    }

    #[test]
    fn exit_codes_match_contract_section_5() {
        // §5: 3 = not-found family, 4 = permission, 5 = conflict, 2 = usage, 1 = generic.
        assert_eq!(VmError::KvmPermissionDenied { path: "/dev/kvm".into() }.exit_code(), 4);
        assert_eq!(VmError::SwtpmMissing.exit_code(), 1);
        assert_eq!(VmError::UefiNotFound { path: "/x".into() }.exit_code(), 3);
        assert_eq!(VmError::GoldenNotFound { name: "g".into() }.exit_code(), 3);
        assert_eq!(VmError::VmNotFound { id: "v".into() }.exit_code(), 3);
        assert_eq!(VmError::GoldenInUse { name: "g".into(), clone_pids: vec![1] }.exit_code(), 5);
        assert_eq!(VmError::InvalidPlatform { value: "x".into() }.exit_code(), 2);
        assert_eq!(VmError::QemuFailed { detail: "x".into() }.exit_code(), 1);
    }

    #[test]
    fn kvm_remediation_names_the_usermod_command() {
        let r = VmError::KvmPermissionDenied { path: "/dev/kvm".into() }
            .remediation()
            .expect("kvm error has remediation");
        assert!(r.contains("usermod -aG kvm"), "remediation must name the fix: {r}");
    }

    #[test]
    fn swtpm_remediation_names_both_package_managers() {
        let r = VmError::SwtpmMissing.remediation().expect("swtpm error has remediation");
        assert!(r.contains("apt install swtpm"), "remediation names apt: {r}");
        assert!(r.contains("brew install swtpm"), "remediation names brew: {r}");
    }

    #[test]
    fn golden_in_use_details_carry_clone_pids() {
        let d = VmError::GoldenInUse { name: "g".into(), clone_pids: vec![41, 42] }.details();
        assert_eq!(d["clone_pids"], serde_json::json!([41, 42]));
    }

    #[test]
    fn details_carry_path_for_kvm_and_uefi() {
        assert_eq!(VmError::KvmPermissionDenied { path: "/dev/kvm".into() }.details()["path"], "/dev/kvm");
        assert_eq!(VmError::UefiNotFound { path: "/x".into() }.details()["path"], "/x");
    }

    #[test]
    fn details_carry_vm_id_for_vm_errors() {
        assert_eq!(VmError::VmNotFound { id: "v".into() }.details()["vm_id"], "v");
        assert_eq!(VmError::VmStopFailed { id: "v".into() }.details()["vm_id"], "v");
    }

    #[test]
    fn details_carry_platform_for_backend_and_invalid_platform() {
        assert_eq!(VmError::BackendUnsupported { platform: "macos".into() }.details()["platform"], "macos");
        assert_eq!(VmError::InvalidPlatform { value: "bsd".into() }.details()["platform"], "bsd");
    }
}
