//! Adoption at the start of the app, through `GET /v1/status`.
//!
//! Each test writes the sidecars of a few VMs, starts the real `testanyware-app` executable in
//! test mode, and reads what it reports. A VM's process is a `sleep` that the test orphans. Its
//! agent is a fake HTTP server and its VNC server a fake that sends the RFB banner, both on
//! loopback. No test starts a real guest.

#![cfg(unix)]

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tempfile::TempDir;
use testanyware_vm::identity::{read_identity, SystemProbe};

/// Every test holds this lock: macOS gives a new socket no atomic close-on-exec, so a thread that
/// spawns the app can hand its child a socket that a sibling test has just opened.
static SERIAL: Mutex<()> = Mutex::new(());

fn serial() -> MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|p| p.into_inner())
}

const APP: &str = env!("CARGO_BIN_EXE_testanyware-app");

/// A process that outlives the test's interest in it, orphaned so that nothing has to reap it.
struct Orphan(i32);

impl Orphan {
    fn start() -> Self {
        let out = Command::new("sh")
            .args(["-c", "sleep 600 </dev/null >/dev/null 2>&1 & echo $!"])
            .output()
            .expect("run sh");
        Orphan(String::from_utf8_lossy(&out.stdout).trim().parse().expect("a pid"))
    }

    fn alive(&self) -> bool {
        Command::new("kill")
            .args(["-0", &self.0.to_string()])
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    /// The identity of this process as the host reads it now, as the JSON of a meta.
    fn identity(&self) -> Value {
        serde_json::to_value(read_identity(&SystemProbe, self.0).expect("a live process")).unwrap()
    }
}

impl Drop for Orphan {
    fn drop(&mut self) {
        let _ = Command::new("kill")
            .args(["-9", &self.0.to_string()])
            .stderr(Stdio::null())
            .status();
    }
}

/// A fake guest service on loopback. It counts the connections that it accepts.
struct Fake {
    addr: SocketAddr,
    accepted: Arc<AtomicUsize>,
}

enum Behaviour {
    /// Read a request, answer `HTTP/1.1 200 OK`.
    Agent,
    /// Send the RFB banner at once, then read until the client leaves.
    Vnc,
    /// Accept, then say nothing.
    Silent,
}

impl Fake {
    fn start(behaviour: Behaviour) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let accepted = Arc::new(AtomicUsize::new(0));
        let count = Arc::clone(&accepted);
        std::thread::spawn(move || {
            for conn in listener.incoming().flatten() {
                count.fetch_add(1, Ordering::SeqCst);
                let behaviour = match behaviour {
                    Behaviour::Agent => 0,
                    Behaviour::Vnc => 1,
                    Behaviour::Silent => 2,
                };
                std::thread::spawn(move || serve(conn, behaviour));
            }
        });
        Fake { addr, accepted }
    }

    fn accepted(&self) -> usize {
        self.accepted.load(Ordering::SeqCst)
    }
}

fn serve(mut conn: std::net::TcpStream, behaviour: u8) {
    let mut buf = [0u8; 1024];
    match behaviour {
        0 => {
            let _ = conn.read(&mut buf);
            let _ = conn.write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\n\r\n{}");
        }
        1 => {
            let _ = conn.write_all(b"RFB 003.008\n");
            while conn.read(&mut buf).map(|n| n > 0).unwrap_or(false) {}
        }
        _ => while conn.read(&mut buf).map(|n| n > 0).unwrap_or(false) {},
    }
}

struct Sandbox {
    tmp: TempDir,
}

impl Sandbox {
    fn new() -> Self {
        Sandbox { tmp: tempfile::tempdir().unwrap() }
    }

    fn vms_dir(&self) -> PathBuf {
        self.tmp.path().join("state").join("vms")
    }

    /// Write a VM's two sidecars. `meta` is the meta without its id.
    fn add_vm(&self, id: &str, mut meta: Value, agent: Option<&Fake>, vnc: &Fake) {
        std::fs::create_dir_all(self.vms_dir()).unwrap();
        meta["id"] = json!(id);
        let mut spec = json!({
            "vnc": { "host": "127.0.0.1", "port": vnc.addr.port() },
            "platform": "linux",
        });
        if let Some(agent) = agent {
            spec["agent"] = json!({ "host": "127.0.0.1", "port": agent.addr.port() });
        }
        std::fs::write(self.vms_dir().join(format!("{id}.json")), spec.to_string()).unwrap();
        std::fs::write(self.vms_dir().join(format!("{id}.meta.json")), meta.to_string()).unwrap();
    }

    fn meta_path(&self, id: &str) -> PathBuf {
        self.vms_dir().join(format!("{id}.meta.json"))
    }
}

struct App {
    child: Child,
    socket: PathBuf,
}

impl Drop for App {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn start_app(sandbox: &Sandbox, probe_deadline_ms: u64) -> App {
    let socket = sandbox.tmp.path().join("run").join("testanyware.sock");
    let mut child = Command::new(APP)
        .args(["--test-mode", "--socket"])
        .arg(&socket)
        .arg("--state-dir")
        .arg(sandbox.tmp.path().join("state"))
        .arg("--data-dir")
        .arg(sandbox.tmp.path().join("data"))
        .arg("--tool-dir")
        .arg(sandbox.tmp.path().join("no-tools"))
        .args(["--test-probe-deadline-ms", &probe_deadline_ms.to_string()])
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn the app");
    let deadline = Instant::now() + Duration::from_secs(10);
    while UnixStream::connect(&socket).is_err() {
        assert!(Instant::now() < deadline, "the app did not serve in 10 seconds");
        if let Some(status) = child.try_wait().unwrap() {
            panic!("the app exited early with {status}");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    App { child, socket }
}

/// `GET /v1/status`, which skips the version handshake.
fn status(socket: &Path) -> Value {
    let mut stream = UnixStream::connect(socket).expect("connect");
    stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    stream
        .write_all(b"GET /v1/status HTTP/1.1\r\nHost: testanyware\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut text = String::new();
    stream.read_to_string(&mut text).expect("read the answer");
    let (_, body) = text.split_once("\r\n\r\n").expect("a header block");
    serde_json::from_str(body).unwrap_or_else(|e| panic!("not JSON ({e}): {body:?}"))
}

/// The status once the app finished adopting.
fn finished_status(socket: &Path) -> Value {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let body = status(socket);
        if body["adoption"]["finished"] == json!(true) {
            return body;
        }
        assert!(Instant::now() < deadline, "adoption did not finish: {body}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn adopted<'a>(status: &'a Value, id: &str) -> &'a Value {
    status["adoption"]["adopted"]
        .as_array()
        .unwrap()
        .iter()
        .find(|vm| vm["id"] == id)
        .unwrap_or_else(|| panic!("{id} is not adopted in {status}"))
}

fn not_adopted<'a>(status: &'a Value, id: &str) -> &'a Value {
    status["adoption"]["not_adopted"]
        .as_array()
        .unwrap()
        .iter()
        .find(|vm| vm["id"] == id)
        .unwrap_or_else(|| panic!("{id} is not in the list of unadopted VMs in {status}"))
}

#[test]
fn a_vm_whose_process_matches_is_adopted_and_both_probes_answer() {
    let _serial = serial();
    let sandbox = Sandbox::new();
    let vm = Orphan::start();
    let (agent, vnc) = (Fake::start(Behaviour::Agent), Fake::start(Behaviour::Vnc));
    sandbox.add_vm(
        "testanyware-ad01",
        json!({ "tool": "qemu", "pid": vm.0, "identity": vm.identity() }),
        Some(&agent),
        &vnc,
    );
    let app = start_app(&sandbox, 5000);

    let body = finished_status(&app.socket);

    let entry = adopted(&body, "testanyware-ad01");
    assert_eq!(entry["legacy"], false);
    assert_eq!(entry["agent"], "answering");
    assert_eq!(entry["vnc"], "answering");
    assert!(body["adoption"]["not_adopted"].as_array().unwrap().is_empty());
    assert!(vm.alive(), "adoption signalled the VM");
}

#[test]
fn a_meta_with_a_reused_pid_is_reported_as_not_adopted_and_signals_nothing() {
    let _serial = serial();
    let sandbox = Sandbox::new();
    let stranger = Orphan::start();
    let vnc = Fake::start(Behaviour::Vnc);
    // The meta recorded another start time than the one the process at the pid has.
    let mut identity = stranger.identity();
    identity["start_time"] = json!(identity["start_time"].as_u64().unwrap() + 1);
    sandbox.add_vm(
        "testanyware-ad02",
        json!({ "tool": "qemu", "pid": stranger.0, "identity": identity }),
        None,
        &vnc,
    );
    let app = start_app(&sandbox, 5000);

    let body = finished_status(&app.socket);

    let entry = not_adopted(&body, "testanyware-ad02");
    assert_eq!(entry["reason"], "start_time_differs");
    assert!(entry["detail"].as_str().unwrap().contains("signals nothing"));
    assert!(body["adoption"]["adopted"].as_array().unwrap().is_empty());
    assert!(stranger.alive(), "adoption signalled the process at a reused pid");
    assert!(sandbox.meta_path("testanyware-ad02").exists(), "adoption removed a record");
    assert_eq!(vnc.accepted(), 0, "the app probed a VM it did not adopt");
}

#[test]
fn a_vm_whose_process_is_gone_is_reported_as_stopped_and_keeps_its_records() {
    let _serial = serial();
    let sandbox = Sandbox::new();
    let vnc = Fake::start(Behaviour::Vnc);
    let gone = {
        let vm = Orphan::start();
        let identity = vm.identity();
        let pid = vm.0;
        drop(vm);
        (pid, identity)
    };
    // The kill ends the process, and the kernel takes a moment to forget it.
    let deadline = Instant::now() + Duration::from_secs(5);
    while Orphan(gone.0).alive() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    sandbox.add_vm(
        "testanyware-ad03",
        json!({ "tool": "qemu", "pid": gone.0, "identity": gone.1 }),
        None,
        &vnc,
    );
    let app = start_app(&sandbox, 5000);

    let body = finished_status(&app.socket);

    assert_eq!(not_adopted(&body, "testanyware-ad03")["reason"], "process_gone");
    assert!(sandbox.meta_path("testanyware-ad03").exists());
}

#[test]
fn an_older_meta_is_adopted_for_streams_and_marked_legacy() {
    let _serial = serial();
    let sandbox = Sandbox::new();
    let vm = Orphan::start();
    let (agent, vnc) = (Fake::start(Behaviour::Agent), Fake::start(Behaviour::Vnc));
    sandbox.add_vm(
        "testanyware-ad04",
        json!({ "tool": "qemu", "pid": vm.0 }),
        Some(&agent),
        &vnc,
    );
    let app = start_app(&sandbox, 5000);

    let body = finished_status(&app.socket);

    let entry = adopted(&body, "testanyware-ad04");
    assert_eq!(entry["legacy"], true);
    assert_eq!(entry["agent"], "answering");
    assert!(vm.alive(), "adoption signalled the VM");
}

#[test]
fn a_tart_guest_without_a_keeper_gets_no_vnc_connection() {
    let _serial = serial();
    let sandbox = Sandbox::new();
    let vm = Orphan::start();
    let (agent, vnc) = (Fake::start(Behaviour::Agent), Fake::start(Behaviour::Vnc));
    sandbox.add_vm(
        "testanyware-ad05",
        json!({ "tool": "tart", "pid": vm.0, "identity": vm.identity() }),
        Some(&agent),
        &vnc,
    );
    let app = start_app(&sandbox, 5000);

    let body = finished_status(&app.socket);

    let entry = adopted(&body, "testanyware-ad05");
    assert_eq!(entry["agent"], "answering");
    assert_eq!(entry["vnc"], "not_probed");
    assert_eq!(vnc.accepted(), 0, "the probe connected to a VNC port that no keeper holds");
}

#[test]
fn a_probe_that_gets_no_answer_in_its_deadline_says_not_answering_and_not_dead() {
    let _serial = serial();
    let sandbox = Sandbox::new();
    let vm = Orphan::start();
    let (agent, vnc) = (Fake::start(Behaviour::Silent), Fake::start(Behaviour::Silent));
    sandbox.add_vm(
        "testanyware-ad06",
        json!({ "tool": "qemu", "pid": vm.0, "identity": vm.identity() }),
        Some(&agent),
        &vnc,
    );
    let app = start_app(&sandbox, 300);

    let body = finished_status(&app.socket);

    // The VM is still adopted: a silent guest is not a dead guest.
    let entry = adopted(&body, "testanyware-ad06");
    assert_eq!(entry["agent"], "not_answering");
    assert_eq!(entry["vnc"], "not_answering");
    assert!(body["adoption"]["not_adopted"].as_array().unwrap().is_empty());
    assert!(vm.alive());
}

#[test]
fn the_app_serves_its_socket_while_a_probe_is_still_waiting() {
    let _serial = serial();
    let sandbox = Sandbox::new();
    let vm = Orphan::start();
    let (agent, vnc) = (Fake::start(Behaviour::Silent), Fake::start(Behaviour::Silent));
    sandbox.add_vm(
        "testanyware-ad07",
        json!({ "tool": "qemu", "pid": vm.0, "identity": vm.identity() }),
        Some(&agent),
        &vnc,
    );
    // The probes wait 60 s, so the first status shows them pending.
    let app = start_app(&sandbox, 60_000);

    let started = Instant::now();
    let deadline = started + Duration::from_secs(10);
    let body = loop {
        let body = status(&app.socket);
        if body["adoption"]["adopted"].as_array().is_some_and(|a| !a.is_empty()) {
            break body;
        }
        assert!(Instant::now() < deadline, "the scan published nothing: {body}");
        std::thread::sleep(Duration::from_millis(20));
    };

    assert!(started.elapsed() < Duration::from_secs(10));
    assert_eq!(body["adoption"]["finished"], false);
    let entry = adopted(&body, "testanyware-ad07");
    assert_eq!(entry["agent"], "pending");
    assert_eq!(entry["vnc"], "pending");
}
