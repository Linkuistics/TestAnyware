//! The socket, its refusals, `GET /v1/status` and the version handshake.
//!
//! Each test runs the real `testanyware-app` executable in test mode and talks raw HTTP on its
//! socket. Only the foreign-peer test runs the library in-process, because no test can connect
//! as another user.

#![cfg(unix)]

use std::io::{Read, Write};
use std::os::unix::fs::{FileTypeExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use serde_json::Value;
use tempfile::TempDir;
use testanyware_app_protocol::{ClientHello, Directories, ProtocolRange};

/// Every test holds this lock. macOS has no atomic close-on-exec for a new socket or pipe, so a
/// thread that spawns the app can hand its child a listener or a pipe that a sibling test has
/// just opened. A "stale" socket then keeps a live listener, and `output()` never sees EOF.
static SERIAL: Mutex<()> = Mutex::new(());

fn serial() -> MutexGuard<'static, ()> {
    SERIAL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

const APP: &str = env!("CARGO_BIN_EXE_testanyware-app");
const BUILD: &str = env!("CARGO_PKG_VERSION");

/// A response as the raw client reads it.
struct Raw {
    status: u16,
    headers: Vec<(String, String)>,
    body: String,
}

impl Raw {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
    fn json(&self) -> Value {
        serde_json::from_str(&self.body)
            .unwrap_or_else(|e| panic!("body is not JSON ({e}): {:?}", self.body))
    }
}

/// Send one request and read until the app closes the connection.
fn raw_request(socket: &Path, path: &str, hello: Option<&ClientHello>) -> Raw {
    let mut stream = UnixStream::connect(socket).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut request = format!("GET {path} HTTP/1.1\r\nHost: testanyware\r\nConnection: close\r\n");
    for (name, value) in hello.map(ClientHello::to_headers).unwrap_or_default() {
        request.push_str(&format!("{name}: {value}\r\n"));
    }
    request.push_str("\r\n");
    stream.write_all(request.as_bytes()).unwrap();
    let mut bytes = Vec::new();
    stream.read_to_end(&mut bytes).expect("read the response");
    parse(&String::from_utf8(bytes).expect("utf-8 response"))
}

fn parse(text: &str) -> Raw {
    let (head, body) = text
        .split_once("\r\n\r\n")
        .unwrap_or_else(|| panic!("no header block in {text:?}"));
    let mut lines = head.lines();
    let status = lines
        .next()
        .unwrap()
        .split_whitespace()
        .nth(1)
        .unwrap()
        .parse()
        .unwrap();
    let headers = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect();
    Raw {
        status,
        headers,
        body: body.to_string(),
    }
}

struct TestApp {
    child: Child,
    socket: PathBuf,
    dirs: Directories,
    _tmp: TempDir,
}

impl Drop for TestApp {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn app_command(socket: &Path, dirs: &Directories) -> Command {
    let mut cmd = Command::new(APP);
    cmd.args(["--test-mode", "--socket"])
        .arg(socket)
        .arg("--state-dir")
        .arg(&dirs.state_dir)
        .arg("--data-dir")
        .arg(&dirs.data_dir);
    cmd
}

fn dirs_in(tmp: &Path) -> Directories {
    Directories {
        state_dir: tmp.join("state"),
        data_dir: tmp.join("data"),
    }
}

fn wait_for_socket(socket: &Path, child: &mut Child) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if let Some(status) = child.try_wait().unwrap() {
            panic!("the app exited early with {status}");
        }
        if UnixStream::connect(socket).is_ok() {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!(
        "the app did not serve on {} in 10 seconds",
        socket.display()
    );
}

fn start() -> TestApp {
    let tmp = tempfile::tempdir().unwrap();
    let socket = tmp.path().join("run").join("testanyware.sock");
    let dirs = dirs_in(tmp.path());
    let mut child = app_command(&socket, &dirs)
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn the app");
    wait_for_socket(&socket, &mut child);
    TestApp {
        child,
        socket,
        dirs,
        _tmp: tmp,
    }
}

fn hello(range: (u32, u32), dirs: &Directories) -> ClientHello {
    ClientHello::new(ProtocolRange::new(range.0, range.1), "9.9.9", dirs.clone())
}

fn run_to_end(mut cmd: Command) -> Output {
    cmd.stdin(Stdio::null()).output().expect("run the app")
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

// ---- the socket directory and the socket ----

#[test]
fn the_app_creates_its_socket_directory_with_mode_0700() {
    let _serial = serial();
    let app = start();
    let dir = app.socket.parent().unwrap();
    let mode = std::fs::metadata(dir).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o700);
    assert!(std::fs::symlink_metadata(&app.socket)
        .unwrap()
        .file_type()
        .is_socket());
}

#[test]
fn the_app_refuses_a_directory_whose_mode_admits_others() {
    let _serial = serial();
    let tmp = tempfile::tempdir().unwrap();
    let run = tmp.path().join("run");
    std::fs::create_dir(&run).unwrap();
    std::fs::set_permissions(&run, std::fs::Permissions::from_mode(0o755)).unwrap();
    let socket = run.join("testanyware.sock");

    let out = run_to_end(app_command(&socket, &dirs_in(tmp.path())));

    assert_eq!(out.status.code(), Some(1), "stderr: {}", stderr(&out));
    assert!(stderr(&out).contains("0755"), "stderr: {}", stderr(&out));
    assert!(
        !socket.exists(),
        "the app bound a socket in an unsafe directory"
    );
    // It must not repair the mode either: the user chose it.
    assert_eq!(
        std::fs::metadata(&run).unwrap().permissions().mode() & 0o777,
        0o755
    );
}

#[test]
fn the_app_refuses_a_socket_path_of_more_than_103_bytes() {
    let _serial = serial();
    let tmp = tempfile::tempdir().unwrap();
    let socket = tmp.path().join("a".repeat(100)).join("testanyware.sock");
    assert!(socket.as_os_str().len() > 103);

    let out = run_to_end(app_command(&socket, &dirs_in(tmp.path())));

    assert_eq!(out.status.code(), Some(1), "stderr: {}", stderr(&out));
    assert!(
        stderr(&out).contains("103 bytes at most"),
        "stderr: {}",
        stderr(&out)
    );
    assert!(
        !socket.parent().unwrap().exists(),
        "the app created a directory for a path it refuses"
    );
}

#[test]
fn the_app_removes_a_socket_file_that_no_process_answers() {
    let _serial = serial();
    let tmp = tempfile::tempdir().unwrap();
    let run = tmp.path().join("run");
    std::fs::create_dir(&run).unwrap();
    std::fs::set_permissions(&run, std::fs::Permissions::from_mode(0o700)).unwrap();
    let socket = run.join("testanyware.sock");
    // std does not unlink on drop, so the file stays after the listener is gone.
    drop(UnixListener::bind(&socket).unwrap());
    assert!(std::fs::symlink_metadata(&socket)
        .unwrap()
        .file_type()
        .is_socket());

    let dirs = dirs_in(tmp.path());
    let mut child = app_command(&socket, &dirs)
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    wait_for_socket(&socket, &mut child);
    let status = raw_request(&socket, "/v1/status", None);
    let _ = child.kill();
    let _ = child.wait();

    assert_eq!(status.status, 200);
}

#[test]
fn the_app_does_not_remove_a_file_that_is_not_a_socket() {
    let _serial = serial();
    let tmp = tempfile::tempdir().unwrap();
    let run = tmp.path().join("run");
    std::fs::create_dir(&run).unwrap();
    std::fs::set_permissions(&run, std::fs::Permissions::from_mode(0o700)).unwrap();
    let socket = run.join("testanyware.sock");
    std::fs::write(&socket, "precious").unwrap();

    let out = run_to_end(app_command(&socket, &dirs_in(tmp.path())));

    assert_eq!(out.status.code(), Some(1), "stderr: {}", stderr(&out));
    assert_eq!(std::fs::read_to_string(&socket).unwrap(), "precious");
}

#[test]
fn a_second_process_exits_when_it_finds_the_first_on_the_socket() {
    let _serial = serial();
    let app = start();

    let out = run_to_end(app_command(&app.socket, &app.dirs));

    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    assert!(
        stderr(&out).contains("already answers"),
        "stderr: {}",
        stderr(&out)
    );
    // The first process still serves, on the same socket file.
    assert_eq!(raw_request(&app.socket, "/v1/status", None).status, 200);
}

// ---- the peer uid ----

/// No test can connect as another user, so this one gives the app a uid that is not ours.
/// The peer then looks foreign, and the app must close the connection without a byte.
// The lock must outlive the awaits: it keeps sibling tests from spawning during this one.
#[allow(clippy::await_holding_lock)]
#[tokio::test(flavor = "multi_thread")]
async fn the_app_closes_a_connection_whose_peer_uid_is_not_its_own() {
    let _serial = serial();
    use std::sync::Arc;
    use testanyware_app::gate::AppIdentity;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let tmp = tempfile::tempdir().unwrap();
    let socket = tmp.path().join("testanyware.sock");
    let listener = tokio::net::UnixListener::bind(&socket).unwrap();
    let real_uid = std::os::unix::fs::MetadataExt::uid(&std::fs::metadata(tmp.path()).unwrap());
    let dirs = dirs_in(tmp.path());

    let ctx = |own_uid| {
        Arc::new(testanyware_app::AppContext::new(
            AppIdentity {
                build: BUILD.into(),
                protocol_range: ProtocolRange::new(1, 1),
                directories: dirs.clone(),
            },
            own_uid,
            None,
        ))
    };

    // Positive control: with the right uid the same request is answered.
    let served = tokio::spawn(testanyware_app::serve(listener, ctx(real_uid)));
    assert!(exchange(&socket).await.starts_with("HTTP/1.1 200"));
    served.abort();
    let _ = served.await;
    std::fs::remove_file(&socket).unwrap();

    let listener = tokio::net::UnixListener::bind(&socket).unwrap();
    let served = tokio::spawn(testanyware_app::serve(listener, ctx(real_uid + 1)));
    assert_eq!(exchange(&socket).await, "", "a foreign peer got an answer");
    served.abort();

    async fn exchange(socket: &Path) -> String {
        let mut stream = tokio::net::UnixStream::connect(socket).await.unwrap();
        let _ = stream
            .write_all(b"GET /v1/status HTTP/1.1\r\nHost: t\r\nConnection: close\r\n\r\n")
            .await;
        let mut bytes = Vec::new();
        let _ = stream.read_to_end(&mut bytes).await;
        String::from_utf8_lossy(&bytes).into_owned()
    }
}

// ---- the status request ----

#[test]
fn status_answers_with_no_handshake_headers_at_all() {
    let _serial = serial();
    let app = start();

    let resp = raw_request(&app.socket, "/v1/status", None);

    assert_eq!(resp.status, 200, "{}", resp.body);
    let body = resp.json();
    assert_eq!(body["build"], BUILD);
    assert_eq!(
        body["protocol_range"],
        serde_json::json!({ "min": 1, "max": 1 })
    );
    assert_eq!(
        body["directories"]["state_dir"],
        app.dirs.state_dir.to_str().unwrap()
    );
    assert_eq!(
        body["directories"]["data_dir"],
        app.dirs.data_dir.to_str().unwrap()
    );
}

#[test]
fn status_answers_for_a_disjoint_range_and_for_other_directories() {
    let _serial = serial();
    let app = start();
    let other = Directories {
        state_dir: "/elsewhere/state".into(),
        data_dir: "/elsewhere/data".into(),
    };

    let resp = raw_request(&app.socket, "/v1/status", Some(&hello((7, 9), &other)));

    assert_eq!(resp.status, 200, "{}", resp.body);
    // The app reports its own values, whatever the client states.
    assert_eq!(
        resp.json()["directories"]["state_dir"],
        app.dirs.state_dir.to_str().unwrap()
    );
    assert_eq!(resp.json()["protocol_range"]["max"], 1);
}

#[test]
fn every_response_carries_the_apps_range_and_build() {
    let _serial = serial();
    let app = start();
    let other = Directories {
        state_dir: "/x".into(),
        data_dir: "/y".into(),
    };
    let responses = [
        raw_request(&app.socket, "/v1/status", None),
        raw_request(&app.socket, "/v1/vms", Some(&hello((5, 6), &app.dirs))),
        raw_request(&app.socket, "/v1/vms", Some(&hello((1, 1), &other))),
        raw_request(&app.socket, "/v1/vms", Some(&hello((1, 1), &app.dirs))),
    ];
    let statuses: Vec<u16> = responses.iter().map(|r| r.status).collect();
    assert_eq!(statuses, [200, 409, 409, 404]);
    for r in &responses {
        assert_eq!(r.header("x-testanyware-protocol"), Some("1-1"));
        assert_eq!(r.header("x-testanyware-build"), Some(BUILD));
    }
}

// ---- the handshake ----

#[test]
fn a_range_that_does_not_overlap_fails_and_names_both_sides() {
    let _serial = serial();
    let app = start();

    let resp = raw_request(&app.socket, "/v1/vms", Some(&hello((2, 3), &app.dirs)));

    assert_eq!(resp.status, 409);
    let err = resp.json();
    assert_eq!(err["code"], "APP_INCOMPATIBLE");
    assert_eq!(err["details"]["cause"], "protocol");
    assert_eq!(err["details"]["app"]["build"], BUILD);
    assert_eq!(
        err["details"]["app"]["protocol_range"],
        serde_json::json!({ "min": 1, "max": 1 })
    );
    assert_eq!(err["details"]["client"]["build"], "9.9.9");
    assert_eq!(
        err["details"]["client"]["protocol_range"],
        serde_json::json!({ "min": 2, "max": 3 })
    );
    let message = err["message"].as_str().unwrap();
    for needle in [BUILD, "9.9.9", "1-1", "2-3"] {
        assert!(
            message.contains(needle),
            "{needle} missing from {message:?}"
        );
    }
}

#[test]
fn a_request_with_no_range_fails_like_one_with_a_disjoint_range() {
    let _serial = serial();
    let app = start();

    let resp = raw_request(&app.socket, "/v1/vms", None);

    assert_eq!(resp.status, 409);
    assert_eq!(resp.json()["details"]["cause"], "protocol");
    assert_eq!(
        resp.json()["details"]["client"]["protocol_range"],
        Value::Null
    );
}

#[test]
fn other_directories_fail_and_name_both_pairs() {
    let _serial = serial();
    let app = start();
    let other = Directories {
        state_dir: "/elsewhere/state".into(),
        data_dir: app.dirs.data_dir.clone(),
    };

    let resp = raw_request(&app.socket, "/v1/vms", Some(&hello((1, 1), &other)));

    assert_eq!(resp.status, 409);
    let err = resp.json();
    assert_eq!(err["code"], "APP_INCOMPATIBLE");
    assert_eq!(err["details"]["cause"], "directories");
    assert_eq!(
        err["details"]["app"]["state_dir"],
        app.dirs.state_dir.to_str().unwrap()
    );
    assert_eq!(err["details"]["client"]["state_dir"], "/elsewhere/state");
    let message = err["message"].as_str().unwrap();
    assert!(
        message.contains("/elsewhere/state")
            && message.contains(app.dirs.state_dir.to_str().unwrap())
    );
    // Only the state directory differs, so only its variable is named.
    let remedy = err["remediation"].as_str().unwrap();
    assert!(
        remedy.contains("XDG_STATE_HOME") && !remedy.contains("XDG_DATA_HOME"),
        "{remedy}"
    );
}

#[test]
fn the_range_is_checked_before_the_directories() {
    let _serial = serial();
    let app = start();
    let other = Directories {
        state_dir: "/x".into(),
        data_dir: "/y".into(),
    };

    let resp = raw_request(&app.socket, "/v1/vms", Some(&hello((4, 4), &other)));

    assert_eq!(resp.json()["details"]["cause"], "protocol");
}

#[test]
fn an_overlapping_range_and_the_same_directories_pass_the_handshake() {
    let _serial = serial();
    let app = start();

    // The range 1-3 overlaps the app's 1-1. No route exists yet, so the answer is a 404.
    let resp = raw_request(&app.socket, "/v1/vms", Some(&hello((1, 3), &app.dirs)));

    assert_eq!(resp.status, 404);
    assert_eq!(resp.json()["code"], "USAGE_ERROR");
}

#[test]
fn the_handshake_guards_every_path_not_only_the_known_ones() {
    let _serial = serial();
    let app = start();

    let resp = raw_request(
        &app.socket,
        "/no/such/path",
        Some(&hello((2, 2), &app.dirs)),
    );

    assert_eq!(resp.status, 409);
}
