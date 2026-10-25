//! `VmError` — the crate error type, mapped to contract §4 codes.

use std::fmt;
use std::time::Duration;

use serde_json::{json, Value};

use crate::adoption::Mismatch;
use crate::health::ProbeError;
use crate::legacy_stop::StopBlockers;

/// What a stop found when the pid in the meta did not name the recorded process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StopUnproven {
    pub pid: i32,
    pub mismatch: Mismatch,
    /// The records that the stop kept: the spec and the meta.
    pub records: Vec<String>,
}

/// The stage of a start that ran out of time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootStage {
    /// The backend never reported a VNC endpoint (tart's `vnc://` line).
    Vnc,
    /// The guest never got an IP address.
    GuestIp,
    /// The agent never became a Healthy agent.
    Agent,
    /// A provisioning VM never accepted SSH (`vm create-golden` only).
    Ssh,
}

impl BootStage {
    /// Stable token for `--json` details.
    pub fn as_str(self) -> &'static str {
        match self {
            BootStage::Vnc => "vnc",
            BootStage::GuestIp => "guest-ip",
            BootStage::Agent => "agent",
            BootStage::Ssh => "ssh",
        }
    }
}

impl fmt::Display for BootStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What a `VM_LIMIT_REACHED` knows: which platform's limit, its size, the VMs
/// holding the slots, and the backend's own words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LimitReached {
    pub platform: String,
    pub limit: u32,
    /// The VMs holding the slots, as the backend named them (possibly none).
    pub holders: Vec<String>,
    /// The backend's own line, verbatim — backends reword the framework's error. Empty when the
    /// refusal was the app's and no `tart run` ran.
    pub raw: String,
    /// What the admission queue of the TestAnyware app knows about this refusal.
    pub admission: AdmissionFacts,
}

/// Who refused a start that the limit stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RefusedBy {
    /// The app: its count was full, or a waiter stood ahead of the start.
    App,
    /// Virtualization.framework, through the `tart run` of the start.
    #[default]
    Framework,
}

impl RefusedBy {
    /// The token of the `refused_by` member in `--json` details.
    pub fn as_str(self) -> &'static str {
        match self {
            RefusedBy::App => "app",
            RefusedBy::Framework => "framework",
        }
    }
}

/// The members that the admission queue adds to the details of `VM_LIMIT_REACHED` (spec
/// "admission-queue": "The command surface"). A start that no app admitted has none of them,
/// and says that the framework refused.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AdmissionFacts {
    /// The request id, or `None` when no app took part.
    pub request_id: Option<String>,
    /// The place the start had, or would take again in the queue, from 1.
    pub place: Option<u32>,
    /// The seconds the start waited in the queue.
    pub waited_secs: u64,
    pub refused_by: RefusedBy,
    /// The count of the app showed a free slot, or contention held the try back, so a guest that
    /// the app cannot see may hold a slot. The remedy names that cause. It is no `details`
    /// member.
    pub unseen_possible: bool,
    /// The app got no usable answer of tart for this start: tart does not answer, or no answer
    /// was newer than the join of the start. The remedy says so. It is no `details` member.
    pub tart_silence: Option<TartSilence>,
}

/// What the app knows about a tart that did not serve a start (spec "admission-queue": "The
/// command surface").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TartSilence {
    /// The seconds from the last answer of tart to the refusal, or `None` when tart has not
    /// answered in this run of the app.
    pub since_last_answer_secs: Option<u64>,
}

impl fmt::Display for LimitReached {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} guest limit reached: {} VMs are already running ({})",
            self.platform,
            self.limit,
            self.holders.join(", ")
        )
    }
}

/// A proven reason a stage could not finish, when the timeout alone is not
/// the whole story.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootCause {
    /// A platform tool connected to the guest's agent port, but this process
    /// was refused the route: macOS Local Network privacy.
    LocalNetworkDenied,
}

impl BootCause {
    /// Stable token for `--json` details.
    pub fn as_str(self) -> &'static str {
        match self {
            BootCause::LocalNetworkDenied => "local_network_denied",
        }
    }
}

/// What a `VM_BOOT_TIMEOUT` knows: the stage that ran out of time, what was
/// being polled, the last probe's error, and how long it tried.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootTimeout {
    pub stage: BootStage,
    /// The endpoint polled during the stage, when it has one (the agent's
    /// `/health` URL).
    pub endpoint: Option<String>,
    pub last_error: Option<ProbeError>,
    pub attempts: u32,
    pub elapsed: Duration,
    /// The proven reason, when there is one. `None` means the stage only ran
    /// out of time.
    pub cause: Option<BootCause>,
    /// A VM was left behind (kept by `--keep-on-failure`, or a teardown that
    /// could not remove it), so the caller must `vm stop` it.
    pub kept: bool,
}

impl BootTimeout {
    /// A timeout that knows only its stage (the golden-provisioning flows,
    /// which poll but do not report probe detail).
    pub fn at(stage: BootStage) -> Self {
        BootTimeout {
            stage,
            endpoint: None,
            last_error: None,
            attempts: 0,
            elapsed: Duration::ZERO,
            cause: None,
            kept: false,
        }
    }
}

impl fmt::Display for BootTimeout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "stage {}", self.stage)?;
        if let Some(endpoint) = &self.endpoint {
            write!(f, ", polling {endpoint}")?;
        }
        if self.attempts > 0 {
            write!(f, ", {} attempts over {}s", self.attempts, self.elapsed.as_secs())?;
        }
        if let Some(err) = &self.last_error {
            write!(f, "; last error: {err}")?;
        }
        if self.cause == Some(BootCause::LocalNetworkDenied) {
            f.write_str(
                "; a platform tool connects to the agent's port, so macOS Local Network \
                 privacy denies this process the route",
            )?;
        }
        Ok(())
    }
}

/// What to do when macOS Local Network privacy denies the process the guest
/// subnet. On a macOS host the process is the TestAnyware app, which starts every VM and
/// holds the grant for every client (ADR-0019). The harness case needs its own sentence
/// on other hosts: a process that a launchd service started has no app that the user can
/// allow.
#[cfg(target_os = "macos")]
const LOCAL_NETWORK_REMEDY: &str =
    "The guest answers a platform tool. Allow TestAnyware in System Settings, Privacy & \
     Security, Local Network. A person does this once, and again when an upgrade of the app asks.";
#[cfg(not(target_os = "macos"))]
const LOCAL_NETWORK_REMEDY: &str =
    "The guest answers a platform tool. Run testanyware from an app that has \
     Local Network access. Allow that app in System Settings, Privacy & Security, Local Network. \
     A background service or agent harness has no app to allow.";

/// What a `VM_LIMIT_REACHED` adds when the framework refused a start while the count of the app
/// showed a free slot (spec "admission-queue": "External occupancy"). The app cannot tell the two
/// causes apart.
const UNSEEN_GUEST_REMEDY: &str =
    "The app counted a free slot, and the framework refused anyway. There are two possible \
     causes. A guest of another user or tool holds a slot that the app cannot see. Or the count \
     of the framework is stuck, which only a restart of the Mac clears.";

/// What a `VM_LIMIT_REACHED` adds when the app had no usable answer of tart for the start.
fn tart_silence_remedy(silence: &TartSilence) -> String {
    let last = match silence.since_last_answer_secs {
        Some(secs) => format!("The last answer of tart came {secs} s before this failure."),
        None => "Tart has not answered since the app started.".to_string(),
    };
    format!(
        "The app had no answer of tart that served this start, because tart does not answer or \
         answers late. {last} Check that `tart list` answers, then retry."
    )
}

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

    #[error("tart failed: {detail}")]
    TartFailed { detail: String },

    #[error("VM '{id}' did not become reachable within the boot window ({timeout})")]
    VmBootTimeout { id: String, timeout: Box<BootTimeout> },

    /// The platform's concurrent-guest limit refused the start (Apple's macOS
    /// guest limit: two per Mac). A "try again later" condition, not a fault.
    /// Boxed so `VmError` stays small enough for `Result` error types.
    #[error("{0}")]
    VmLimitReached(Box<LimitReached>),

    #[error("could not discover the agent port via the QEMU monitor")]
    MonitorDiscoveryFailed,

    #[error("failed to spawn a child process: {detail}")]
    SpawnFailed { detail: String },

    /// The client of the TestAnyware app closed its connection before the start ended, and the
    /// app cleaned up as after a failed start. No client reads it, so it has no remedy.
    #[error("the start of VM '{id}' was cancelled: the client closed its connection")]
    StartCancelled { id: String },

    #[error("golden image '{name}' not found")]
    GoldenNotFound { name: String },

    #[error("golden image '{name}' is in use by running clones (PIDs {clone_pids:?})")]
    GoldenInUse { name: String, clone_pids: Vec<i32> },

    #[error("no VM found for id '{id}'")]
    VmNotFound { id: String },

    #[error("VM '{id}' did not stop cleanly")]
    VmStopFailed { id: String },

    /// The stop of an older QEMU guest, which signals nothing, found the guest still alive. The
    /// blockers name what still answers or holds the disk. Its code is `VM_STOP_FAILED`.
    #[error("VM '{id}' did not stop: {}", blockers.describe())]
    VmStopBlocked { id: String, blockers: Box<StopBlockers> },

    /// The stop found that the pid in the meta names another process, so it signalled nothing and
    /// removed nothing. Its code is `VM_STOP_FAILED`.
    #[error("VM '{id}' did not stop: the recorded pid {} is not proven to be its process, because {}", unproven.pid, unproven.mismatch.describe())]
    VmStopUnproven { id: String, unproven: Box<StopUnproven> },

    #[error("no backend can serve platform '{platform}' (tart support is a later task)")]
    BackendUnsupported { platform: String },

    /// The host OS cannot launch local QEMU VMs. A Windows-host `vm start`
    /// would reach the AF_UNIX QEMU monitor (`monitor.rs`) and the Unix
    /// process/detach helpers, none of which have a runtime path on
    /// Windows. The Windows host binary is **build/link-verified only**
    /// (ADR-0009 no-silent-caps), so this surfaces the gap loudly at the
    /// `vm start` entry instead of failing cryptically deep in the QEMU
    /// launch. Linux and macOS hosts never produce it.
    #[error("local QEMU VM-host is not supported on this host: {detail}")]
    HostUnsupported { detail: String },

    #[error("unknown platform '{value}' (expected macos, linux, or windows)")]
    InvalidPlatform { value: String },

    #[error("SSH error: {detail}")]
    SshConnectFailed { detail: String },

    #[error("golden image creation failed: {detail}")]
    GoldenCreateFailed { detail: String },

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
    /// A `VM_BOOT_TIMEOUT` that knows only its stage.
    pub fn boot_timeout(id: impl Into<String>, stage: BootStage) -> Self {
        VmError::VmBootTimeout { id: id.into(), timeout: Box::new(BootTimeout::at(stage)) }
    }

    /// Stable contract §4 code surfaced in `--json` output.
    pub fn code(&self) -> &'static str {
        match self {
            VmError::KvmPermissionDenied { .. } => "KVM_PERMISSION_DENIED",
            VmError::SwtpmMissing => "SWTPM_MISSING",
            VmError::UefiNotFound { .. } => "UEFI_NOT_FOUND",
            VmError::QemuFailed { .. } | VmError::MonitorDiscoveryFailed => "QEMU_FAILED",
            VmError::TartFailed { .. } => "TART_FAILED",
            VmError::VmBootTimeout { .. } => "VM_BOOT_TIMEOUT",
            VmError::VmLimitReached(_) => "VM_LIMIT_REACHED",
            VmError::SpawnFailed { .. } => "SPAWN_FAILED",
            VmError::StartCancelled { .. } => "START_CANCELLED",
            VmError::GoldenNotFound { .. } => "GOLDEN_NOT_FOUND",
            VmError::GoldenInUse { .. } => "GOLDEN_IN_USE",
            VmError::VmNotFound { .. } => "VM_NOT_FOUND",
            VmError::VmStopFailed { .. }
            | VmError::VmStopBlocked { .. }
            | VmError::VmStopUnproven { .. } => "VM_STOP_FAILED",
            VmError::BackendUnsupported { .. } => "VM_BACKEND_UNSUPPORTED",
            VmError::HostUnsupported { .. } => "VM_HOST_UNSUPPORTED",
            VmError::InvalidPlatform { .. } => "INVALID_PLATFORM",
            VmError::SshConnectFailed { .. } => "SSH_CONNECT_FAILED",
            VmError::GoldenCreateFailed { .. } => "GOLDEN_CREATE_FAILED",
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
            // §5: rate limited / try again later.
            VmError::VmLimitReached(_) => 6,
            // §5: boot/startup-timeout family.
            VmError::VmBootTimeout { .. } => 7,
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
            VmError::VmStopBlocked { id, .. } => Some(format!(
                "Quit that QEMU by hand, then run `testanyware vm stop {id}` again. \
                 The app removed nothing, so the VM keeps its records."
            )),
            VmError::VmStopUnproven { id, unproven } => Some(format!(
                "Signalled nothing, because killing a stranger is worse than leaving a VM. \
                 If the VM still runs, stop it by hand (`tart stop {id}` and `tart delete {id}` \
                 for a tart guest, or quit its QEMU). Then remove {} and run \
                 `testanyware vm list` to confirm that it is gone.",
                unproven.records.join(" and ")
            )),
            VmError::BackendUnsupported { .. } => Some(
                "QEMU serves linux and windows guests. macOS guests use the tart \
                 backend, which is not yet ported to the Rust CLI."
                    .into(),
            ),
            VmError::VmBootTimeout { id, timeout } => {
                let next = if timeout.kept {
                    format!("The VM was left behind; reclaim it with `testanyware vm stop {id}`.")
                } else {
                    "Re-run with --keep-on-failure to leave the VM running for inspection.".into()
                };
                Some(match timeout.cause {
                    Some(BootCause::LocalNetworkDenied) => format!("{LOCAL_NETWORK_REMEDY} {next}"),
                    None => next,
                })
            }
            VmError::VmLimitReached(limit) => {
                let stop = if limit.holders.is_empty() {
                    "Stop a running macOS VM (`testanyware vm list`, `testanyware vm stop <id>`), \
                     or retry once one has finished."
                        .to_string()
                } else {
                    format!(
                        "Stop one of the running macOS VMs ({}) with `testanyware vm stop <id>`, \
                         or retry once one has finished.",
                        limit.holders.join(", ")
                    )
                };
                let mut remedy = stop;
                if limit.admission.unseen_possible {
                    remedy = format!("{remedy} {UNSEEN_GUEST_REMEDY}");
                }
                if let Some(silence) = &limit.admission.tart_silence {
                    remedy = format!("{remedy} {}", tart_silence_remedy(silence));
                }
                Some(remedy)
            }
            VmError::HostUnsupported { .. } => Some(
                "Launch VMs from a Linux or macOS host. The Windows host binary is \
                 build/link-verified only — it has no local-QEMU runtime path \
                 (the AF_UNIX monitor and Unix process control are macOS/Linux-only). \
                 See docs/adr/0009-self-hosted-host-cli-verification-harness.md."
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
            VmError::VmNotFound { id }
            | VmError::VmStopFailed { id }
            | VmError::StartCancelled { id } => json!({ "vm_id": id }),
            VmError::VmStopBlocked { id, blockers } => blockers.details(id),
            VmError::VmStopUnproven { id, unproven } => {
                let mut details = crate::adoption::doubt_details(id, unproven.pid, &unproven.mismatch);
                details["records"] = json!(unproven.records);
                details
            }
            VmError::VmBootTimeout { id, timeout } => json!({
                "vm_id": id,
                "stage": timeout.stage.as_str(),
                "endpoint": timeout.endpoint,
                "attempts": timeout.attempts,
                "elapsed_secs": timeout.elapsed.as_secs_f64(),
                "last_error": timeout.last_error.as_ref().map(|e| json!({
                    "kind": e.kind.as_str(),
                    "os_errno": e.os_errno,
                    "detail": e.detail,
                })),
                "cause": timeout.cause.map(BootCause::as_str),
                "kept": timeout.kept,
            }),
            VmError::VmLimitReached(limit) => json!({
                "platform": limit.platform,
                "limit": limit.limit,
                "holders": limit.holders,
                "tart_error": limit.raw,
                "request_id": limit.admission.request_id,
                "place": limit.admission.place,
                "waited_secs": limit.admission.waited_secs,
                "refused_by": limit.admission.refused_by.as_str(),
            }),
            VmError::TartFailed { detail } => json!({ "tart_error": detail }),
            VmError::QemuFailed { detail } => json!({ "qemu_error": detail }),
            VmError::SshConnectFailed { detail }
            | VmError::GoldenCreateFailed { detail }
            | VmError::HostUnsupported { detail } => {
                json!({ "detail": detail })
            }
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
    fn vm_error_stays_small_enough_to_be_a_result_error() {
        // Rich variants (boot-timeout detail, limit refusal) are boxed so
        // `Result<_, (SetupVm, VmError)>` callers do not trip
        // `clippy::result_large_err`. Widening a variant past this fails here
        // instead of surfacing as a lint in unrelated callers.
        assert!(
            std::mem::size_of::<VmError>() <= 56,
            "VmError is {} bytes",
            std::mem::size_of::<VmError>()
        );
    }

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
        let blocked = VmError::VmStopBlocked {
            id: "v".into(),
            blockers: Box::new(StopBlockers {
                monitor_socket: None,
                swtpm_socket: None,
                disk_holders: vec![9],
                disk_check_error: None,
            }),
        };
        assert_eq!(blocked.code(), "VM_STOP_FAILED");
        assert_eq!(blocked.exit_code(), 1);
        assert_eq!(blocked.details()["vm_id"], "v");
        assert!(blocked.to_string().contains("process 9 still holds the clone's disk"));
        assert_eq!(VmError::BackendUnsupported { platform: "macos".into() }.code(), "VM_BACKEND_UNSUPPORTED");
        assert_eq!(VmError::HostUnsupported { detail: "x".into() }.code(), "VM_HOST_UNSUPPORTED");
        assert_eq!(VmError::InvalidPlatform { value: "bsd".into() }.code(), "INVALID_PLATFORM");
        assert_eq!(VmError::SshConnectFailed { detail: "x".into() }.code(), "SSH_CONNECT_FAILED");
        assert_eq!(VmError::GoldenCreateFailed { detail: "x".into() }.code(), "GOLDEN_CREATE_FAILED");
        assert_eq!(VmError::Io("disk full".into()).code(), "IO_ERROR");
    }

    #[test]
    fn golden_provisioning_errors_are_generic_exit_1_with_detail() {
        // SSH_CONNECT_FAILED / GOLDEN_CREATE_FAILED are §4.1/§4.2 additions
        // for `vm create-golden` (ADR-0007); both are generic failures (exit
        // 1) and carry their cause in `details.detail`.
        let ssh = VmError::SshConnectFailed { detail: "connect 1.2.3.4:22: refused".into() };
        assert_eq!(ssh.exit_code(), 1);
        assert_eq!(ssh.details()["detail"], "connect 1.2.3.4:22: refused");
        let golden = VmError::GoldenCreateFailed { detail: "health gate failed".into() };
        assert_eq!(golden.exit_code(), 1);
        assert_eq!(golden.details()["detail"], "health gate failed");
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

    #[test]
    fn host_unsupported_is_generic_exit_1_with_detail_and_remediation() {
        // The Windows-host local-QEMU boundary (ADR-0009): a generic
        // failure (§5 → exit 1) that carries its cause in `details.detail`
        // and points the operator at a Linux/macOS host.
        let err = VmError::HostUnsupported {
            detail: "Windows host: local QEMU launch (vm start) is build-verified only".into(),
        };
        assert_eq!(err.code(), "VM_HOST_UNSUPPORTED");
        assert_eq!(err.exit_code(), 1);
        assert!(err.details()["detail"].as_str().unwrap().contains("build-verified only"));
        assert!(err.remediation().unwrap().contains("Linux or macOS host"));
    }

    #[test]
    fn tart_failed_maps_to_its_contract_code_exit_and_detail() {
        let err = VmError::TartFailed { detail: "tart clone: image not found".into() };
        // §4.2: TART_FAILED, generic failure (§5 → exit 1).
        assert_eq!(err.code(), "TART_FAILED");
        assert_eq!(err.exit_code(), 1);
        // §4.2: `details.tart_error` carries the subprocess stderr.
        assert_eq!(err.details()["tart_error"], "tart clone: image not found");
    }

    #[test]
    fn qemu_failed_carries_the_backends_reason_in_qemu_error() {
        let err = VmError::QemuFailed { detail: "qemu: could not open disk".into() };
        assert_eq!(err.code(), "QEMU_FAILED");
        assert_eq!(err.exit_code(), 1);
        // Mirrors `details.tart_error`, so a driver reads either backend's
        // reason the same way.
        assert_eq!(err.details()["qemu_error"], "qemu: could not open disk");
    }

    #[test]
    fn vm_boot_timeout_maps_to_its_contract_code_and_exit_7() {
        let err = VmError::boot_timeout("testanyware-abcd1234", BootStage::Vnc);
        // §4.2 code + §5 boot-timeout family → exit 7.
        assert_eq!(err.code(), "VM_BOOT_TIMEOUT");
        assert_eq!(err.exit_code(), 7);
        assert_eq!(err.details()["vm_id"], "testanyware-abcd1234");
        assert_eq!(err.details()["stage"], "vnc");
    }

    fn agent_timeout(kept: bool) -> VmError {
        VmError::VmBootTimeout {
            id: "testanyware-abcd1234".into(),
            timeout: Box::new(BootTimeout {
                stage: BootStage::Agent,
                endpoint: Some("http://192.168.64.5:8648/health".into()),
                last_error: Some(ProbeError {
                    kind: crate::health::ProbeKind::Unreachable,
                    os_errno: Some(65),
                    detail: "tcp connect error".into(),
                }),
                attempts: 60,
                elapsed: Duration::from_secs(118),
                cause: None,
                kept,
            }),
        }
    }

    #[test]
    fn a_denied_agent_timeout_names_its_cause_and_the_remedy() {
        let err = VmError::VmBootTimeout {
            id: "testanyware-abcd1234".into(),
            timeout: Box::new(BootTimeout {
                cause: Some(BootCause::LocalNetworkDenied),
                ..match agent_timeout(false) {
                    VmError::VmBootTimeout { timeout, .. } => *timeout,
                    _ => unreachable!(),
                }
            }),
        };
        assert_eq!(err.code(), "VM_BOOT_TIMEOUT", "the contract code is unchanged");
        assert_eq!(err.exit_code(), 7);
        assert_eq!(err.details()["cause"], "local_network_denied");
        assert!(err.to_string().contains("Local Network privacy denies this process"));
        let remedy = err.remediation().unwrap();
        assert!(remedy.contains("Local Network"), "{remedy}");
        assert!(remedy.contains("--keep-on-failure"), "the keep hint stays: {remedy}");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn on_a_macos_host_the_denial_remedy_names_the_app() {
        let err = VmError::VmBootTimeout {
            id: "testanyware-abcd1234".into(),
            timeout: Box::new(BootTimeout {
                cause: Some(BootCause::LocalNetworkDenied),
                ..match agent_timeout(false) {
                    VmError::VmBootTimeout { timeout, .. } => *timeout,
                    _ => unreachable!(),
                }
            }),
        };
        let remedy = err.remediation().unwrap();
        assert!(
            remedy.contains("Allow TestAnyware in System Settings, Privacy & Security, Local Network"),
            "{remedy}"
        );
        assert!(!remedy.contains("agent harness"), "no advice for a harness that cannot allow anything: {remedy}");
    }

    #[test]
    fn a_cancelled_start_has_its_own_code_and_names_the_vm() {
        let err = VmError::StartCancelled { id: "vm1".into() };
        assert_eq!(err.code(), "START_CANCELLED");
        assert_eq!(err.details()["vm_id"], "vm1");
    }

    #[test]
    fn a_plain_timeout_has_no_cause() {
        assert_eq!(agent_timeout(false).details()["cause"], serde_json::Value::Null);
    }

    #[test]
    fn vm_boot_timeout_names_the_stage_endpoint_last_error_attempts_and_elapsed() {
        let err = agent_timeout(false);
        let message = err.to_string();
        for needle in [
            "stage agent",
            "http://192.168.64.5:8648/health",
            "60 attempts over 118s",
            "host unreachable (os error 65)",
        ] {
            assert!(message.contains(needle), "{needle:?} missing from: {message}");
        }
        let d = err.details();
        assert_eq!(d["stage"], "agent");
        assert_eq!(d["endpoint"], "http://192.168.64.5:8648/health");
        assert_eq!(d["attempts"], 60);
        assert_eq!(d["elapsed_secs"], 118.0);
        assert_eq!(d["last_error"]["kind"], "host_unreachable");
        assert_eq!(d["last_error"]["os_errno"], 65);
        assert_eq!(d["kept"], false);
    }

    #[test]
    fn vm_boot_timeout_remediation_depends_on_whether_the_vm_was_kept() {
        assert!(agent_timeout(false).remediation().unwrap().contains("--keep-on-failure"));
        let kept = agent_timeout(true);
        assert!(kept.remediation().unwrap().contains("vm stop testanyware-abcd1234"));
        assert_eq!(kept.details()["kept"], true);
    }

    #[test]
    fn vm_limit_reached_is_exit_6_and_names_the_holders() {
        let err = VmError::VmLimitReached(Box::new(LimitReached {
            platform: "macos".into(),
            limit: 2,
            holders: vec!["koine-verify-1".into(), "testanyware-a1b2c3d4".into()],
            raw: "The number of VMs exceeds the system limit (other running VMs: koine-verify-1, testanyware-a1b2c3d4)".into(),
            admission: AdmissionFacts::default(),
        }));
        // §4.2 code + §5 "rate limited / try again later" → exit 6.
        assert_eq!(err.code(), "VM_LIMIT_REACHED");
        assert_eq!(err.exit_code(), 6);
        let d = err.details();
        assert_eq!(d["platform"], "macos");
        assert_eq!(d["limit"], 2);
        assert_eq!(d["holders"], serde_json::json!(["koine-verify-1", "testanyware-a1b2c3d4"]));
        assert!(d["tart_error"].as_str().unwrap().contains("exceeds the system limit"));
        // No app took part: the new members say so.
        assert_eq!(d["request_id"], serde_json::Value::Null);
        assert_eq!(d["place"], serde_json::Value::Null);
        assert_eq!(d["waited_secs"], 0);
        assert_eq!(d["refused_by"], "framework");
        assert!(err.to_string().contains("koine-verify-1, testanyware-a1b2c3d4"));
        assert!(err.remediation().unwrap().contains("vm stop"));
    }

    fn limit_with(unseen_possible: bool) -> VmError {
        VmError::VmLimitReached(Box::new(LimitReached {
            platform: "macos".into(),
            limit: 2,
            holders: vec![],
            raw: String::new(),
            admission: AdmissionFacts {
                unseen_possible,
                ..AdmissionFacts::default()
            },
        }))
    }

    #[test]
    fn the_remedy_names_both_causes_when_the_framework_refused_beside_a_free_count() {
        let remedy = limit_with(true).remediation().unwrap();
        assert!(remedy.contains("guest of another user or tool"), "{remedy}");
        assert!(remedy.contains("stuck"), "{remedy}");
        assert!(remedy.contains("restart of the Mac"), "{remedy}");
        // The stop advice stays.
        assert!(remedy.contains("vm stop"), "{remedy}");
    }

    #[test]
    fn the_remedy_names_neither_cause_when_the_count_was_full() {
        let remedy = limit_with(false).remediation().unwrap();
        assert!(!remedy.contains("stuck"), "{remedy}");
        assert!(!remedy.contains("another user"), "{remedy}");
    }

    #[test]
    fn the_unseen_guest_flag_is_no_member_of_the_details() {
        let details = limit_with(true).details();
        assert!(details.get("unseen_possible").is_none(), "{details}");
    }

    #[test]
    fn the_remedy_says_that_tart_does_not_answer_with_the_time_of_the_last_answer() {
        let silent = |since_last_answer_secs| {
            VmError::VmLimitReached(Box::new(LimitReached {
                platform: "macos".into(),
                limit: 2,
                holders: vec![],
                raw: String::new(),
                admission: AdmissionFacts {
                    tart_silence: Some(TartSilence {
                        since_last_answer_secs,
                    }),
                    ..AdmissionFacts::default()
                },
            }))
        };
        let remedy = silent(Some(42)).remediation().unwrap();
        assert!(remedy.contains("tart does not answer"), "{remedy}");
        assert!(remedy.contains("42 s"), "{remedy}");
        assert!(remedy.contains("vm stop"), "the stop advice stays: {remedy}");
        let remedy = silent(None).remediation().unwrap();
        assert!(remedy.contains("has not answered since the app started"), "{remedy}");
        assert!(!limit_with(false).remediation().unwrap().contains("tart does not answer"));
    }

    #[test]
    fn the_silence_of_tart_is_no_member_of_the_details() {
        let details = VmError::VmLimitReached(Box::new(LimitReached {
            platform: "macos".into(),
            limit: 2,
            holders: vec![],
            raw: String::new(),
            admission: AdmissionFacts {
                tart_silence: Some(TartSilence {
                    since_last_answer_secs: None,
                }),
                ..AdmissionFacts::default()
            },
        }))
        .details();
        assert!(details.get("tart_silence").is_none(), "{details}");
    }
}
