//! The identity of a process: the proof that a recorded pid still names the process that
//! `vm start` recorded.
//!
//! A pid alone proves nothing. The system recycles a pid after its process ends, and a signal
//! sent to a recycled pid ends a stranger. So the meta records, beside each pid, the start
//! time of the process and the UUID of the boot session. The app signals a process only when
//! the pid, the start time and the boot session all match what the meta recorded.
//!
//! This module reads the identity and nothing else. [`read_identity`] is a pure function over
//! a [`ProcessProbe`], so a test supplies the answers of a probe and needs no process.
//! [`SystemProbe`] is the probe of the host.

use serde::{Deserialize, Serialize};

/// What distinguishes a process from every other process that has had the same pid.
///
/// The start time has no unit that is common to the hosts. It is the microseconds since the
/// Unix epoch on macOS, and the clock ticks since boot on Linux. The reader compares it for
/// equality with a value that the same host recorded, in the same boot session, and does
/// nothing else with it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessIdentity {
    pub start_time: u64,
    /// The UUID of the boot session. A reboot starts the clock again, so a start time without
    /// it can match a different process of a later boot.
    pub boot_session: String,
}

/// What a host answers about a process. A test implements it with fixed answers.
pub trait ProcessProbe {
    /// The start time of `pid`, or `None` when no such process exists.
    fn start_time(&self, pid: i32) -> Option<u64>;
    /// The UUID of the current boot session, or `None` when the host does not give one.
    fn boot_session(&self) -> Option<String>;
}

/// The identity of `pid` now, or `None` when the process does not exist or the host cannot
/// name its boot session. An identity that lacks either value proves nothing, so none is made.
pub fn read_identity(probe: &impl ProcessProbe, pid: i32) -> Option<ProcessIdentity> {
    if pid <= 0 {
        return None;
    }
    let boot_session = probe.boot_session().filter(|session| !session.is_empty())?;
    let start_time = probe.start_time(pid)?;
    Some(ProcessIdentity {
        start_time,
        boot_session,
    })
}

/// The identity of `pid` when there is a pid to read. The meta holds an optional pid for
/// each process that may not exist, such as the keeper of a QEMU guest.
pub fn read_optional_identity(
    probe: &impl ProcessProbe,
    pid: Option<i32>,
) -> Option<ProcessIdentity> {
    read_identity(probe, pid?)
}

/// The probe of the host that runs this code.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemProbe;

#[cfg(target_os = "macos")]
impl ProcessProbe for SystemProbe {
    fn start_time(&self, pid: i32) -> Option<u64> {
        let mut info = std::mem::MaybeUninit::<libc::proc_bsdinfo>::zeroed();
        let size = std::mem::size_of::<libc::proc_bsdinfo>() as libc::c_int;
        // SAFETY: `info` is a writable buffer of exactly `size` bytes, and `proc_pidinfo`
        // writes at most `size` bytes into it.
        let written = unsafe {
            libc::proc_pidinfo(
                pid,
                libc::PROC_PIDTBSDINFO,
                0,
                info.as_mut_ptr().cast(),
                size,
            )
        };
        if written != size {
            return None;
        }
        // SAFETY: `proc_pidinfo` filled the whole structure.
        let info = unsafe { info.assume_init() };
        Some(info.pbi_start_tvsec * 1_000_000 + info.pbi_start_tvusec)
    }

    fn boot_session(&self) -> Option<String> {
        let name = c"kern.bootsessionuuid";
        let mut buffer = [0u8; 64];
        let mut len = buffer.len();
        // SAFETY: `buffer` and `len` describe one writable buffer, and `name` ends with a NUL.
        let status = unsafe {
            libc::sysctlbyname(
                name.as_ptr(),
                buffer.as_mut_ptr().cast(),
                &mut len,
                std::ptr::null_mut(),
                0,
            )
        };
        if status != 0 {
            return None;
        }
        let text = std::str::from_utf8(&buffer[..len.min(buffer.len())]).ok()?;
        Some(text.trim_end_matches('\0').trim().to_string())
    }
}

#[cfg(target_os = "linux")]
impl ProcessProbe for SystemProbe {
    fn start_time(&self, pid: i32) -> Option<u64> {
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
        parse_proc_stat_start_time(&stat)
    }

    fn boot_session(&self) -> Option<String> {
        let id = std::fs::read_to_string("/proc/sys/kernel/random/boot_id").ok()?;
        Some(id.trim().to_string())
    }
}

/// Other hosts (Windows) run no local QEMU, so no process there has an identity to prove.
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
impl ProcessProbe for SystemProbe {
    fn start_time(&self, _pid: i32) -> Option<u64> {
        None
    }

    fn boot_session(&self) -> Option<String> {
        None
    }
}

/// Field 22 of `/proc/<pid>/stat`: the start time in clock ticks since boot. The second field
/// is the command name in parentheses, and it may hold spaces and parentheses, so the fields
/// count from the last `)`. The text after it starts at field 3.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn parse_proc_stat_start_time(stat: &str) -> Option<u64> {
    let after_command = &stat[stat.rfind(')')? + 1..];
    after_command.split_whitespace().nth(22 - 3)?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FixedProbe {
        start: Option<u64>,
        session: Option<&'static str>,
    }

    impl ProcessProbe for FixedProbe {
        fn start_time(&self, _pid: i32) -> Option<u64> {
            self.start
        }

        fn boot_session(&self) -> Option<String> {
            self.session.map(str::to_string)
        }
    }

    #[test]
    fn an_identity_is_the_start_time_with_the_boot_session() {
        let probe = FixedProbe {
            start: Some(1234),
            session: Some("boot-a"),
        };
        assert_eq!(
            read_identity(&probe, 42),
            Some(ProcessIdentity {
                start_time: 1234,
                boot_session: "boot-a".into()
            })
        );
    }

    #[test]
    fn a_process_that_does_not_exist_has_no_identity() {
        let probe = FixedProbe {
            start: None,
            session: Some("boot-a"),
        };
        assert_eq!(read_identity(&probe, 42), None);
    }

    #[test]
    fn a_host_with_no_boot_session_gives_no_identity() {
        for session in [None, Some("")] {
            let probe = FixedProbe {
                start: Some(1),
                session,
            };
            assert_eq!(read_identity(&probe, 42), None, "session {session:?}");
        }
    }

    #[test]
    fn a_pid_that_is_not_positive_is_never_probed() {
        struct Panicking;
        impl ProcessProbe for Panicking {
            fn start_time(&self, _: i32) -> Option<u64> {
                panic!("probed");
            }
            fn boot_session(&self) -> Option<String> {
                panic!("probed");
            }
        }
        for pid in [0, -1, i32::MIN] {
            assert_eq!(read_identity(&Panicking, pid), None);
        }
        assert_eq!(read_optional_identity(&Panicking, None), None);
    }

    #[test]
    fn the_start_time_is_field_22_even_when_the_command_name_holds_parentheses() {
        // `(a) b c)` is a legal command name. The 20 fields after the state letter come next.
        let stat = "123 (a) b c) S 1 123 123 0 -1 4194560 100 0 0 0 1 2 0 0 20 0 1 0 987654 1 2";
        assert_eq!(parse_proc_stat_start_time(stat), Some(987654));
        assert_eq!(parse_proc_stat_start_time("123 (short) S 1"), None);
        assert_eq!(parse_proc_stat_start_time("no command name"), None);
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    fn the_host_probe_reads_a_stable_identity_of_a_live_process() {
        let me = std::process::id() as i32;
        let first = read_identity(&SystemProbe, me).expect("the identity of this process");
        let second = read_identity(&SystemProbe, me).expect("the identity again");
        assert_eq!(
            first, second,
            "an identity does not change while the process lives"
        );
        assert!(first.start_time > 0 && !first.boot_session.is_empty());
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    fn the_host_probe_tells_two_processes_apart_and_misses_a_dead_pid() {
        let mut child = std::process::Command::new("sleep")
            .arg("30")
            .spawn()
            .unwrap();
        let child_pid = child.id() as i32;
        let mine = read_identity(&SystemProbe, std::process::id() as i32).unwrap();
        let theirs = read_identity(&SystemProbe, child_pid);
        let _ = child.kill();
        let _ = child.wait();

        let theirs = theirs.expect("the identity of the child");
        assert_eq!(theirs.boot_session, mine.boot_session, "one boot session");
        assert!(
            read_identity(&SystemProbe, i32::MAX).is_none(),
            "no such process"
        );
        assert!(
            read_identity(&SystemProbe, child_pid).is_none(),
            "the child is reaped"
        );
    }
}
