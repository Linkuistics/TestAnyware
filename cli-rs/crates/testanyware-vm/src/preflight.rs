//! Host preflight checks for the QEMU backend.

use crate::error::VmError;
use crate::qemu_profile::which;

/// Verify `/dev/kvm` is readable and writable (Linux only). macOS uses
/// HVF and Windows uses WHPX, so this is a no-op off Linux. A missing or
/// unwritable `/dev/kvm` is the most common first-run failure on Linux
/// hosts; the remediation names `usermod -aG kvm $USER`.
#[cfg(target_os = "linux")]
pub fn check_kvm() -> Result<(), VmError> {
    const KVM: &str = "/dev/kvm";
    if !std::path::Path::new(KVM).exists() {
        return Err(VmError::KvmPermissionDenied { path: KVM.into() });
    }
    match std::fs::OpenOptions::new().read(true).write(true).open(KVM) {
        Ok(_) => Ok(()),
        Err(_) => Err(VmError::KvmPermissionDenied { path: KVM.into() }),
    }
}

#[cfg(not(target_os = "linux"))]
pub fn check_kvm() -> Result<(), VmError> {
    Ok(())
}

/// Verify swtpm is installed. Required for Windows guests (TPM 2.0
/// socket). The remediation names the package on both Linux and macOS.
pub fn check_swtpm() -> Result<(), VmError> {
    swtpm_result(which("swtpm").is_some())
}

/// Map "is swtpm present?" to the preflight result. Split out from the
/// `which` lookup so the negative branch is unit-testable without
/// depending on whether the dev host happens to have swtpm installed
/// — note `which` also searches `/opt/homebrew/bin` on macOS, so a
/// scrubbed `PATH` does not reliably simulate absence.
fn swtpm_result(swtpm_present: bool) -> Result<(), VmError> {
    if swtpm_present {
        Ok(())
    } else {
        Err(VmError::SwtpmMissing)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn check_kvm_is_a_noop_off_linux() {
        // macOS uses HVF; there is no /dev/kvm to gate on.
        assert!(check_kvm().is_ok());
    }

    #[test]
    fn swtpm_result_maps_presence_to_outcome() {
        // Absent swtpm must yield SwtpmMissing; present must be Ok.
        // Tested via the pure helper so the result does not depend on
        // whether this host has swtpm installed.
        assert!(matches!(swtpm_result(false), Err(VmError::SwtpmMissing)));
        assert!(swtpm_result(true).is_ok());
    }

    #[test]
    fn check_swtpm_returns_a_result_without_panicking() {
        // The real check resolves swtpm via PATH (+ Homebrew dirs on
        // macOS); whatever the host state it must return a Result, not
        // panic. Ok when installed, SwtpmMissing when not.
        let _ = check_swtpm();
    }
}
