//! The stop of an older QEMU guest: through its control sockets, never by a signal.
//!
//! An older meta records no process identity. Its pid proves nothing, because the system
//! recycles a pid and a signal sent to a recycled pid ends a stranger. So this stop signals no
//! process. It asks QEMU to quit on its monitor socket, asks swtpm to shut down on its control
//! socket, and removes the clone only after it has seen that neither socket answers and that no
//! process holds the clone's disk.
//!
//! Every fact about the host comes through [`LegacyStopProbes`], so the decision is a function
//! of the answers and a test supplies them. [`SystemProbes`] is the probe of the host.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{json, Value};

use crate::meta::{VmMeta, VmTool};
use crate::paths::VmPaths;

/// The directory that a starter with no `TMPDIR` used for its session directories.
const FALLBACK_TMP_DIR: &str = "/tmp";

/// True when `meta` is the meta of a QEMU guest that records no identity for QEMU itself. The app
/// stops such a guest through its sockets. A meta with an identity keeps its other stop.
pub fn is_older_qemu_meta(meta: &VmMeta) -> bool {
    meta.tool == VmTool::Qemu && crate::adoption::decide(meta, &NoProcesses) == crate::adoption::Adoption::Legacy
}

/// A host with no process at all. The decision of a meta with no identity does not look at one.
struct NoProcesses;

impl crate::identity::ProcessProbe for NoProcesses {
    fn start_time(&self, _pid: i32) -> Option<u64> {
        None
    }
    fn boot_session(&self) -> Option<String> {
        None
    }
}

/// What the host answers about the sockets and the disk of one guest.
pub trait LegacyStopProbes {
    /// True when something accepts a connection on `socket`. A socket file with no listener, or
    /// no socket file, does not answer.
    fn socket_answers(&self, socket: &Path) -> bool;
    /// Send `quit` on the QEMU monitor at `socket`. True when the command was written.
    fn send_monitor_quit(&self, socket: &Path) -> bool;
    /// Send the shutdown command on the swtpm control socket. True when swtpm answered success.
    fn send_swtpm_shutdown(&self, socket: &Path) -> bool;
    /// The pids of the processes that hold a disk image in `clone_dir`. An error means the host
    /// could not tell, which proves nothing.
    fn disk_holders(&self, clone_dir: &Path) -> Result<Vec<i32>, String>;
    /// Wait between two looks at the guest.
    fn pause(&self, interval: Duration);
}

/// How long the stop waits for QEMU and swtpm to end. Each wait ends when it has looked
/// `attempts + 1` times, or when `limit` of real time has passed, whichever comes first. The
/// limit counts the time of each look, because a look at the holders of a disk runs `lsof`
/// and takes about half a second on a Mac.
#[derive(Debug, Clone, Copy)]
pub struct SettleWait {
    pub attempts: u32,
    pub interval: Duration,
    pub limit: Duration,
}

impl SettleWait {
    /// The wait of the app: 10 s for QEMU to end after `quit`, and 10 s for swtpm after its
    /// shutdown command.
    pub const DEFAULT: SettleWait = SettleWait {
        attempts: 50,
        interval: Duration::from_millis(200),
        limit: Duration::from_secs(10),
    };

    /// A wait that only the number of looks bounds, for a test whose probes need no real time.
    pub const fn looks(attempts: u32) -> SettleWait {
        SettleWait {
            attempts,
            interval: Duration::ZERO,
            limit: Duration::from_secs(3600),
        }
    }
}

/// What still answered or held the disk when the stop gave up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StopBlockers {
    /// The monitor socket, when it still answers.
    pub monitor_socket: Option<PathBuf>,
    /// The swtpm control socket, when it still answers.
    pub swtpm_socket: Option<PathBuf>,
    /// The pids of the processes that hold the clone's disk.
    pub disk_holders: Vec<i32>,
    /// Why the host could not tell who holds the disk.
    pub disk_check_error: Option<String>,
}

impl StopBlockers {
    /// True when nothing answers and nothing holds the disk.
    pub fn is_clear(&self) -> bool {
        self.monitor_socket.is_none()
            && self.swtpm_socket.is_none()
            && self.disk_holders.is_empty()
            && self.disk_check_error.is_none()
    }

    /// The blockers as one sentence for the message of the error.
    pub fn describe(&self) -> String {
        let mut parts = Vec::new();
        if let Some(socket) = &self.monitor_socket {
            parts.push(format!(
                "the QEMU monitor still answers at {}",
                socket.display()
            ));
        }
        if let Some(socket) = &self.swtpm_socket {
            parts.push(format!("swtpm still answers at {}", socket.display()));
        }
        if !self.disk_holders.is_empty() {
            let pids: Vec<String> = self.disk_holders.iter().map(i32::to_string).collect();
            parts.push(format!(
                "process {} still holds the clone's disk",
                pids.join(", ")
            ));
        }
        if let Some(error) = &self.disk_check_error {
            parts.push(format!(
                "the app cannot tell who holds the clone's disk: {error}"
            ));
        }
        parts.join("; ")
    }

    /// The blockers as the `details` of the error.
    pub fn details(&self, id: &str) -> Value {
        json!({
            "vm_id": id,
            "monitor_socket": self.monitor_socket.as_ref().map(|p| p.display().to_string()),
            "swtpm_socket": self.swtpm_socket.as_ref().map(|p| p.display().to_string()),
            "disk_holders": self.disk_holders,
            "disk_check_error": self.disk_check_error,
        })
    }
}

/// The directories that can hold the session of the guest `id`, first choice first: the one
/// the meta records, the one under the app's own temporary directory, and the one under
/// `/tmp`. A starter with another `TMPDIR` put its session elsewhere, and none of these names it.
pub fn session_candidates(id: &str, meta: &VmMeta, paths: &VmPaths) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(recorded) = meta.session_dir.as_deref().filter(|d| !d.is_empty()) {
        candidates.push(PathBuf::from(recorded));
    }
    candidates.push(paths.session_dir(id));
    candidates.push(Path::new(FALLBACK_TMP_DIR).join(format!("testanyware-{id}")));
    candidates
}

/// The first of `candidates` that is a directory.
pub fn find_session_dir(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates.iter().find(|dir| dir.is_dir()).cloned()
}

/// Look at the guest now.
fn observe(
    probes: &(impl LegacyStopProbes + ?Sized),
    monitor: Option<&Path>,
    swtpm: Option<&Path>,
    clone_dir: &Path,
) -> StopBlockers {
    let (disk_holders, disk_check_error) = match probes.disk_holders(clone_dir) {
        Ok(holders) => (holders, None),
        Err(error) => (Vec::new(), Some(error)),
    };
    StopBlockers {
        monitor_socket: monitor
            .filter(|socket| probes.socket_answers(socket))
            .map(Path::to_path_buf),
        swtpm_socket: swtpm
            .filter(|socket| probes.socket_answers(socket))
            .map(Path::to_path_buf),
        disk_holders,
        disk_check_error,
    }
}

/// Look until `done` holds for what is seen, or the wait ends. Returns the last look.
fn settle(
    probes: &(impl LegacyStopProbes + ?Sized),
    wait: SettleWait,
    mut look: impl FnMut() -> StopBlockers,
    done: impl Fn(&StopBlockers) -> bool,
) -> StopBlockers {
    let started = std::time::Instant::now();
    let mut seen = look();
    for _ in 0..wait.attempts {
        if done(&seen) || started.elapsed() >= wait.limit {
            break;
        }
        probes.pause(wait.interval);
        seen = look();
    }
    seen
}

/// Stop an older QEMU guest. `session_dir` is where its sockets are, or `None` when the app found
/// no session directory. `clone_dir` is the directory of its disk.
///
/// The order is the safety of the stop:
/// 1. When the monitor answers, send `quit`. An orderly exit of QEMU also ends its swtpm.
/// 2. Wait until the monitor is silent and no process holds the disk. That is QEMU gone.
/// 3. Only then, when swtpm still answers, send it the shutdown command. A guest that is
///    still running keeps its TPM.
/// 4. Look at everything once more. The stop succeeded only if all of it is clear.
///
/// Nothing here sends a signal, and nothing here removes a file. The caller removes the guest's
/// files when this returns `Ok`.
pub fn stop_through_sockets(
    probes: &(impl LegacyStopProbes + ?Sized),
    wait: SettleWait,
    session_dir: Option<&Path>,
    clone_dir: &Path,
) -> Result<(), StopBlockers> {
    let monitor = session_dir.map(|dir| dir.join("monitor.sock"));
    let swtpm = session_dir.map(crate::swtpm::control_socket);
    let look = || observe(probes, monitor.as_deref(), swtpm.as_deref(), clone_dir);
    let qemu_gone = |seen: &StopBlockers| {
        seen.monitor_socket.is_none()
            && seen.disk_holders.is_empty()
            && seen.disk_check_error.is_none()
    };

    if let Some(socket) = monitor.as_deref() {
        if probes.socket_answers(socket) {
            probes.send_monitor_quit(socket);
        }
    }
    let seen = settle(probes, wait, look, qemu_gone);
    if !qemu_gone(&seen) {
        return Err(seen);
    }

    if let Some(socket) = swtpm.as_deref() {
        if probes.socket_answers(socket) {
            probes.send_swtpm_shutdown(socket);
        }
    }
    let seen = settle(probes, wait, look, StopBlockers::is_clear);
    if seen.is_clear() {
        Ok(())
    } else {
        Err(seen)
    }
}

/// The probe of the host that runs this code.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemProbes;

/// How long one step on a control socket waits.
#[cfg_attr(not(unix), allow(dead_code))]
const SOCKET_STEP: Duration = Duration::from_millis(500);

#[cfg(unix)]
impl LegacyStopProbes for SystemProbes {
    fn socket_answers(&self, socket: &Path) -> bool {
        std::os::unix::net::UnixStream::connect(socket).is_ok()
    }

    fn send_monitor_quit(&self, socket: &Path) -> bool {
        use std::io::Write;
        let Ok(mut stream) = std::os::unix::net::UnixStream::connect(socket) else {
            return false;
        };
        let _ = stream.set_write_timeout(Some(SOCKET_STEP));
        stream
            .write_all(b"quit\n")
            .and_then(|()| stream.flush())
            .is_ok()
    }

    fn send_swtpm_shutdown(&self, socket: &Path) -> bool {
        crate::swtpm::shutdown(socket, SOCKET_STEP)
    }

    fn disk_holders(&self, clone_dir: &Path) -> Result<Vec<i32>, String> {
        let disks = disk_images(clone_dir);
        if disks.is_empty() {
            return Ok(Vec::new());
        }
        // `lsof -t` prints one pid per line, and exits 1 when no process holds any file.
        let output = std::process::Command::new("lsof")
            .arg("-t")
            .args(&disks)
            .output()
            .map_err(|e| format!("lsof: {e}"))?;
        Ok(parse_pids(&String::from_utf8_lossy(&output.stdout)))
    }

    fn pause(&self, interval: Duration) {
        std::thread::sleep(interval);
    }
}

/// On a host with no AF_UNIX sockets nothing answers and nobody can be asked, so the probe
/// reports what it cannot see as a refusal to prove anything.
#[cfg(not(unix))]
impl LegacyStopProbes for SystemProbes {
    fn socket_answers(&self, _socket: &Path) -> bool {
        false
    }
    fn send_monitor_quit(&self, _socket: &Path) -> bool {
        false
    }
    fn send_swtpm_shutdown(&self, _socket: &Path) -> bool {
        false
    }
    fn disk_holders(&self, _clone_dir: &Path) -> Result<Vec<i32>, String> {
        Err("this host cannot list the holders of a file".into())
    }
    fn pause(&self, interval: Duration) {
        std::thread::sleep(interval);
    }
}

/// The qcow2 images in `clone_dir`.
fn disk_images(clone_dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(clone_dir) else {
        return Vec::new();
    };
    let mut disks: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|e| e.to_str()) == Some("qcow2"))
        .collect();
    disks.sort();
    disks
}

/// The positive numbers in `text`, one per line, without repeats.
fn parse_pids(text: &str) -> Vec<i32> {
    let mut pids: Vec<i32> = text
        .lines()
        .filter_map(|line| line.trim().parse::<i32>().ok())
        .filter(|pid| *pid > 0)
        .collect();
    pids.sort_unstable();
    pids.dedup();
    pids
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    /// A guest whose state the test sets. Each look at a field may change it by a script.
    #[derive(Default)]
    struct Fake {
        monitor_up: Cell<bool>,
        swtpm_up: Cell<bool>,
        holders: RefCell<Vec<i32>>,
        holders_error: RefCell<Option<String>>,
        /// `quit` ends QEMU: the monitor goes silent and the disk is free.
        quit_ends_qemu: bool,
        /// The shutdown command ends swtpm.
        shutdown_ends_swtpm: bool,
        /// QEMU's exit also ends swtpm (the `terminate` option).
        qemu_exit_ends_swtpm: bool,
        sent: RefCell<Vec<&'static str>>,
        pauses: Cell<u32>,
    }

    impl LegacyStopProbes for Fake {
        fn socket_answers(&self, socket: &Path) -> bool {
            match socket.file_name().and_then(|n| n.to_str()) {
                Some("monitor.sock") => self.monitor_up.get(),
                Some("swtpm-sock") => self.swtpm_up.get(),
                other => panic!("unexpected socket {other:?}"),
            }
        }
        fn send_monitor_quit(&self, _socket: &Path) -> bool {
            self.sent.borrow_mut().push("quit");
            if self.quit_ends_qemu {
                self.monitor_up.set(false);
                self.holders.borrow_mut().clear();
                if self.qemu_exit_ends_swtpm {
                    self.swtpm_up.set(false);
                }
            }
            true
        }
        fn send_swtpm_shutdown(&self, _socket: &Path) -> bool {
            self.sent.borrow_mut().push("swtpm-shutdown");
            if self.shutdown_ends_swtpm {
                self.swtpm_up.set(false);
            }
            true
        }
        fn disk_holders(&self, _clone_dir: &Path) -> Result<Vec<i32>, String> {
            match self.holders_error.borrow().clone() {
                Some(error) => Err(error),
                None => Ok(self.holders.borrow().clone()),
            }
        }
        fn pause(&self, _interval: Duration) {
            self.pauses.set(self.pauses.get() + 1);
        }
    }

    const WAIT: SettleWait = SettleWait::looks(3);

    fn stop(fake: &Fake) -> Result<(), StopBlockers> {
        stop_through_sockets(fake, WAIT, Some(Path::new("/s")), Path::new("/c"))
    }

    fn running() -> Fake {
        let fake = Fake {
            quit_ends_qemu: true,
            shutdown_ends_swtpm: true,
            ..Fake::default()
        };
        fake.monitor_up.set(true);
        fake.swtpm_up.set(true);
        fake.holders.borrow_mut().push(4242);
        fake
    }

    #[test]
    fn a_monitor_that_answers_gets_quit_and_the_stop_succeeds() {
        let fake = Fake {
            qemu_exit_ends_swtpm: true,
            ..running()
        };
        assert_eq!(stop(&fake), Ok(()));
        // QEMU's orderly exit ended swtpm, so the app sends swtpm nothing.
        assert_eq!(*fake.sent.borrow(), ["quit"]);
    }

    #[test]
    fn a_swtpm_that_survives_qemu_gets_its_shutdown_command() {
        let fake = running();
        assert_eq!(stop(&fake), Ok(()));
        assert_eq!(*fake.sent.borrow(), ["quit", "swtpm-shutdown"]);
    }

    #[test]
    fn a_guest_that_is_already_gone_succeeds_and_sends_nothing() {
        let fake = Fake::default();
        assert_eq!(stop(&fake), Ok(()));
        assert!(fake.sent.borrow().is_empty());
    }

    #[test]
    fn a_silent_monitor_with_a_disk_holder_fails_and_names_the_holder() {
        // The scenario "An older QEMU guest that does not stop".
        let fake = Fake::default();
        fake.holders.borrow_mut().push(777);
        let blockers = stop(&fake).unwrap_err();
        assert_eq!(blockers.disk_holders, [777]);
        assert_eq!(blockers.monitor_socket, None);
        assert!(blockers
            .describe()
            .contains("process 777 still holds the clone's disk"));
        assert!(fake.sent.borrow().is_empty());
        assert_eq!(
            fake.pauses.get(),
            WAIT.attempts,
            "the wait is bounded and used in full"
        );
    }

    #[test]
    fn a_monitor_that_ignores_quit_fails_and_leaves_swtpm_alone() {
        let fake = Fake {
            quit_ends_qemu: false,
            ..running()
        };
        let blockers = stop(&fake).unwrap_err();
        assert!(blockers.monitor_socket.is_some());
        assert_eq!(blockers.disk_holders, [4242]);
        // A running guest keeps its TPM: swtpm gets no shutdown while QEMU is up.
        assert_eq!(*fake.sent.borrow(), ["quit"]);
        assert!(blockers.swtpm_socket.is_some());
    }

    #[test]
    fn a_swtpm_that_ignores_shutdown_fails_and_names_its_socket() {
        let fake = Fake {
            shutdown_ends_swtpm: false,
            ..running()
        };
        let blockers = stop(&fake).unwrap_err();
        assert_eq!(blockers.swtpm_socket, Some(PathBuf::from("/s/swtpm-sock")));
        assert!(blockers.describe().contains("swtpm still answers"));
    }

    #[test]
    fn a_host_that_cannot_list_the_holders_proves_nothing() {
        let fake = Fake::default();
        *fake.holders_error.borrow_mut() = Some("lsof: not found".into());
        let blockers = stop(&fake).unwrap_err();
        assert!(blockers.disk_holders.is_empty());
        assert_eq!(
            blockers.disk_check_error.as_deref(),
            Some("lsof: not found")
        );
        assert!(blockers.describe().contains("cannot tell who holds"));
    }

    #[test]
    fn a_guest_with_no_session_directory_is_judged_by_its_disk_alone() {
        let fake = Fake::default();
        assert_eq!(
            stop_through_sockets(&fake, WAIT, None, Path::new("/c")),
            Ok(())
        );
        fake.holders.borrow_mut().push(9);
        assert!(stop_through_sockets(&fake, WAIT, None, Path::new("/c")).is_err());
    }

    #[test]
    fn the_wait_ends_at_its_limit_however_slow_each_look_is() {
        /// A host whose every look at the disk takes 40 ms, as `lsof` takes time.
        struct SlowLook;
        impl LegacyStopProbes for SlowLook {
            fn socket_answers(&self, _socket: &Path) -> bool {
                false
            }
            fn send_monitor_quit(&self, _socket: &Path) -> bool {
                true
            }
            fn send_swtpm_shutdown(&self, _socket: &Path) -> bool {
                true
            }
            fn disk_holders(&self, _clone_dir: &Path) -> Result<Vec<i32>, String> {
                std::thread::sleep(Duration::from_millis(40));
                Ok(vec![1])
            }
            fn pause(&self, _interval: Duration) {}
        }
        let wait = SettleWait {
            attempts: 100_000,
            interval: Duration::ZERO,
            limit: Duration::from_millis(200),
        };
        let started = std::time::Instant::now();
        let blockers = stop_through_sockets(&SlowLook, wait, None, Path::new("/c")).unwrap_err();
        assert_eq!(blockers.disk_holders, [1]);
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "the wait ran {:?}, far past its limit",
            started.elapsed()
        );
    }

    #[test]
    fn a_qemu_that_needs_a_moment_to_exit_is_waited_for() {
        /// QEMU frees the disk on the third look, as an exiting process does.
        struct Slow {
            looks: Cell<u32>,
        }
        impl LegacyStopProbes for Slow {
            fn socket_answers(&self, _socket: &Path) -> bool {
                false
            }
            fn send_monitor_quit(&self, _socket: &Path) -> bool {
                true
            }
            fn send_swtpm_shutdown(&self, _socket: &Path) -> bool {
                true
            }
            fn disk_holders(&self, _clone_dir: &Path) -> Result<Vec<i32>, String> {
                self.looks.set(self.looks.get() + 1);
                Ok(if self.looks.get() < 3 {
                    vec![5]
                } else {
                    Vec::new()
                })
            }
            fn pause(&self, _interval: Duration) {}
        }
        let slow = Slow {
            looks: Cell::new(0),
        };
        assert_eq!(
            stop_through_sockets(&slow, WAIT, Some(Path::new("/s")), Path::new("/c")),
            Ok(())
        );
    }

    #[test]
    fn the_candidates_are_the_recorded_dir_then_the_own_temp_dir_then_tmp() {
        let paths = VmPaths::new("/st".into(), "/da".into(), "/own".into());
        let mut meta = VmMeta::new("testanyware-ab", VmTool::Qemu, 5);
        assert_eq!(
            session_candidates("testanyware-ab", &meta, &paths),
            [
                PathBuf::from("/own/testanyware-testanyware-ab"),
                PathBuf::from("/tmp/testanyware-testanyware-ab"),
            ]
        );
        meta.session_dir = Some("/rec/s".into());
        assert_eq!(
            session_candidates("testanyware-ab", &meta, &paths)[0],
            PathBuf::from("/rec/s")
        );
    }

    #[test]
    fn the_first_existing_candidate_wins() {
        let dir = tempfile::tempdir().unwrap();
        let second = dir.path().join("second");
        let third = dir.path().join("third");
        std::fs::create_dir(&second).unwrap();
        std::fs::create_dir(&third).unwrap();
        let first = dir.path().join("first");
        assert_eq!(
            find_session_dir(&[first.clone(), second.clone(), third]),
            Some(second)
        );
        assert_eq!(find_session_dir(&[first]), None);
    }

    #[test]
    fn only_a_qemu_meta_without_any_identity_is_older() {
        use crate::identity::ProcessIdentity;
        let older = VmMeta::new("v", VmTool::Qemu, 5);
        assert!(is_older_qemu_meta(&older));
        assert!(!is_older_qemu_meta(&VmMeta::new("v", VmTool::Tart, 5)));
        let identity = ProcessIdentity {
            start_time: 1,
            boot_session: "b".into(),
        };
        let with = VmMeta {
            identity: Some(identity.clone()),
            ..older.clone()
        };
        assert!(!is_older_qemu_meta(&with));
        // The main process decides. A meta with a helper's identity and none for QEMU proves
        // nothing about QEMU, so it gets the stop that signals nothing.
        for helper_only in [
            VmMeta {
                keeper_identity: Some(identity.clone()),
                ..older.clone()
            },
            VmMeta {
                swtpm_identity: Some(identity),
                ..older.clone()
            },
        ] {
            assert!(is_older_qemu_meta(&helper_only));
        }
    }

    #[test]
    fn the_pids_of_lsof_are_positive_numbers_without_repeats() {
        assert_eq!(parse_pids("12\n7\n12\nx\n-3\n0\n"), [7, 12]);
        assert!(parse_pids("").is_empty());
    }

    #[test]
    fn the_details_name_every_blocker() {
        let blockers = StopBlockers {
            monitor_socket: Some("/s/monitor.sock".into()),
            swtpm_socket: None,
            disk_holders: vec![3],
            disk_check_error: None,
        };
        let details = blockers.details("v");
        assert_eq!(details["vm_id"], "v");
        assert_eq!(details["monitor_socket"], "/s/monitor.sock");
        assert_eq!(details["swtpm_socket"], Value::Null);
        assert_eq!(details["disk_holders"], json!([3]));
    }

    #[cfg(unix)]
    mod host {
        use super::*;
        use std::io::Read;

        #[test]
        fn a_socket_with_a_listener_answers_and_a_stale_socket_file_does_not() {
            let dir = tempfile::tempdir().unwrap();
            let live = dir.path().join("live");
            let _listener = std::os::unix::net::UnixListener::bind(&live).unwrap();
            assert!(SystemProbes.socket_answers(&live));

            let stale = dir.path().join("stale");
            drop(std::os::unix::net::UnixListener::bind(&stale).unwrap());
            assert!(stale.exists(), "a dropped listener leaves its socket file");
            // Another test thread may fork a child that holds a copy of the closed listener's
            // descriptor until its `exec`, so the refusal can come a moment late.
            let mut answers = true;
            for _ in 0..50 {
                answers = SystemProbes.socket_answers(&stale);
                if !answers {
                    break;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            assert!(!answers, "a socket file with no listener answered");
            assert!(!SystemProbes.socket_answers(&dir.path().join("none")));
        }

        #[test]
        fn quit_reaches_the_monitor_as_a_line() {
            let dir = tempfile::tempdir().unwrap();
            let socket = dir.path().join("monitor.sock");
            let listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();
            let server = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                let mut line = String::new();
                let mut byte = [0u8; 1];
                while stream.read(&mut byte).unwrap() == 1 && byte[0] != b'\n' {
                    line.push(byte[0] as char);
                }
                line
            });
            assert!(SystemProbes.send_monitor_quit(&socket));
            assert_eq!(server.join().unwrap(), "quit");
        }

        #[test]
        fn the_disk_holders_are_the_processes_that_have_a_qcow2_open() {
            if std::process::Command::new("lsof")
                .arg("-v")
                .output()
                .is_err()
            {
                eprintln!("skipped: lsof is not installed");
                return;
            }
            let dir = tempfile::tempdir().unwrap();
            let disk = dir.path().join("testanyware-x.qcow2");
            std::fs::write(&disk, b"x").unwrap();
            assert_eq!(SystemProbes.disk_holders(dir.path()), Ok(Vec::new()));

            // A child that holds the disk open, as QEMU does.
            let mut child = std::process::Command::new("sh")
                .arg("-c")
                .arg("exec 3<\"$1\"; sleep 30")
                .arg("holder")
                .arg(&disk)
                .spawn()
                .unwrap();
            let pid = child.id() as i32;
            // The shell opens the file and execs sleep; wait until lsof sees it.
            let mut seen = Vec::new();
            for _ in 0..50 {
                seen = SystemProbes.disk_holders(dir.path()).unwrap();
                if !seen.is_empty() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            let _ = child.kill();
            let _ = child.wait();
            assert!(seen.contains(&pid), "holders {seen:?} lack {pid}");
        }
    }
}
