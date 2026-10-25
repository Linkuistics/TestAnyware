//! `config.rhai` in the app (ADR-0021, spec "Configuration").
//!
//! Each test runs the real `testanyware-app` executable in test mode with a `--config` file and
//! talks raw HTTP on its socket. The evaluator and the order of the sources have unit tests of
//! their own. These tests prove what the app does with the result: the directories it holds, the
//! status it reports, the refusal of a request with other directories, and the restart that reloads
//! the file.

#![cfg(unix)]

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use serde_json::Value;
use tempfile::TempDir;
use testanyware_app_protocol::{ClientHello, Directories, ProtocolRange};

/// Every test holds this lock: macOS gives a new socket no atomic close-on-exec, so a thread that
/// spawns the app can hand its child a socket that a sibling test has just opened.
static SERIAL: Mutex<()> = Mutex::new(());

fn serial() -> MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|p| p.into_inner())
}

const APP: &str = env!("CARGO_BIN_EXE_testanyware-app");

struct TestApp {
    child: Child,
    socket: PathBuf,
    config: PathBuf,
    /// The directories that the arguments name. These are the app's built-in ones in test mode.
    builtin: Directories,
    tmp: TempDir,
}

impl Drop for TestApp {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl TestApp {
    /// A directory pair that the file can name, away from the built-in pair.
    fn file_dirs(&self) -> Directories {
        Directories {
            state_dir: self.tmp.path().join("file-state"),
            data_dir: self.tmp.path().join("file-data"),
        }
    }

    /// Stop the app and start it again on the same socket, as a person quits and opens it.
    fn restart(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        self.child = spawn(&self.socket, &self.builtin, Some(&self.config), &[]);
        wait_for_socket(&self.socket, &mut self.child);
    }
}

fn spawn(
    socket: &Path,
    builtin: &Directories,
    config: Option<&Path>,
    tool_dirs: &[&Path],
) -> Child {
    let mut cmd = Command::new(APP);
    cmd.args(["--test-mode", "--socket"])
        .arg(socket)
        .arg("--state-dir")
        .arg(&builtin.state_dir)
        .arg("--data-dir")
        .arg(&builtin.data_dir);
    // A directory with no tools in it, so that the app never runs the developer's `tart`.
    let no_tools = builtin.data_dir.with_file_name("no-tools");
    cmd.arg("--tool-dir").arg(&no_tools);
    for dir in tool_dirs {
        cmd.arg("--tool-dir").arg(dir);
    }
    if let Some(config) = config {
        cmd.arg("--config").arg(config);
    }
    cmd.stderr(Stdio::null()).spawn().expect("spawn the app")
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

/// Start the app with a `config.rhai` that `source` writes. `source` receives the temporary
/// directory, so that a file can name directories inside it. `None` leaves the file out.
fn start_with(source: Option<&dyn Fn(&Path) -> String>) -> TestApp {
    start_with_tools(source, &[])
}

fn start_with_tools(source: Option<&dyn Fn(&Path) -> String>, tool_dirs: &[&Path]) -> TestApp {
    let tmp = tempfile::tempdir().unwrap();
    let socket = tmp.path().join("run").join("testanyware.sock");
    let config = tmp.path().join("config.rhai");
    if let Some(source) = source {
        std::fs::write(&config, source(tmp.path())).unwrap();
    }
    let builtin = Directories {
        state_dir: tmp.path().join("state"),
        data_dir: tmp.path().join("data"),
    };
    let mut child = spawn(&socket, &builtin, Some(&config), tool_dirs);
    wait_for_socket(&socket, &mut child);
    TestApp {
        child,
        socket,
        config,
        builtin,
        tmp,
    }
}

struct Raw {
    status: u16,
    body: String,
}

impl Raw {
    fn json(&self) -> Value {
        serde_json::from_str(&self.body)
            .unwrap_or_else(|e| panic!("body is not JSON ({e}): {:?}", self.body))
    }
}

fn request(socket: &Path, path: &str, hello: Option<&ClientHello>) -> Raw {
    let mut stream = UnixStream::connect(socket).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut text = format!("GET {path} HTTP/1.1\r\nHost: testanyware\r\nConnection: close\r\n");
    for (name, value) in hello.map(ClientHello::to_headers).unwrap_or_default() {
        text.push_str(&format!("{name}: {value}\r\n"));
    }
    text.push_str("\r\n");
    stream.write_all(text.as_bytes()).unwrap();
    let mut bytes = Vec::new();
    stream.read_to_end(&mut bytes).expect("read the response");
    let text = String::from_utf8(bytes).expect("utf-8 response");
    let (head, body) = text.split_once("\r\n\r\n").expect("a header block");
    let status = head
        .lines()
        .next()
        .unwrap()
        .split_whitespace()
        .nth(1)
        .unwrap()
        .parse()
        .unwrap();
    Raw {
        status,
        body: body.to_string(),
    }
}

fn hello(dirs: &Directories) -> ClientHello {
    ClientHello::new(ProtocolRange::new(1, 1), "9.9.9", dirs.clone())
}

fn status(app: &TestApp) -> Value {
    request(&app.socket, "/v1/status", None).json()
}

/// A file that names both directories inside the temporary directory.
fn names_directories(tmp: &Path) -> String {
    format!(
        r#"#{{ state_dir: "{0}/file-state", data_dir: "{0}/file-data" }}"#,
        tmp.display()
    )
}

// ---- a file that evaluates ----

#[test]
fn the_app_holds_the_directories_that_the_file_names() {
    let _serial = serial();
    let app = start_with(Some(&names_directories));

    let body = status(&app);

    let file = app.file_dirs();
    assert_eq!(
        body["directories"]["state_dir"],
        file.state_dir.to_str().unwrap()
    );
    assert_eq!(
        body["directories"]["data_dir"],
        file.data_dir.to_str().unwrap()
    );
    let configuration = &body["configuration"];
    assert_eq!(configuration["state"], "loaded");
    assert_eq!(configuration["file"], app.config.to_str().unwrap());
    assert_eq!(configuration["changed_since_evaluation"], false);
    assert_eq!(configuration["error"], Value::Null);
    assert!(configuration["evaluated_at"].as_u64().unwrap() > 1_700_000_000);
    // A request with the file's directories passes the handshake.
    let served = request(&app.socket, "/v1/vms?limit=100", Some(&hello(&file)));
    assert_eq!(served.status, 200, "{}", served.body);
}

#[test]
fn a_file_that_names_one_directory_leaves_the_other_to_the_built_in_default() {
    let _serial = serial();
    let app = start_with(Some(&|tmp| {
        format!(r#"#{{ data_dir: "{}/file-data" }}"#, tmp.display())
    }));

    let body = status(&app);

    assert_eq!(
        body["directories"]["state_dir"],
        app.builtin.state_dir.to_str().unwrap()
    );
    assert_eq!(
        body["directories"]["data_dir"],
        app.file_dirs().data_dir.to_str().unwrap()
    );
}

#[test]
fn a_request_with_the_built_in_directories_fails_when_the_file_names_others() {
    let _serial = serial();
    let app = start_with(Some(&names_directories));

    let resp = request(&app.socket, "/v1/vms?limit=100", Some(&hello(&app.builtin)));

    assert_eq!(resp.status, 409);
    let err = resp.json();
    assert_eq!(err["code"], "APP_INCOMPATIBLE");
    assert_eq!(err["details"]["cause"], "directories");
    assert_eq!(
        err["details"]["app"]["state_dir"],
        app.file_dirs().state_dir.to_str().unwrap()
    );
    // Both directories differ, and the remedy names each variable and each setting of the file.
    let remedy = err["remediation"].as_str().unwrap();
    for needle in [
        "config.rhai",
        "XDG_STATE_HOME",
        "XDG_DATA_HOME",
        "`state_dir`",
        "`data_dir`",
    ] {
        assert!(remedy.contains(needle), "{needle} missing from {remedy}");
    }
    // The file is the one the app evaluated, so the remedy asks for no restart.
    assert!(!remedy.contains("Quit and open"), "{remedy}");
}

#[test]
fn a_request_that_states_no_directories_fails_on_them_with_the_same_remedy() {
    let _serial = serial();
    let app = start_with(Some(&names_directories));
    // A range that overlaps and no directories.
    let silent = ClientHello {
        directories: None,
        ..hello(&app.builtin)
    };

    let resp = request(&app.socket, "/v1/vms?limit=100", Some(&silent));

    assert_eq!(resp.status, 409);
    assert_eq!(resp.json()["details"]["cause"], "directories");
    assert!(resp.json()["remediation"]
        .as_str()
        .unwrap()
        .contains("config.rhai"));
}

// ---- a file that fails ----

#[test]
fn a_file_that_fails_leaves_the_app_on_its_defaults_and_in_the_status() {
    let _serial = serial();
    let app = start_with(Some(&|_| "let x = 1;\nlet y = ;".to_string()));

    let body = status(&app);

    // The app runs: it answers, and it holds the built-in directories.
    assert_eq!(
        body["directories"]["state_dir"],
        app.builtin.state_dir.to_str().unwrap()
    );
    assert_eq!(
        body["directories"]["data_dir"],
        app.builtin.data_dir.to_str().unwrap()
    );
    let configuration = &body["configuration"];
    assert_eq!(configuration["state"], "failed");
    assert_eq!(configuration["file"], app.config.to_str().unwrap());
    assert!(configuration["evaluated_at"].as_u64().is_some());
    let error = &configuration["error"];
    assert_eq!(error["code"], "CONFIG_INVALID");
    assert_eq!(error["details"]["file"], app.config.to_str().unwrap());
    assert_eq!(error["details"]["line"], 2, "{error}");
    assert!(!error["details"]["message"].as_str().unwrap().is_empty());
    // A request with the built-in directories is served.
    let served = request(&app.socket, "/v1/vms?limit=100", Some(&hello(&app.builtin)));
    assert_eq!(served.status, 200, "{}", served.body);
}

#[test]
fn an_unknown_key_fails_the_file_without_a_line() {
    let _serial = serial();
    let app = start_with(Some(&|_| r#"#{ state_directory: "/x" }"#.to_string()));

    let configuration = status(&app)["configuration"].clone();

    assert_eq!(configuration["state"], "failed");
    assert_eq!(configuration["error"]["code"], "CONFIG_INVALID");
    assert_eq!(configuration["error"]["details"]["line"], Value::Null);
}

#[test]
fn a_macos_slots_outside_0_to_2_fails_the_file_and_leaves_the_app_on_its_defaults() {
    let _serial = serial();
    let app = start_with(Some(&|_| "#{ macos_slots: 3 }".to_string()));

    let configuration = status(&app)["configuration"].clone();

    assert_eq!(configuration["state"], "failed");
    assert_eq!(configuration["error"]["code"], "CONFIG_INVALID");
    assert_eq!(configuration["error"]["details"]["line"], Value::Null);
    assert!(
        configuration["error"]["message"]
            .as_str()
            .unwrap()
            .contains("`macos_slots` must be 0, 1 or 2, but it is 3"),
        "{configuration}"
    );
}

#[test]
fn a_macos_slots_of_0_1_or_2_loads() {
    let _serial = serial();
    for slots in 0..=2 {
        let app = start_with(Some(&|_| format!("#{{ macos_slots: {slots} }}")));
        assert_eq!(status(&app)["configuration"]["state"], "loaded", "{slots}");
    }
}

#[test]
fn a_directory_mismatch_names_the_file_that_fails_as_the_reason() {
    let _serial = serial();
    let app = start_with(Some(&|_| "let y = ;".to_string()));
    let other = Directories {
        state_dir: "/wanted/state".into(),
        data_dir: app.builtin.data_dir.clone(),
    };

    let resp = request(&app.socket, "/v1/vms?limit=100", Some(&hello(&other)));

    assert_eq!(resp.status, 409);
    let remedy = resp.json()["remediation"].as_str().unwrap().to_string();
    assert!(
        remedy.contains("XDG_STATE_HOME") && !remedy.contains("XDG_DATA_HOME"),
        "{remedy}"
    );
    assert!(
        remedy.contains(&format!("{} fails", app.config.display())),
        "{remedy}"
    );
    assert!(remedy.contains("quit and open TestAnyware"), "{remedy}");
}

// ---- no file ----

#[test]
fn a_file_that_does_not_exist_is_no_error() {
    let _serial = serial();
    let app = start_with(None);

    let configuration = status(&app)["configuration"].clone();

    assert_eq!(configuration["state"], "absent");
    assert_eq!(configuration["file"], app.config.to_str().unwrap());
    assert_eq!(configuration["error"], Value::Null);
    // A test-mode run writes nothing, so the absent file stays absent (spec "Configuration").
    assert!(!app.config.exists());
}

#[test]
fn a_test_mode_run_without_config_writes_no_file_at_the_default_place() {
    let _serial = serial();
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let socket = tmp.path().join("run").join("testanyware.sock");
    let mut cmd = Command::new(APP);
    cmd.args(["--test-mode", "--socket"])
        .arg(&socket)
        .arg("--state-dir")
        .arg(tmp.path().join("state"))
        .arg("--data-dir")
        .arg(tmp.path().join("data"))
        .env("HOME", &home)
        .env_remove("TESTANYWARE_CONFIG")
        .stderr(Stdio::null());
    let mut child = cmd.spawn().expect("spawn the app");
    wait_for_socket(&socket, &mut child);
    let _ = request(&socket, "/v1/status", None);
    let _ = child.kill();
    let _ = child.wait();

    // The home directory is as empty as it was: no `.config`, no file.
    assert_eq!(std::fs::read_dir(&home).unwrap().count(), 0);
}

#[test]
fn a_test_mode_run_without_config_reads_no_file_of_the_user() {
    let _serial = serial();
    let tmp = tempfile::tempdir().unwrap();
    // A file at the default place, which a developer's machine would have.
    let home = tmp.path().join("home");
    let default_file = home.join(".config/testanyware/config.rhai");
    std::fs::create_dir_all(default_file.parent().unwrap()).unwrap();
    std::fs::write(&default_file, r#"#{ state_dir: "/from/the/users/file" }"#).unwrap();
    let socket = tmp.path().join("run").join("testanyware.sock");
    let builtin = Directories {
        state_dir: tmp.path().join("state"),
        data_dir: tmp.path().join("data"),
    };
    let mut cmd = Command::new(APP);
    cmd.args(["--test-mode", "--socket"])
        .arg(&socket)
        .arg("--state-dir")
        .arg(&builtin.state_dir)
        .arg("--data-dir")
        .arg(&builtin.data_dir)
        .env("HOME", &home)
        .env_remove("TESTANYWARE_CONFIG")
        .stderr(Stdio::null());
    let mut child = cmd.spawn().expect("spawn the app");
    wait_for_socket(&socket, &mut child);

    let body = request(&socket, "/v1/status", None).json();
    let _ = child.kill();
    let _ = child.wait();

    assert_eq!(body["configuration"]["state"], "disabled");
    assert_eq!(body["configuration"]["file"], Value::Null);
    assert_eq!(
        body["directories"]["state_dir"],
        builtin.state_dir.to_str().unwrap()
    );
}

// ---- a restart reloads the file ----

#[test]
fn an_edit_is_reported_until_a_restart_reads_it() {
    let _serial = serial();
    let mut app = start_with(Some(&names_directories));
    let first = app.file_dirs();

    // The edit changes the length of the file, which the app compares.
    std::fs::write(
        &app.config,
        r#"#{ state_dir: "/second/state", data_dir: "/second/data" }"#,
    )
    .unwrap();

    // The running app keeps what it evaluated, and says that the file moved on.
    let body = status(&app);
    assert_eq!(
        body["directories"]["state_dir"],
        first.state_dir.to_str().unwrap()
    );
    assert_eq!(body["configuration"]["changed_since_evaluation"], true);
    // A request with the new directories fails, and the remedy asks for the restart.
    let second = Directories {
        state_dir: "/second/state".into(),
        data_dir: "/second/data".into(),
    };
    let refused = request(&app.socket, "/v1/vms?limit=100", Some(&hello(&second)));
    assert_eq!(refused.status, 409);
    let remedy = refused.json()["remediation"].as_str().unwrap().to_string();
    assert!(remedy.contains("evaluated an older"), "{remedy}");
    assert!(remedy.contains("Quit and open TestAnyware"), "{remedy}");

    // A restart reads the file again.
    app.restart();
    let body = status(&app);
    assert_eq!(body["directories"]["state_dir"], "/second/state");
    assert_eq!(body["configuration"]["changed_since_evaluation"], false);
    let served = request(&app.socket, "/v1/vms?limit=100", Some(&hello(&second)));
    assert_eq!(served.status, 200, "{}", served.body);
}

#[test]
fn a_restart_reads_a_file_that_a_person_fixed() {
    let _serial = serial();
    let mut app = start_with(Some(&|_| "let y = ;".to_string()));
    assert_eq!(status(&app)["configuration"]["state"], "failed");

    std::fs::write(&app.config, names_directories(app.tmp.path())).unwrap();
    assert_eq!(
        status(&app)["configuration"]["changed_since_evaluation"],
        true
    );
    app.restart();

    let configuration = status(&app)["configuration"].clone();
    assert_eq!(configuration["state"], "loaded");
    assert_eq!(configuration["error"], Value::Null);
}

// ---- the tool locations ----

/// A `tart` that logs its arguments and lists no VM.
#[cfg(target_os = "macos")]
fn fake_tart(dir: &Path, log: &Path) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir_all(dir).unwrap();
    let tart = dir.join("tart");
    std::fs::write(
        &tart,
        format!("#!/bin/sh\necho \"$@\" >> '{}'\necho '[]'\n", log.display()),
    )
    .unwrap();
    std::fs::set_permissions(&tart, std::fs::Permissions::from_mode(0o755)).unwrap();
    tart
}

// The listing runs `tart` on a macOS host only.
#[cfg(target_os = "macos")]
#[test]
fn the_app_runs_the_tart_that_the_file_names_and_not_the_one_in_its_search() {
    let _serial = serial();
    let scratch = tempfile::tempdir().unwrap();
    let in_search = fake_tart(
        &scratch.path().join("search"),
        &scratch.path().join("search.log"),
    );
    let named = fake_tart(
        &scratch.path().join("named"),
        &scratch.path().join("named.log"),
    );
    assert!(in_search.is_file());
    let source = format!(r#"#{{ tools: #{{ tart: "{}" }} }}"#, named.display());
    let app = start_with_tools(Some(&|_| source.clone()), &[&scratch.path().join("search")]);

    let served = request(&app.socket, "/v1/vms?limit=100", Some(&hello(&app.builtin)));

    assert_eq!(served.status, 200, "{}", served.body);
    let named_log = std::fs::read_to_string(scratch.path().join("named.log")).unwrap_or_default();
    assert!(
        named_log.contains("list"),
        "the named tart did not run: {named_log:?}"
    );
    assert!(
        !scratch.path().join("search.log").exists(),
        "the tart in the search ran although the file names another"
    );
}

#[cfg(target_os = "macos")]
#[test]
fn without_a_file_the_app_runs_the_tart_of_its_search() {
    let _serial = serial();
    let scratch = tempfile::tempdir().unwrap();
    fake_tart(
        &scratch.path().join("search"),
        &scratch.path().join("search.log"),
    );
    let app = start_with_tools(None, &[&scratch.path().join("search")]);

    let served = request(&app.socket, "/v1/vms?limit=100", Some(&hello(&app.builtin)));

    assert_eq!(served.status, 200, "{}", served.body);
    let log = std::fs::read_to_string(scratch.path().join("search.log")).unwrap_or_default();
    assert!(log.contains("list"), "{log:?}");
}

#[test]
fn a_tool_that_the_file_does_not_know_fails_the_file_and_not_the_app() {
    let _serial = serial();
    let app = start_with(Some(&|_| {
        r#"#{ tools: #{ git: "/usr/bin/git" } }"#.to_string()
    }));

    let configuration = status(&app)["configuration"].clone();

    assert_eq!(configuration["state"], "failed");
    assert_eq!(configuration["error"]["code"], "CONFIG_INVALID");
}
