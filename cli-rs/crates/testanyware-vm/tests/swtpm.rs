//! The real `swtpm`, started with the arguments of `vm start`: it names its pid, it ends when
//! its control connection closes, and `CMD_SHUTDOWN` on the control socket stops it before any
//! client has connected. A host with no `swtpm` skips these tests.

#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use testanyware_vm::identity::{read_identity, SystemProbe};
use testanyware_vm::process::process_alive;
use testanyware_vm::swtpm;

/// The first `swtpm` on `PATH`, or `None`.
fn find_swtpm() -> Option<PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|dir| dir.join("swtpm"))
        .find(|candidate| candidate.is_file())
}

struct Daemon {
    // A unix socket path holds about 100 bytes, so the session is a short directory in `/tmp`.
    session: tempfile::TempDir,
    pid: i32,
}

impl Daemon {
    fn socket(&self) -> PathBuf {
        swtpm::control_socket(self.session.path())
    }
}

fn start(swtpm_bin: &Path) -> Daemon {
    let session = tempfile::Builder::new()
        .prefix("tas")
        .tempdir_in("/tmp")
        .unwrap();
    let tpm_dir = session.path().join("tpm");
    std::fs::create_dir(&tpm_dir).unwrap();
    let args = swtpm::launch_args(
        &tpm_dir,
        &swtpm::control_socket(session.path()),
        &swtpm::pid_file(session.path()),
    );
    let status = std::process::Command::new(swtpm_bin)
        .args(&args)
        .status()
        .unwrap();
    assert!(status.success(), "swtpm {args:?}: {status}");
    let pid_file = swtpm::pid_file(session.path());
    let deadline = Instant::now() + Duration::from_secs(5);
    let pid = loop {
        if let Some(pid) = swtpm::read_pid(&pid_file) {
            break pid;
        }
        assert!(Instant::now() < deadline, "swtpm wrote no pid file");
        std::thread::sleep(Duration::from_millis(20));
    };
    Daemon { session, pid }
}

fn ends_within(pid: i32, limit: Duration) -> bool {
    let deadline = Instant::now() + limit;
    while process_alive(pid) {
        if Instant::now() > deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    true
}

#[test]
fn the_pid_file_names_the_daemon_and_its_identity_is_readable() {
    let Some(bin) = find_swtpm() else {
        eprintln!("skipped: swtpm is not installed");
        return;
    };
    let daemon = start(&bin);
    assert!(process_alive(daemon.pid));
    assert!(read_identity(&SystemProbe, daemon.pid).is_some());
    assert!(swtpm::shutdown(&daemon.socket(), Duration::from_secs(2)));
    assert!(ends_within(daemon.pid, Duration::from_secs(5)));
}

#[test]
fn swtpm_ends_when_its_control_connection_closes() {
    let Some(bin) = find_swtpm() else {
        eprintln!("skipped: swtpm is not installed");
        return;
    };
    let daemon = start(&bin);
    // QEMU holds this connection. When QEMU ends, the connection closes.
    let connection = std::os::unix::net::UnixStream::connect(daemon.socket()).unwrap();
    assert!(
        process_alive(daemon.pid),
        "swtpm lives while the connection is open"
    );
    drop(connection);
    assert!(
        ends_within(daemon.pid, Duration::from_secs(5)),
        "swtpm {} outlived its control connection",
        daemon.pid
    );
}

#[test]
fn shutdown_stops_a_swtpm_that_no_client_ever_connected_to() {
    let Some(bin) = find_swtpm() else {
        eprintln!("skipped: swtpm is not installed");
        return;
    };
    let daemon = start(&bin);
    // The control: `terminate` alone ends nothing before a first connection.
    std::thread::sleep(Duration::from_millis(500));
    assert!(
        process_alive(daemon.pid),
        "swtpm waits for a first connection"
    );

    assert!(swtpm::shutdown(&daemon.socket(), Duration::from_secs(2)));
    assert!(
        ends_within(daemon.pid, Duration::from_secs(5)),
        "CMD_SHUTDOWN ends swtpm"
    );
    assert!(
        !swtpm::shutdown(&daemon.socket(), Duration::from_millis(300)),
        "no swtpm answers on the socket any more"
    );
}

#[test]
fn the_teardown_of_a_start_that_failed_before_qemu_connected_stops_swtpm_through_its_socket() {
    let Some(bin) = find_swtpm() else {
        eprintln!("skipped: swtpm is not installed");
        return;
    };
    let daemon = start(&bin);
    // The TPM state is not under `<clone>/<id>-tpm`, so the `pgrep` backstop of the teardown
    // cannot match this swtpm. Only the control socket can stop it.
    let clone_dir = daemon.session.path().join("clone-that-never-was");
    assert!(process_alive(daemon.pid));

    testanyware_vm::qemu::teardown(0, &clone_dir, daemon.session.path());

    assert!(
        ends_within(daemon.pid, Duration::from_secs(5)),
        "swtpm {} survived",
        daemon.pid
    );
    assert!(
        !daemon.session.path().exists(),
        "the teardown removes the session directory"
    );
}
