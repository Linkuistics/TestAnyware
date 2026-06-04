//! Host-specific QEMU details, selected by `#[cfg]`.
//!
//! Per the 2026-05-22 per-platform-facilities decision, the Rust port
//! uses the best native accelerator per host (HVF on macOS, KVM on
//! Linux) rather than a lowest-common-denominator engine. Guest
//! architecture follows the host: goldens are built per-host by
//! `vm-create-golden-*.sh`, and KVM/HVF only accelerate same-arch guests.

use std::path::PathBuf;
// `Path` is only used by the macOS UEFI-candidate resolver below.
#[cfg(target_os = "macos")]
use std::path::Path;

/// Host-resolved QEMU launch parameters.
#[derive(Debug, Clone)]
pub struct QemuProfile {
    /// `qemu-system-*` binary name (resolved on PATH at launch time).
    pub qemu_binary: &'static str,
    /// `-accel` value.
    pub accelerator: &'static str,
    /// `-machine` value.
    pub machine: &'static str,
    /// `-cpu` value.
    pub cpu: &'static str,
    /// Ordered UEFI code-firmware candidates; the first that exists wins.
    pub uefi_code_candidates: Vec<PathBuf>,
}

/// The profile for the current host. macOS-aarch64 is faithful to the
/// Swift `QEMURunner`; the Linux branches follow the same device model
/// with KVM + the host architecture's firmware.
pub fn host_profile() -> QemuProfile {
    #[cfg(target_os = "macos")]
    {
        // macOS hosts are Apple Silicon: aarch64 guests under HVF.
        QemuProfile {
            qemu_binary: "qemu-system-aarch64",
            accelerator: "hvf",
            machine: "virt,highmem=on,gic-version=3",
            cpu: "host",
            uefi_code_candidates: macos_uefi_candidates(),
        }
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        QemuProfile {
            qemu_binary: "qemu-system-x86_64",
            accelerator: "kvm",
            machine: "q35",
            cpu: "host",
            uefi_code_candidates: vec![
                PathBuf::from("/usr/share/OVMF/OVMF_CODE.fd"),
                PathBuf::from("/usr/share/edk2/x64/OVMF_CODE.fd"),
                PathBuf::from("/usr/share/edk2-ovmf/x64/OVMF_CODE.fd"),
                PathBuf::from("/usr/share/qemu/edk2-x86_64-code.fd"),
            ],
        }
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    {
        QemuProfile {
            qemu_binary: "qemu-system-aarch64",
            accelerator: "kvm",
            machine: "virt,gic-version=3",
            cpu: "host",
            uefi_code_candidates: vec![
                PathBuf::from("/usr/share/AAVMF/AAVMF_CODE.fd"),
                PathBuf::from("/usr/share/edk2/aarch64/QEMU_CODE.fd"),
                PathBuf::from("/usr/share/qemu/edk2-aarch64-code.fd"),
            ],
        }
    }
    #[cfg(not(any(
        target_os = "macos",
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
    )))]
    {
        // Windows host support is backlog task 14; give a usable default
        // so the crate still compiles on unanticipated targets.
        QemuProfile {
            qemu_binary: "qemu-system-x86_64",
            accelerator: "tcg",
            machine: "q35",
            cpu: "max",
            uefi_code_candidates: vec![],
        }
    }
}

/// macOS UEFI candidates: derived from the resolved `qemu-system-aarch64`
/// install prefix (`<prefix>/share/qemu/edk2-aarch64-code.fd`), as the
/// Swift `QEMURunner.start` does, plus the standard Homebrew location.
#[cfg(target_os = "macos")]
fn macos_uefi_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(bin) = which("qemu-system-aarch64") {
        // <prefix>/bin/qemu-system-aarch64 → <prefix>/share/qemu/...
        if let Some(prefix) = bin.parent().and_then(Path::parent) {
            out.push(prefix.join("share/qemu/edk2-aarch64-code.fd"));
        }
    }
    out.push(PathBuf::from("/opt/homebrew/share/qemu/edk2-aarch64-code.fd"));
    out
}

/// Return the first existing UEFI code firmware among `candidates`.
pub fn resolve_uefi_code(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates.iter().find(|p| p.is_file()).cloned()
}

/// Resolve `name` to an absolute path by scanning `$PATH`. On macOS,
/// `/opt/homebrew/bin` and `/usr/local/bin` are appended so the qemu
/// toolchain resolves even when the CLI runs from a scrubbed environment.
pub fn which(name: &str) -> Option<PathBuf> {
    // `mut` is consumed only by the macOS Homebrew-dir push below; on other
    // hosts the `PATH` list is used as-is.
    #[cfg_attr(not(target_os = "macos"), allow(unused_mut))]
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    #[cfg(target_os = "macos")]
    {
        dirs.push(PathBuf::from("/opt/homebrew/bin"));
        dirs.push(PathBuf::from("/usr/local/bin"));
    }
    for dir in dirs {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // Gated to the host families with a real profile: the
    // `not(any(...))` fallback branch is a compile-only stub with no
    // accelerator-backed config and an empty UEFI candidate list, so the
    // "must list UEFI candidates" assertion only holds here.
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    fn host_profile_is_internally_consistent() {
        let p = host_profile();
        assert!(p.qemu_binary.starts_with("qemu-system-"), "binary: {}", p.qemu_binary);
        assert!(!p.accelerator.is_empty(), "accelerator must be set");
        assert!(!p.machine.is_empty(), "machine must be set");
        assert!(!p.uefi_code_candidates.is_empty(), "must list UEFI candidates");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_profile_uses_hvf() {
        let p = host_profile();
        assert_eq!(p.accelerator, "hvf");
        assert_eq!(p.qemu_binary, "qemu-system-aarch64");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_profile_uses_kvm() {
        let p = host_profile();
        assert_eq!(p.accelerator, "kvm");
    }

    #[test]
    fn resolve_uefi_code_picks_the_first_existing_candidate() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("edk2-code.fd");
        std::fs::write(&real, b"firmware").unwrap();
        let candidates = vec![
            dir.path().join("missing-a.fd"),
            real.clone(),
            dir.path().join("missing-b.fd"),
        ];
        assert_eq!(resolve_uefi_code(&candidates), Some(real));
    }

    #[test]
    fn resolve_uefi_code_is_none_when_no_candidate_exists() {
        let dir = tempfile::tempdir().unwrap();
        let candidates = vec![dir.path().join("nope.fd")];
        assert_eq!(resolve_uefi_code(&candidates), None);
    }

    #[test]
    fn which_finds_a_known_binary() {
        // `sh` is on PATH on every supported host.
        assert!(which("sh").is_some(), "which(sh) should resolve");
        assert!(which("a-binary-that-does-not-exist-xyzzy").is_none());
    }
}
