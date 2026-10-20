//! Adoption: what the app may do with a VM it did not start in its current run.
//!
//! [`decide`] is a pure function of a meta and the live facts of the main process, which a
//! [`ProcessProbe`] gives. It returns one [`Adoption`]. The same comparison, [`check_process`],
//! guards every signal: a process receives a signal only when its pid, its start time and the
//! boot session all match what the meta recorded (spec: "Lifecycle and adoption").
//!
//! The decision looks at the main process only, because that process is the VM. The keeper and
//! swtpm are checked again, one by one, at the moment of a signal. So a decision made earlier
//! never authorises a signal.

use serde_json::{json, Value};

use crate::identity::{ProcessIdentity, ProcessProbe};
use crate::meta::VmMeta;

/// Why a live process is not the one that the meta recorded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mismatch {
    /// The pid names a process that started at another time.
    StartTime { recorded: u64, live: u64 },
    /// The pid was recorded in another boot session, so the host started again since.
    BootSession { recorded: String, live: String },
    /// The host names no boot session, so no identity can be proven.
    NoBootSession,
}

impl Mismatch {
    /// A stable word for the reason, for the status and for tests.
    pub fn code(&self) -> &'static str {
        match self {
            Mismatch::StartTime { .. } => "start_time_differs",
            Mismatch::BootSession { .. } => "boot_session_differs",
            Mismatch::NoBootSession => "no_boot_session",
        }
    }

    /// The reason in a sentence.
    pub fn describe(&self) -> String {
        match self {
            Mismatch::StartTime { recorded, live } => format!(
                "the process now at the pid started at {live}, and the meta recorded {recorded}"
            ),
            Mismatch::BootSession { recorded, live } => format!(
                "the meta recorded boot session {recorded}, and this host runs boot session {live}"
            ),
            Mismatch::NoBootSession => {
                "this host names no boot session, so the pid cannot be proven".to_string()
            }
        }
    }
}

/// The result of comparing one recorded process with the live process at its pid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessCheck {
    /// The pid, the start time and the boot session all match.
    Match,
    /// No process has the pid.
    Gone,
    /// A process has the pid, and it is not the recorded one.
    Other(Mismatch),
}

/// Compare the process at `pid` with `recorded`. A pid that is not positive names no process.
pub fn check_process(
    probe: &impl ProcessProbe,
    pid: i32,
    recorded: &ProcessIdentity,
) -> ProcessCheck {
    if pid <= 0 {
        return ProcessCheck::Gone;
    }
    let Some(live_start) = probe.start_time(pid) else {
        return ProcessCheck::Gone;
    };
    let Some(live_session) = probe.boot_session().filter(|session| !session.is_empty()) else {
        return ProcessCheck::Other(Mismatch::NoBootSession);
    };
    if live_session != recorded.boot_session {
        return ProcessCheck::Other(Mismatch::BootSession {
            recorded: recorded.boot_session.clone(),
            live: live_session,
        });
    }
    if live_start != recorded.start_time {
        return ProcessCheck::Other(Mismatch::StartTime {
            recorded: recorded.start_time,
            live: live_start,
        });
    }
    ProcessCheck::Match
}

/// The pid to signal for a recorded process: `Some(pid)` only on a full match. No pid, no
/// recorded identity, a process that is gone and a stranger at the pid all give `None`.
pub fn verified_pid(
    probe: &impl ProcessProbe,
    pid: Option<i32>,
    recorded: Option<&ProcessIdentity>,
) -> Option<i32> {
    let (pid, recorded) = (pid?, recorded?);
    (check_process(probe, pid, recorded) == ProcessCheck::Match).then_some(pid)
}

/// What the app does with one VM at its start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Adoption {
    /// The main process is the recorded one. The app serves the VM and may signal the
    /// processes that still match.
    Adopt,
    /// The main process is gone. The app reports the VM as stopped and leaves its records for
    /// `vm stop`.
    Stopped,
    /// The pid names another process. The app signals nothing and reports the VM.
    Doubt { pid: i32, mismatch: Mismatch },
    /// The meta records no identity, so it is an older one. The app serves the streams, and it
    /// stops the VM by name or through its control sockets, never by a signal.
    Legacy,
}

/// Decide what the app does with the VM that `meta` records.
pub fn decide(meta: &VmMeta, probe: &impl ProcessProbe) -> Adoption {
    let Some(recorded) = meta.identity.as_ref() else {
        return Adoption::Legacy;
    };
    match check_process(probe, meta.pid, recorded) {
        ProcessCheck::Match => Adoption::Adopt,
        ProcessCheck::Gone => Adoption::Stopped,
        ProcessCheck::Other(mismatch) => Adoption::Doubt {
            pid: meta.pid,
            mismatch,
        },
    }
}

/// The `details` that name a pid that the app did not trust.
pub fn doubt_details(id: &str, pid: i32, mismatch: &Mismatch) -> Value {
    json!({
        "vm_id": id,
        "pid": pid,
        "reason": mismatch.code(),
        "detail": mismatch.describe(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meta::VmTool;

    /// A host with one live process, or none.
    struct Host {
        start: Option<u64>,
        session: Option<&'static str>,
    }

    impl ProcessProbe for Host {
        fn start_time(&self, _pid: i32) -> Option<u64> {
            self.start
        }
        fn boot_session(&self) -> Option<String> {
            self.session.map(str::to_string)
        }
    }

    fn identity(start_time: u64, session: &str) -> ProcessIdentity {
        ProcessIdentity {
            start_time,
            boot_session: session.into(),
        }
    }

    fn meta_with(identity: Option<ProcessIdentity>) -> VmMeta {
        VmMeta {
            identity,
            ..VmMeta::new("testanyware-ab12cd34", VmTool::Tart, 4242)
        }
    }

    #[test]
    fn a_process_that_matches_on_all_three_values_is_adopted() {
        let host = Host {
            start: Some(100),
            session: Some("boot-a"),
        };
        assert_eq!(
            decide(&meta_with(Some(identity(100, "boot-a"))), &host),
            Adoption::Adopt
        );
    }

    #[test]
    fn a_process_that_is_gone_is_reported_as_stopped() {
        let host = Host {
            start: None,
            session: Some("boot-a"),
        };
        assert_eq!(
            decide(&meta_with(Some(identity(100, "boot-a"))), &host),
            Adoption::Stopped
        );
    }

    #[test]
    fn a_reused_pid_is_doubt_and_names_the_start_times() {
        let host = Host {
            start: Some(999),
            session: Some("boot-a"),
        };
        assert_eq!(
            decide(&meta_with(Some(identity(100, "boot-a"))), &host),
            Adoption::Doubt {
                pid: 4242,
                mismatch: Mismatch::StartTime {
                    recorded: 100,
                    live: 999
                }
            }
        );
    }

    #[test]
    fn a_pid_from_an_earlier_boot_is_doubt_even_with_the_same_start_time() {
        let host = Host {
            start: Some(100),
            session: Some("boot-b"),
        };
        let decided = decide(&meta_with(Some(identity(100, "boot-a"))), &host);
        assert!(
            matches!(
                decided,
                Adoption::Doubt {
                    mismatch: Mismatch::BootSession { .. },
                    ..
                }
            ),
            "{decided:?}"
        );
    }

    #[test]
    fn a_host_with_no_boot_session_proves_nothing_about_a_live_pid() {
        for session in [None, Some("")] {
            let host = Host {
                start: Some(100),
                session,
            };
            assert_eq!(
                decide(&meta_with(Some(identity(100, "boot-a"))), &host),
                Adoption::Doubt {
                    pid: 4242,
                    mismatch: Mismatch::NoBootSession
                },
                "session {session:?}"
            );
        }
    }

    #[test]
    fn a_meta_with_no_identity_is_legacy_whatever_the_host_says() {
        for start in [None, Some(100)] {
            let host = Host {
                start,
                session: Some("boot-a"),
            };
            assert_eq!(decide(&meta_with(None), &host), Adoption::Legacy);
        }
    }

    #[test]
    fn a_pid_that_is_not_positive_is_gone_and_is_never_probed() {
        struct Panicking;
        impl ProcessProbe for Panicking {
            fn start_time(&self, _: i32) -> Option<u64> {
                panic!("probed");
            }
            fn boot_session(&self) -> Option<String> {
                panic!("probed");
            }
        }
        for pid in [0, -1] {
            assert_eq!(
                check_process(&Panicking, pid, &identity(1, "boot-a")),
                ProcessCheck::Gone
            );
        }
    }

    #[test]
    fn a_pid_is_verified_only_on_a_full_match_with_a_recorded_identity() {
        let matching = Host {
            start: Some(100),
            session: Some("boot-a"),
        };
        let recorded = identity(100, "boot-a");
        assert_eq!(verified_pid(&matching, Some(7), Some(&recorded)), Some(7));
        // No pid, or no recorded identity: nothing to prove, so nothing to signal.
        assert_eq!(verified_pid(&matching, None, Some(&recorded)), None);
        assert_eq!(verified_pid(&matching, Some(7), None), None);
        let stranger = Host {
            start: Some(101),
            session: Some("boot-a"),
        };
        assert_eq!(verified_pid(&stranger, Some(7), Some(&recorded)), None);
        let gone = Host {
            start: None,
            session: Some("boot-a"),
        };
        assert_eq!(verified_pid(&gone, Some(7), Some(&recorded)), None);
    }

    #[test]
    fn the_details_of_a_doubt_name_the_pid_and_the_reason() {
        let details = doubt_details(
            "v",
            9,
            &Mismatch::StartTime {
                recorded: 1,
                live: 2,
            },
        );
        assert_eq!(details["vm_id"], "v");
        assert_eq!(details["pid"], 9);
        assert_eq!(details["reason"], "start_time_differs");
    }
}
