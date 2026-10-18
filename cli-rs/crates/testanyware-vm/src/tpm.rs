//! The TPM decision: the one place that answers whether a QEMU guest gets a
//! TPM (ADR-0017). The decision itself is pure — it takes the guest's platform,
//! the accelerator and a *way to observe* the host — so its table is
//! unit-testable without QEMU or swtpm.
//!
//! Every TPM site consumes this answer: the swtpm launch, the copy of the
//! golden's TPM state, and the `-chardev` / `-tpmdev` / `-device` triple in
//! both QEMU argument vectors (`vm start` and Windows golden creation). The
//! swtpm rule lives here too, as the *refuse* answer (`Err(SwtpmMissing)`);
//! no site re-derives "does this guest need a TPM?" or "is swtpm there?".
//!
//! The host is observed lazily: [`decide_tpm`] calls its observer only for a
//! guest that needs a TPM, so a Linux or macOS guest never pays for a probe.

use std::path::PathBuf;

use crate::error::VmError;
use crate::lifecycle::Platform;
use crate::qemu_profile::{which, QemuProfile};

/// Whether a guest gets a TPM, and if not, why.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TpmDecision {
    /// Start swtpm, copy the golden's TPM state, and wire the TPM device.
    Attach {
        /// Wire the device with `ppi=off`. Set only under HVF with a QEMU that
        /// has the property: its 1 KiB PPI region aborts QEMU there (ADR-0017).
        ppi_off: bool,
    },
    /// Do none of those.
    Omit(OmitReason),
}

/// Why a guest goes without a TPM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OmitReason {
    /// The guest's platform does not use one (anything but Windows).
    GuestDoesNotNeedOne,
    /// The host's QEMU binary has no `tpm-tis-device` (an x86_64 binary, or a
    /// build without TPM support).
    HostCannotSupplyOne,
}

/// What the host's QEMU binary offers for the `virt` TPM front end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrontEnd {
    /// No `tpm-tis-device` in this binary.
    Absent,
    /// `tpm-tis-device` exists; `ppi` says whether it lists a `ppi` property
    /// (QEMU 11.1 and later).
    Present { ppi: bool },
}

/// Everything the decision needs to know about the host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostTpm {
    pub front_end: FrontEnd,
    pub swtpm_installed: bool,
}

/// The swtpm socket and the device options of an attached TPM, as one value so
/// that "PPI off, no TPM" cannot be written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TpmWiring {
    /// swtpm's control socket.
    pub socket: PathBuf,
    /// Add `ppi=off` to the device.
    pub ppi_off: bool,
}

impl TpmDecision {
    /// `true` when every TPM site must run. The one question callers ask.
    pub fn attaches(self) -> bool {
        matches!(self, TpmDecision::Attach { .. })
    }

    /// The device wiring for an attached TPM whose swtpm listens on `socket`;
    /// `None` when the decision omits the TPM.
    pub fn wiring(self, socket: PathBuf) -> Option<TpmWiring> {
        match self {
            TpmDecision::Attach { ppi_off } => Some(TpmWiring { socket, ppi_off }),
            TpmDecision::Omit(_) => None,
        }
    }
}

/// Decide whether a `platform` guest gets a TPM, in ADR-0017's order. Windows
/// 11 requires one; no other guest does, and for those `observe` is never
/// called. `accelerator` is the profile's `-accel` value.
pub fn decide_tpm(
    platform: Platform,
    accelerator: &str,
    observe: impl FnOnce() -> Result<HostTpm, VmError>,
) -> Result<TpmDecision, VmError> {
    if platform != Platform::Windows {
        return Ok(TpmDecision::Omit(OmitReason::GuestDoesNotNeedOne));
    }
    let host = observe()?;
    let ppi_listed = match host.front_end {
        FrontEnd::Absent => return Ok(TpmDecision::Omit(OmitReason::HostCannotSupplyOne)),
        FrontEnd::Present { ppi } => ppi,
    };
    if !host.swtpm_installed {
        return Err(VmError::SwtpmMissing);
    }
    Ok(TpmDecision::Attach { ppi_off: accelerator == "hvf" && ppi_listed })
}

/// [`decide_tpm`] against the real host: probe the profile's QEMU binary and
/// look for swtpm. The entry point `vm start`, `--dry-run` and Windows golden
/// creation call, once, before cloning or booting anything.
pub fn decide_tpm_on_host(platform: Platform, profile: &QemuProfile) -> Result<TpmDecision, VmError> {
    decide_tpm(platform, profile.accelerator, || observe_host(profile.qemu_binary))
}

/// Read the host's QEMU binary and swtpm. A QEMU that cannot be run is
/// `QemuFailed`, as the launch would report it — it is not "the host cannot
/// supply a TPM", which is for a QEMU that ran and lacks the device.
fn observe_host(qemu_binary: &str) -> Result<HostTpm, VmError> {
    let qemu = which(qemu_binary)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| qemu_binary.to_string());
    let output = std::process::Command::new(&qemu)
        .args(["-machine", "none", "-device", "tpm-tis-device,help"])
        .output()
        .map_err(|e| VmError::QemuFailed { detail: format!("{qemu}: {e}") })?;
    // QEMU exits 0 whether or not the device exists, and prints the property
    // list on stdout but the "not found" complaint on stderr: read both.
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let front_end = parse_front_end_probe(&text).ok_or_else(|| VmError::QemuFailed {
        detail: format!("{qemu} -device tpm-tis-device,help: unrecognised output: {}", text.trim()),
    })?;
    Ok(HostTpm { front_end, swtpm_installed: which("swtpm").is_some() })
}

/// Parse the text of `<qemu> -machine none -device tpm-tis-device,help`.
/// `None` means the text is neither a property list nor QEMU's "not found".
fn parse_front_end_probe(text: &str) -> Option<FrontEnd> {
    if text.lines().any(|l| l.trim_start().starts_with("tpm-tis-device options:")) {
        let ppi = text.lines().any(|l| l.trim_start().starts_with("ppi="));
        Some(FrontEnd::Present { ppi })
    } else if text.contains("Device 'tpm-tis-device' not found") {
        Some(FrontEnd::Absent)
    } else {
        None
    }
}

/// Where a clone's swtpm state comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TpmStateSource {
    /// Copy the golden's `<golden>-tpm` directory.
    CopyGolden,
    /// Start empty: the golden has none. Nothing on these goldens is sealed to
    /// the TPM, so a fresh state serves (a golden built where the TPM was
    /// omitted, started where it attaches).
    Fresh,
}

/// Choose the clone's swtpm state, given whether the golden has any.
pub fn tpm_state_source(golden_state_exists: bool) -> TpmStateSource {
    if golden_state_exists { TpmStateSource::CopyGolden } else { TpmStateSource::Fresh }
}

/// The QEMU arguments that wire the TPM device to swtpm's control socket, or
/// nothing without a `wiring`. `tpm-tis-device` is the only TPM front end the
/// `virt` machine can take.
pub(crate) fn device_args(wiring: Option<&TpmWiring>) -> Vec<String> {
    let Some(wiring) = wiring else {
        return Vec::new();
    };
    let ppi = if wiring.ppi_off { ",ppi=off" } else { "" };
    vec![
        "-chardev".into(), format!("socket,id=chrtpm,path={}", wiring.socket.display()),
        "-tpmdev".into(), "emulator,id=tpm0,chardev=chrtpm".into(),
        "-device".into(), format!("tpm-tis-device,tpmdev=tpm0{ppi}"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    /// The three captured probe texts (ADR-0017, `docs/research/windows-tpm-on-hvf.md`).
    const QEMU_11_1_2: &str = "tpm-tis-device options:\n  irq=<uint32>           -  (default: 5)\n  ppi=<bool>             - on/off (default: on)\n  sysbus-irq[0]=<link<irq>>\n  tpmdev=<str>           - ID of a tpm to use as a backend\n";
    const QEMU_11_0_1: &str = "tpm-tis-device options:\n  irq=<uint32>           -  (default: 5)\n  tpm-tis-mmio[0]=<link<irq>>\n  tpmdev=<str>           - ID of a tpm to use as a backend\n";
    const NOT_FOUND: &str = "qemu-system-x86_64: -device tpm-tis-device,help: Device 'tpm-tis-device' not found\n";

    fn host(front_end: FrontEnd, swtpm_installed: bool) -> HostTpm {
        HostTpm { front_end, swtpm_installed }
    }

    fn decide(platform: Platform, accel: &str, h: HostTpm) -> Result<TpmDecision, VmError> {
        decide_tpm(platform, accel, || Ok(h))
    }

    #[test]
    fn a_guest_that_needs_no_tpm_omits_without_observing() {
        for platform in [Platform::Linux, Platform::Macos] {
            let observed = Cell::new(false);
            let d = decide_tpm(platform, "hvf", || {
                observed.set(true);
                Ok(host(FrontEnd::Present { ppi: true }, true))
            })
            .unwrap();
            assert_eq!(d, TpmDecision::Omit(OmitReason::GuestDoesNotNeedOne));
            assert!(!observed.get(), "no probe runs for a guest that needs no TPM");
        }
    }

    #[test]
    fn windows_omits_when_the_front_end_is_absent() {
        let d = decide(Platform::Windows, "hvf", host(FrontEnd::Absent, true)).unwrap();
        assert_eq!(d, TpmDecision::Omit(OmitReason::HostCannotSupplyOne));
    }

    #[test]
    fn windows_attaches_with_ppi_off_under_hvf_when_ppi_is_listed() {
        let d = decide(Platform::Windows, "hvf", host(FrontEnd::Present { ppi: true }, true)).unwrap();
        assert_eq!(d, TpmDecision::Attach { ppi_off: true });
    }

    #[test]
    fn windows_attaches_without_ppi_off_when_ppi_is_not_listed() {
        let d = decide(Platform::Windows, "hvf", host(FrontEnd::Present { ppi: false }, true)).unwrap();
        assert_eq!(d, TpmDecision::Attach { ppi_off: false });
    }

    #[test]
    fn windows_keeps_the_default_ppi_outside_hvf() {
        for accel in ["kvm", "tcg", "whpx"] {
            let d = decide(Platform::Windows, accel, host(FrontEnd::Present { ppi: true }, true)).unwrap();
            assert_eq!(d, TpmDecision::Attach { ppi_off: false }, "{accel}");
        }
    }

    #[test]
    fn windows_without_swtpm_is_refused() {
        let err = decide(Platform::Windows, "hvf", host(FrontEnd::Present { ppi: true }, false)).unwrap_err();
        assert!(matches!(err, VmError::SwtpmMissing));
        assert_eq!(err.code(), "SWTPM_MISSING");
    }

    #[test]
    fn the_tables_order_puts_an_absent_front_end_before_the_swtpm_refusal() {
        // Without the device there is nothing for swtpm to back, so the host
        // cannot supply a TPM and the guest omits; swtpm is not asked for.
        let d = decide(Platform::Windows, "hvf", host(FrontEnd::Absent, false)).unwrap();
        assert_eq!(d, TpmDecision::Omit(OmitReason::HostCannotSupplyOne));
    }

    #[test]
    fn a_failed_observation_is_the_decisions_error() {
        let err = decide_tpm(Platform::Windows, "hvf", || {
            Err(VmError::QemuFailed { detail: "no qemu".into() })
        })
        .unwrap_err();
        assert!(matches!(err, VmError::QemuFailed { .. }));
    }

    #[test]
    fn only_attach_attaches() {
        assert!(TpmDecision::Attach { ppi_off: false }.attaches());
        assert!(TpmDecision::Attach { ppi_off: true }.attaches());
        assert!(!TpmDecision::Omit(OmitReason::GuestDoesNotNeedOne).attaches());
        assert!(!TpmDecision::Omit(OmitReason::HostCannotSupplyOne).attaches());
    }

    #[test]
    fn wiring_exists_only_for_attach_and_carries_the_ppi_choice() {
        let sock = || PathBuf::from("/s/swtpm-sock");
        assert_eq!(
            TpmDecision::Attach { ppi_off: true }.wiring(sock()),
            Some(TpmWiring { socket: sock(), ppi_off: true }),
        );
        assert_eq!(
            TpmDecision::Attach { ppi_off: false }.wiring(sock()),
            Some(TpmWiring { socket: sock(), ppi_off: false }),
        );
        assert_eq!(TpmDecision::Omit(OmitReason::HostCannotSupplyOne).wiring(sock()), None);
        assert_eq!(TpmDecision::Omit(OmitReason::GuestDoesNotNeedOne).wiring(sock()), None);
    }

    #[test]
    fn the_probe_parser_reads_the_three_captured_texts() {
        assert_eq!(parse_front_end_probe(QEMU_11_1_2), Some(FrontEnd::Present { ppi: true }));
        assert_eq!(parse_front_end_probe(QEMU_11_0_1), Some(FrontEnd::Present { ppi: false }));
        assert_eq!(parse_front_end_probe(NOT_FOUND), Some(FrontEnd::Absent));
    }

    #[test]
    fn the_probe_parser_rejects_text_that_is_neither() {
        assert_eq!(parse_front_end_probe(""), None);
        assert_eq!(parse_front_end_probe("qemu: Segmentation fault\n"), None);
    }

    #[test]
    fn a_missing_golden_state_starts_a_fresh_one() {
        assert_eq!(tpm_state_source(true), TpmStateSource::CopyGolden);
        assert_eq!(tpm_state_source(false), TpmStateSource::Fresh);
    }

    #[test]
    fn device_args_are_the_chardev_tpmdev_device_triple() {
        let wiring = TpmWiring { socket: "/s/swtpm-sock".into(), ppi_off: false };
        assert_eq!(device_args(Some(&wiring)), vec![
            "-chardev", "socket,id=chrtpm,path=/s/swtpm-sock",
            "-tpmdev", "emulator,id=tpm0,chardev=chrtpm",
            "-device", "tpm-tis-device,tpmdev=tpm0",
        ]);
    }

    #[test]
    fn device_args_add_ppi_off_exactly_when_the_wiring_says_so() {
        let wiring = TpmWiring { socket: "/s/swtpm-sock".into(), ppi_off: true };
        let args = device_args(Some(&wiring));
        assert_eq!(args[5], "tpm-tis-device,tpmdev=tpm0,ppi=off");
        assert_eq!(args.len(), 6);
    }

    #[test]
    fn device_args_are_empty_without_wiring() {
        assert!(device_args(None).is_empty());
    }
}
