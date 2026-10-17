//! The start-outcome decision: did `vm start` succeed, and if not, what is the
//! error and what happens to the VM?
//!
//! A pure function from what the boot *observed* ([`BootObservation`]) to a
//! [`StartDecision`]. The imperative shell (the lifecycle) runs the stages and
//! fills the observation in; every rule about honest failure lives here, where
//! it is unit-tested with no tart or QEMU:
//!
//! - **Strict:** a start succeeds only with a Healthy agent. A VM that is up
//!   but whose agent never became healthy is a `VM_BOOT_TIMEOUT`, never an
//!   agent-less success.
//! - **The backend's death wins:** once the backend process has exited, its
//!   log tail is the reason (`TART_FAILED` / `QEMU_FAILED`), or — when it is
//!   the platform's concurrent-guest limit — `VM_LIMIT_REACHED`.
//! - **Teardown unless kept:** a failed start removes its VM unless
//!   `--keep-on-failure` asked otherwise, and a VM whose process is gone has
//!   nothing left to keep.

use std::time::Duration;

use crate::error::{BootStage, BootTimeout, LimitReached, VmError};
use crate::health::AgentWait;
use crate::lifecycle::Platform;

/// Apple's cap on concurrently running macOS guests per Mac (the macOS
/// licence, §2B(iii); enforced by Virtualization.framework).
pub const MACOS_GUEST_LIMIT: u32 = 2;

/// Which backend ran the VM; selects `TART_FAILED` vs `QEMU_FAILED`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    Tart,
    Qemu,
}

/// The backend process at the moment the boot was last observed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessState {
    Running,
    /// Exited; `log_tail` is the tail of the *current* run's log.
    Exited {
        log_tail: String,
    },
}

/// The VNC-endpoint stage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VncObservation {
    Found,
    Missing { attempts: u32, elapsed: Duration },
}

/// The guest-address stage: the guest IP for tart, the agent port forward for
/// QEMU.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddressObservation {
    NotReached,
    Found,
    Missing { attempts: u32, elapsed: Duration },
}

/// The agent stage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentObservation {
    NotReached,
    Waited { endpoint: String, wait: AgentWait },
}

/// Everything the boot observed, in stage order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootObservation {
    pub id: String,
    pub backend: Backend,
    pub platform: Platform,
    pub process: ProcessState,
    pub vnc: VncObservation,
    pub address: AddressObservation,
    pub agent: AgentObservation,
}

/// What to do with the VM of a failed start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disposition {
    /// Stop and delete it; leave no sidecars.
    Teardown,
    /// Leave it running with its sidecars (`--keep-on-failure`).
    Keep,
}

/// The verdict.
#[derive(Debug)]
pub enum StartDecision {
    /// The agent is healthy: finish the start.
    Ready,
    Failed {
        error: VmError,
        disposition: Disposition,
    },
}

/// Decide the outcome of a boot. See the module docs for the rules.
pub fn decide_start(obs: &BootObservation, keep_on_failure: bool) -> StartDecision {
    // The backend's death is the root cause of whatever the later stages saw.
    if let ProcessState::Exited { log_tail } = &obs.process {
        return StartDecision::Failed {
            error: backend_exit_error(obs, log_tail),
            disposition: Disposition::Teardown,
        };
    }

    let timeout = match (&obs.vnc, &obs.address, &obs.agent) {
        (VncObservation::Missing { attempts, elapsed }, _, _) => BootTimeout {
            attempts: *attempts,
            elapsed: *elapsed,
            ..BootTimeout::at(BootStage::Vnc)
        },
        (_, AddressObservation::Missing { attempts, elapsed }, _) => BootTimeout {
            attempts: *attempts,
            elapsed: *elapsed,
            ..BootTimeout::at(BootStage::GuestIp)
        },
        (
            _,
            _,
            AgentObservation::Waited {
                wait: AgentWait::Healthy { .. },
                ..
            },
        ) => {
            return StartDecision::Ready;
        }
        (
            _,
            _,
            AgentObservation::Waited {
                endpoint,
                wait:
                    AgentWait::NotHealthy {
                        last_error,
                        attempts,
                        elapsed,
                    },
            },
        ) => BootTimeout {
            endpoint: Some(endpoint.clone()),
            last_error: last_error.clone(),
            attempts: *attempts,
            elapsed: *elapsed,
            ..BootTimeout::at(BootStage::Agent)
        },
        // The wait saw the VM die but the shell's last sample still found it
        // running: the exit raced the sample. The backend is gone either way.
        (
            _,
            _,
            AgentObservation::Waited {
                wait: AgentWait::VmExited { last_error, .. },
                ..
            },
        ) => {
            let detail = match last_error {
                Some(err) => {
                    format!("the VM process exited during the agent wait (last probe: {err})")
                }
                None => "the VM process exited during the agent wait".to_string(),
            };
            return StartDecision::Failed {
                error: backend_failed(obs.backend, detail),
                disposition: Disposition::Teardown,
            };
        }
        // Every earlier stage passed yet the agent was never waited on: report
        // it as an agent-stage timeout with no probes rather than as success.
        (_, _, AgentObservation::NotReached) => BootTimeout::at(BootStage::Agent),
    };

    let disposition = if keep_on_failure {
        Disposition::Keep
    } else {
        Disposition::Teardown
    };
    let timeout = BootTimeout {
        kept: disposition == Disposition::Keep,
        ..timeout
    };
    StartDecision::Failed {
        error: VmError::VmBootTimeout {
            id: obs.id.clone(),
            timeout: Box::new(timeout),
        },
        disposition,
    }
}

fn backend_exit_error(obs: &BootObservation, log_tail: &str) -> VmError {
    // The guest limit is Virtualization.framework's, so only tart can report
    // it; a QEMU log that happens to contain the phrase is a QEMU failure.
    if obs.backend == Backend::Tart {
        if let Some(refusal) = parse_limit_refusal(log_tail) {
            return VmError::VmLimitReached(Box::new(LimitReached {
                platform: obs.platform.as_str().to_string(),
                limit: MACOS_GUEST_LIMIT,
                holders: refusal.holders,
                raw: refusal.raw,
            }));
        }
    }
    backend_failed(obs.backend, exit_detail(log_tail))
}

/// The reason a backend process that exited gives: its log tail, or a plain
/// statement that it said nothing.
pub fn exit_detail(log_tail: &str) -> String {
    if log_tail.trim().is_empty() {
        "the VM process exited without output".to_string()
    } else {
        log_tail.trim().to_string()
    }
}

fn backend_failed(backend: Backend, detail: String) -> VmError {
    match backend {
        Backend::Tart => VmError::TartFailed { detail },
        Backend::Qemu => VmError::QemuFailed { detail },
    }
}

/// A refusal because the platform's concurrent-guest limit is full.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LimitRefusal {
    /// The VMs holding the slots, as the backend named them (possibly none).
    pub holders: Vec<String>,
    /// The backend's own line, verbatim.
    pub raw: String,
}

/// Recognise tart's macOS-guest-limit refusal in a log tail.
///
/// tart rewords Virtualization.framework's `VZErrorVirtualMachineLimitExceeded`
/// as `The number of VMs exceeds the system limit (other running VMs: a, b)`
/// (observed with tart 2.40.1 on macOS 27). Only the stable phrase `exceeds
/// the system limit` is matched; the holders list is best-effort.
pub fn parse_limit_refusal(log_tail: &str) -> Option<LimitRefusal> {
    const STABLE: &str = "exceeds the system limit";
    const HOLDERS: &str = "other running VMs:";
    let raw = log_tail
        .lines()
        .find(|line| line.contains(STABLE))?
        .trim()
        .to_string();
    let holders = raw
        .split_once(HOLDERS)
        .map(|(_, rest)| rest.trim().trim_end_matches(')'))
        .map(|names| {
            names
                .split(',')
                .map(str::trim)
                .filter(|n| !n.is_empty())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();
    Some(LimitRefusal { holders, raw })
}

/// How much of a dead backend's log goes into its error.
pub const LOG_TAIL_LINES: usize = 20;
pub const LOG_TAIL_BYTES: usize = 4096;

/// The tail of the log at `path` ([`LOG_TAIL_LINES`] / [`LOG_TAIL_BYTES`]);
/// empty if it cannot be read. Read it *before* tearing the VM down: QEMU's
/// log lives in the clone directory that teardown deletes.
pub fn read_log_tail(path: &std::path::Path) -> String {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    log_tail(&text, LOG_TAIL_LINES, LOG_TAIL_BYTES)
}

/// The tail of a log: at most `max_lines` lines, then at most `max_bytes`
/// bytes (cut on a character boundary), so an error never carries a whole
/// multi-megabyte log.
pub fn log_tail(text: &str, max_lines: usize, max_bytes: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.len().saturating_sub(max_lines);
    let joined = lines[start..].join("\n");
    if joined.len() <= max_bytes {
        return joined;
    }
    let mut cut = joined.len() - max_bytes;
    while !joined.is_char_boundary(cut) {
        cut += 1;
    }
    joined[cut..].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::health::{ProbeError, ProbeKind};

    const LIMIT_LINE: &str =
        "The number of VMs exceeds the system limit (other running VMs: k2probe-b, k2probe-a)";

    fn unreachable_probe() -> ProbeError {
        ProbeError {
            kind: ProbeKind::Unreachable,
            os_errno: Some(65),
            detail: "tcp connect error".into(),
        }
    }

    /// A boot that went perfectly: process up, every stage passed.
    fn healthy() -> BootObservation {
        BootObservation {
            id: "testanyware-abcd1234".into(),
            backend: Backend::Tart,
            platform: Platform::Macos,
            process: ProcessState::Running,
            vnc: VncObservation::Found,
            address: AddressObservation::Found,
            agent: AgentObservation::Waited {
                endpoint: "http://192.168.64.5:8648/health".into(),
                wait: AgentWait::Healthy { attempts: 4 },
            },
        }
    }

    fn agent_never_healthy() -> BootObservation {
        BootObservation {
            agent: AgentObservation::Waited {
                endpoint: "http://192.168.64.5:8648/health".into(),
                wait: AgentWait::NotHealthy {
                    last_error: Some(unreachable_probe()),
                    attempts: 60,
                    elapsed: Duration::from_secs(118),
                },
            },
            ..healthy()
        }
    }

    fn exited(log: &str) -> BootObservation {
        BootObservation {
            process: ProcessState::Exited {
                log_tail: log.into(),
            },
            ..healthy()
        }
    }

    fn failure(decision: StartDecision) -> (VmError, Disposition) {
        match decision {
            StartDecision::Failed { error, disposition } => (error, disposition),
            StartDecision::Ready => panic!("expected a failed start, got Ready"),
        }
    }

    #[test]
    fn a_healthy_agent_is_the_only_success() {
        assert!(matches!(
            decide_start(&healthy(), false),
            StartDecision::Ready
        ));
        assert!(matches!(
            decide_start(&healthy(), true),
            StartDecision::Ready
        ));
    }

    #[test]
    fn an_alive_vm_whose_agent_never_became_healthy_is_a_boot_timeout_naming_the_last_probe_error()
    {
        // The strict rule: finding 1's case (VM alive, agent unhealthy) must
        // not be an agent-less success.
        let (error, disposition) = failure(decide_start(&agent_never_healthy(), false));
        assert_eq!(error.code(), "VM_BOOT_TIMEOUT");
        assert_eq!(error.exit_code(), 7);
        let d = error.details();
        assert_eq!(d["stage"], "agent");
        assert_eq!(d["endpoint"], "http://192.168.64.5:8648/health");
        assert_eq!(d["attempts"], 60);
        assert_eq!(d["last_error"]["kind"], "host_unreachable");
        assert_eq!(d["last_error"]["os_errno"], 65);
        assert_eq!(d["kept"], false);
        assert_eq!(disposition, Disposition::Teardown);
    }

    #[test]
    fn keep_on_failure_keeps_an_alive_vm_and_says_so_in_the_error() {
        let (error, disposition) = failure(decide_start(&agent_never_healthy(), true));
        assert_eq!(disposition, Disposition::Keep);
        assert_eq!(error.details()["kept"], true);
    }

    #[test]
    fn a_missing_vnc_url_is_a_vnc_stage_timeout() {
        let obs = BootObservation {
            vnc: VncObservation::Missing {
                attempts: 60,
                elapsed: Duration::from_secs(59),
            },
            address: AddressObservation::NotReached,
            agent: AgentObservation::NotReached,
            ..healthy()
        };
        let (error, disposition) = failure(decide_start(&obs, false));
        assert_eq!(error.code(), "VM_BOOT_TIMEOUT");
        assert_eq!(error.details()["stage"], "vnc");
        assert_eq!(error.details()["attempts"], 60);
        assert_eq!(disposition, Disposition::Teardown);
    }

    #[test]
    fn a_missing_guest_ip_is_a_guest_ip_stage_timeout() {
        let obs = BootObservation {
            address: AddressObservation::Missing {
                attempts: 30,
                elapsed: Duration::from_secs(58),
            },
            agent: AgentObservation::NotReached,
            ..healthy()
        };
        let (error, _) = failure(decide_start(&obs, true));
        assert_eq!(error.details()["stage"], "guest-ip");
        assert_eq!(error.details()["kept"], true);
    }

    #[test]
    fn an_agent_stage_that_was_never_run_is_not_a_success() {
        let obs = BootObservation {
            agent: AgentObservation::NotReached,
            ..healthy()
        };
        let (error, _) = failure(decide_start(&obs, false));
        assert_eq!(error.details()["stage"], "agent");
        assert_eq!(error.details()["attempts"], 0);
    }

    #[test]
    fn a_backend_that_exited_is_tart_failed_with_its_log_tail() {
        let (error, disposition) = failure(decide_start(
            &exited("Error: disk image is locked\n"),
            false,
        ));
        assert_eq!(error.code(), "TART_FAILED");
        assert_eq!(error.exit_code(), 1);
        assert_eq!(error.details()["tart_error"], "Error: disk image is locked");
        assert_eq!(disposition, Disposition::Teardown);
    }

    #[test]
    fn a_qemu_backend_that_exited_is_qemu_failed() {
        let obs = BootObservation {
            backend: Backend::Qemu,
            ..exited("qemu: could not open disk")
        };
        let (error, _) = failure(decide_start(&obs, false));
        assert_eq!(error.code(), "QEMU_FAILED");
    }

    #[test]
    fn the_backends_death_wins_over_every_later_stage_observation() {
        // The process died: whatever the agent probes then saw is a symptom.
        let obs = BootObservation {
            process: ProcessState::Exited {
                log_tail: "tart crashed".into(),
            },
            ..agent_never_healthy()
        };
        let (error, _) = failure(decide_start(&obs, false));
        assert_eq!(error.code(), "TART_FAILED");
        let obs = BootObservation {
            vnc: VncObservation::Missing {
                attempts: 1,
                elapsed: Duration::ZERO,
            },
            ..exited("tart crashed")
        };
        let (error, _) = failure(decide_start(&obs, false));
        assert_eq!(error.code(), "TART_FAILED");
    }

    #[test]
    fn a_backend_that_exited_silently_still_explains_itself() {
        let (error, _) = failure(decide_start(&exited("  \n"), false));
        assert_eq!(
            error.details()["tart_error"],
            "the VM process exited without output"
        );
    }

    #[test]
    fn keep_on_failure_cannot_keep_a_vm_whose_process_is_gone() {
        let (_, disposition) = failure(decide_start(&exited("tart crashed"), true));
        assert_eq!(disposition, Disposition::Teardown);
    }

    #[test]
    fn the_macos_guest_limit_is_vm_limit_reached_naming_the_holders() {
        let obs = exited(LIMIT_LINE);
        let (error, disposition) = failure(decide_start(&obs, false));
        assert_eq!(error.code(), "VM_LIMIT_REACHED");
        assert_eq!(error.exit_code(), 6);
        let d = error.details();
        assert_eq!(d["platform"], "macos");
        assert_eq!(d["limit"], 2);
        assert_eq!(d["holders"], serde_json::json!(["k2probe-b", "k2probe-a"]));
        assert_eq!(d["tart_error"], LIMIT_LINE);
        assert_eq!(
            disposition,
            Disposition::Teardown,
            "a refused start leaves no clone"
        );
        let (_, kept) = failure(decide_start(&obs, true));
        assert_eq!(
            kept,
            Disposition::Teardown,
            "...not even under --keep-on-failure"
        );
    }

    #[test]
    fn a_wait_that_saw_the_vm_exit_before_the_process_sample_is_still_a_backend_failure() {
        let obs = BootObservation {
            agent: AgentObservation::Waited {
                endpoint: "http://192.168.64.5:8648/health".into(),
                wait: AgentWait::VmExited {
                    last_error: Some(unreachable_probe()),
                    attempts: 3,
                    elapsed: Duration::from_secs(6),
                },
            },
            ..healthy()
        };
        let (error, disposition) = failure(decide_start(&obs, true));
        assert_eq!(error.code(), "TART_FAILED");
        assert!(error.details()["tart_error"]
            .as_str()
            .unwrap()
            .contains("host unreachable"));
        assert_eq!(disposition, Disposition::Teardown);
    }

    // ---- QEMU observations ------------------------------------------------

    /// A QEMU boot that went perfectly: the forward is the loopback port.
    fn qemu_healthy() -> BootObservation {
        BootObservation {
            id: "testanyware-win0001".into(),
            backend: Backend::Qemu,
            platform: Platform::Windows,
            process: ProcessState::Running,
            vnc: VncObservation::Found,
            address: AddressObservation::Found,
            agent: AgentObservation::Waited {
                endpoint: "http://localhost:51234/health".into(),
                wait: AgentWait::Healthy { attempts: 31 },
            },
        }
    }

    fn qemu_agent_never_healthy() -> BootObservation {
        BootObservation {
            agent: AgentObservation::Waited {
                endpoint: "http://localhost:51234/health".into(),
                wait: AgentWait::NotHealthy {
                    last_error: Some(ProbeError {
                        kind: ProbeKind::Timeout,
                        os_errno: None,
                        detail: "request timed out".into(),
                    }),
                    attempts: 120,
                    elapsed: Duration::from_secs(600),
                },
            },
            ..qemu_healthy()
        }
    }

    #[test]
    fn a_qemu_guest_with_a_healthy_agent_is_ready() {
        assert!(matches!(
            decide_start(&qemu_healthy(), false),
            StartDecision::Ready
        ));
    }

    #[test]
    fn a_qemu_guest_whose_agent_never_became_healthy_is_a_boot_timeout_not_a_warning() {
        // The rule QEMU used to break: it warned and exited 0 with `agent: null`.
        let (error, disposition) = failure(decide_start(&qemu_agent_never_healthy(), false));
        assert_eq!(error.code(), "VM_BOOT_TIMEOUT");
        assert_eq!(error.exit_code(), 7);
        let d = error.details();
        assert_eq!(d["stage"], "agent");
        assert_eq!(d["endpoint"], "http://localhost:51234/health");
        assert_eq!(d["attempts"], 120);
        assert_eq!(d["last_error"]["kind"], "timeout");
        assert_eq!(d["kept"], false);
        assert_eq!(disposition, Disposition::Teardown);
    }

    #[test]
    fn keep_on_failure_keeps_a_qemu_guest_that_is_still_running() {
        let (error, disposition) = failure(decide_start(&qemu_agent_never_healthy(), true));
        assert_eq!(disposition, Disposition::Keep);
        assert_eq!(error.details()["kept"], true);
    }

    #[test]
    fn a_qemu_process_that_exited_is_qemu_failed_with_its_log_tail() {
        let obs = BootObservation {
            process: ProcessState::Exited {
                log_tail: "qemu-system-aarch64: could not open disk image".into(),
            },
            ..qemu_agent_never_healthy()
        };
        let (error, disposition) = failure(decide_start(&obs, false));
        assert_eq!(error.code(), "QEMU_FAILED");
        assert_eq!(error.exit_code(), 1);
        assert_eq!(
            error.details()["qemu_error"],
            "qemu-system-aarch64: could not open disk image"
        );
        assert_eq!(disposition, Disposition::Teardown);
    }

    #[test]
    fn keep_on_failure_cannot_keep_a_qemu_guest_whose_process_is_gone() {
        let obs = BootObservation {
            process: ProcessState::Exited {
                log_tail: "killed".into(),
            },
            ..qemu_healthy()
        };
        let (_, disposition) = failure(decide_start(&obs, true));
        assert_eq!(disposition, Disposition::Teardown);
    }

    #[test]
    fn a_qemu_wait_that_saw_the_process_exit_is_qemu_failed() {
        let obs = BootObservation {
            agent: AgentObservation::Waited {
                endpoint: "http://localhost:51234/health".into(),
                wait: AgentWait::VmExited {
                    last_error: None,
                    attempts: 4,
                    elapsed: Duration::from_secs(20),
                },
            },
            ..qemu_healthy()
        };
        let (error, disposition) = failure(decide_start(&obs, true));
        assert_eq!(error.code(), "QEMU_FAILED");
        assert!(error.details()["qemu_error"]
            .as_str()
            .unwrap()
            .contains("exited during the agent wait"));
        assert_eq!(disposition, Disposition::Teardown);
    }

    #[test]
    fn only_a_tart_exit_can_be_the_macos_guest_limit() {
        // The limit is Virtualization.framework's; a QEMU log that happens to
        // contain the phrase is still a QEMU failure.
        let obs = BootObservation {
            process: ProcessState::Exited {
                log_tail: LIMIT_LINE.into(),
            },
            ..qemu_healthy()
        };
        let (error, _) = failure(decide_start(&obs, false));
        assert_eq!(error.code(), "QEMU_FAILED");
    }

    #[test]
    fn a_qemu_that_exited_silently_still_explains_itself() {
        let obs = BootObservation {
            process: ProcessState::Exited {
                log_tail: " \n".into(),
            },
            ..qemu_healthy()
        };
        let (error, _) = failure(decide_start(&obs, false));
        assert_eq!(
            error.details()["qemu_error"],
            "the VM process exited without output"
        );
    }

    #[test]
    fn limit_refusal_parses_the_observed_tart_line_amid_other_output() {
        let log = format!("some preamble\n{LIMIT_LINE}\ntrailer\n");
        let refusal = parse_limit_refusal(&log).expect("the observed line must match");
        assert_eq!(refusal.holders, vec!["k2probe-b", "k2probe-a"]);
        assert_eq!(refusal.raw, LIMIT_LINE);
    }

    #[test]
    fn limit_refusal_matches_the_stable_phrase_even_without_a_holders_list() {
        let refusal = parse_limit_refusal("The number of VMs exceeds the system limit").unwrap();
        assert!(refusal.holders.is_empty());
        let empty =
            parse_limit_refusal("The number of VMs exceeds the system limit (other running VMs: )")
                .unwrap();
        assert!(empty.holders.is_empty());
    }

    #[test]
    fn limit_refusal_ignores_unrelated_logs() {
        assert_eq!(
            parse_limit_refusal("VNC server is running at vnc://:x@127.0.0.1:1"),
            None
        );
        assert_eq!(parse_limit_refusal(""), None);
    }

    #[test]
    fn log_tail_keeps_the_last_lines_and_bounds_the_bytes() {
        assert_eq!(log_tail("a\nb\nc\nd\n", 2, 100), "c\nd");
        assert_eq!(log_tail("short", 20, 100), "short");
        let tail = log_tail(&"x".repeat(50), 20, 10);
        assert_eq!(tail.len(), 10);
        // A cut inside a multi-byte character must move to the next boundary.
        let tail = log_tail("ééééé", 20, 3);
        assert!(tail.len() <= 3 && tail.chars().all(|c| c == 'é'));
        assert_eq!(log_tail("", 20, 100), "");
    }
}
