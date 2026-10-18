//! The TPM decision: the one place that answers whether a QEMU guest gets a
//! TPM. Pure — it reads no host state, so its table is unit-testable without
//! QEMU or swtpm.
//!
//! Every TPM site consumes this answer: the swtpm preflight, the copy of the
//! golden's TPM state, the swtpm launch, and the `-chardev` / `-tpmdev` /
//! `-device` triple in both QEMU argument vectors (`vm start` and Windows
//! golden creation). No site re-derives "does this guest need a TPM?".
//!
//! The decision is shaped to grow. A host check will add the answer *omit,
//! because the host cannot supply one* as another [`OmitReason`], and the
//! answer *refuse* as the `Err` of [`decide_tpm`]. Callers already take
//! `decide_tpm(..)?` and ask [`TpmDecision::attaches`], so neither addition
//! reshapes them.

use std::path::Path;

use crate::error::VmError;
use crate::lifecycle::Platform;

/// Whether a guest gets a TPM, and if not, why.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TpmDecision {
    /// Start swtpm, copy the golden's TPM state, and wire the TPM device.
    Attach,
    /// Do none of those.
    Omit(OmitReason),
}

/// Why a guest goes without a TPM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OmitReason {
    /// The guest's platform does not use one (anything but Windows).
    GuestDoesNotNeedOne,
}

impl TpmDecision {
    /// `true` when every TPM site must run. The one question callers ask.
    pub fn attaches(self) -> bool {
        matches!(self, TpmDecision::Attach)
    }
}

/// Decide whether a `platform` guest gets a TPM. Windows 11 requires one; no
/// other guest does.
pub fn decide_tpm(platform: Platform) -> Result<TpmDecision, VmError> {
    Ok(match platform {
        Platform::Windows => TpmDecision::Attach,
        Platform::Macos | Platform::Linux => TpmDecision::Omit(OmitReason::GuestDoesNotNeedOne),
    })
}

/// The QEMU arguments that wire the TPM device to swtpm's control socket, or
/// nothing when `tpm_socket` is `None`. `tpm-tis-device` is the only TPM front
/// end the `virt` machine can take.
pub(crate) fn device_args(tpm_socket: Option<&Path>) -> Vec<String> {
    let Some(socket) = tpm_socket else {
        return Vec::new();
    };
    vec![
        "-chardev".into(), format!("socket,id=chrtpm,path={}", socket.display()),
        "-tpmdev".into(), "emulator,id=tpm0,chardev=chrtpm".into(),
        "-device".into(), "tpm-tis-device,tpmdev=tpm0".into(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decision_table_attaches_for_windows_only() {
        assert_eq!(decide_tpm(Platform::Windows).unwrap(), TpmDecision::Attach);
        assert_eq!(
            decide_tpm(Platform::Linux).unwrap(),
            TpmDecision::Omit(OmitReason::GuestDoesNotNeedOne),
        );
        assert_eq!(
            decide_tpm(Platform::Macos).unwrap(),
            TpmDecision::Omit(OmitReason::GuestDoesNotNeedOne),
        );
    }

    #[test]
    fn only_attach_attaches() {
        assert!(TpmDecision::Attach.attaches());
        assert!(!TpmDecision::Omit(OmitReason::GuestDoesNotNeedOne).attaches());
    }

    #[test]
    fn device_args_are_the_chardev_tpmdev_device_triple() {
        let args = device_args(Some(Path::new("/s/swtpm-sock")));
        assert_eq!(args, vec![
            "-chardev", "socket,id=chrtpm,path=/s/swtpm-sock",
            "-tpmdev", "emulator,id=tpm0,chardev=chrtpm",
            "-device", "tpm-tis-device,tpmdev=tpm0",
        ]);
    }

    #[test]
    fn device_args_are_empty_without_a_socket() {
        assert!(device_args(None).is_empty());
    }
}
