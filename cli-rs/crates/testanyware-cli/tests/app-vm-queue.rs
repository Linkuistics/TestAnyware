//! The admission queue of the TestAnyware app (spec "admission-queue": "The order", "A refusal by
//! the framework", "A cancel", "The wait and its end" and "A quit, a crash and a restart").
//!
//! The real Host CLI binary runs against the real app executable in test mode, as in
//! `app-vm-start-routing`. Several CLI processes run at once, one for each start, as several
//! shells would run them. `tart` is the fake script with a capacity of 2: like the framework, it
//! refuses a third running VM and logs the refusal. So a test that finds no refusal in the log
//! shows that the queue held the third start back, and the framework never refused it. A test of
//! a guest that the app cannot see gives the fake a capacity below the count of the app.
//!
//! The tart backend exists on a macOS host only.

#![cfg(target_os = "macos")]

mod common;

use std::io::{BufRead, BufReader, Read};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::{serial, FakeAgent, OutputExt, TestApp, CLI};
use serde_json::{json, Value};
use wiremock::ResponseTemplate;

const FAKE_TART: &str = include_str!("../../testanyware-vm/tests/fixtures/fake-tart");
const MACOS_BASE: &str = "testanyware-golden-macos-27";
const LINUX_BASE: &str = "testanyware-golden-linux-24.04";

/// The fakes, then the app. The fakes start first and the app spawns once (see `common`).
struct Fixture {
    app: TestApp,
    _agent: FakeAgent,
    /// The port of the agent that the app probes, for a restart of the app.
    agent_port: u16,
}

/// Hold every connection that arrives, as a VNC server that never speaks does.
fn silent_vnc_server() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let mut held = Vec::new();
        for conn in listener.incoming().flatten() {
            held.push(conn);
        }
    });
    port
}

/// A port with nothing behind it.
fn closed_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

impl Fixture {
    /// An app whose starts probe a healthy fake agent, over a fake `tart` of capacity 2.
    fn healthy() -> Self {
        Self::healthy_after(|_| {})
    }

    /// As [`Self::healthy`], after `prepare` has written what the app reads when it starts.
    fn healthy_after(prepare: impl FnOnce(&TestApp)) -> Self {
        Self::healthy_with(None, prepare)
    }

    /// An app whose `config.rhai` is `config`, when there is one.
    fn healthy_with_config(config: &str) -> Self {
        Self::healthy_with(Some(config), |_| {})
    }

    fn healthy_with(config: Option<&str>, prepare: impl FnOnce(&TestApp)) -> Self {
        let agent = FakeAgent::start();
        agent.mount(
            "GET",
            "/health",
            ResponseTemplate::new(200)
                .set_body_json(json!({ "accessible": true, "platform": "macos" })),
        );
        let port = agent.addr().port();
        Self::with_agent_port(agent, port, config, prepare)
    }

    /// An app whose starts probe a port that refuses, so a start stays in its agent wait and
    /// keeps its slot.
    fn starved() -> Self {
        Self::with_agent_port(FakeAgent::start(), closed_port(), None, |_| {})
    }

    fn with_agent_port(
        agent: FakeAgent,
        agent_port: u16,
        config: Option<&str>,
        prepare: impl FnOnce(&TestApp),
    ) -> Self {
        let vnc_port = silent_vnc_server();
        let prepare = |app: &TestApp| {
            let tart = app.tool_dir().join("tart");
            std::fs::write(&tart, FAKE_TART).unwrap();
            std::fs::set_permissions(&tart, std::os::unix::fs::PermissionsExt::from_mode(0o755))
                .unwrap();
            let state = app.tool_dir().join("state");
            std::fs::create_dir_all(&state).unwrap();
            std::fs::write(state.join("vnc-port"), vnc_port.to_string()).unwrap();
            std::fs::write(state.join("capacity"), "2").unwrap();
            prepare(app);
        };
        let args = ["--test-agent-port".to_string(), agent_port.to_string()];
        let app = match config {
            Some(config) => TestApp::start_after_with_config_text_and_args(
                prepare,
                |app| config.replace("$STATE", &app.state_dir().display().to_string())
                    .replace("$DATA", &app.data_dir().display().to_string()),
                &args,
            ),
            None => TestApp::start_after_with_args(prepare, &args),
        };
        Self { app, _agent: agent, agent_port }
    }

    fn fake_state(&self) -> PathBuf {
        self.app.tool_dir().join("state")
    }

    /// The app crashes: SIGKILL, with no cleanup. The fake `tart run` of a start outlives it, as
    /// a real VM does.
    fn crash(&mut self) {
        self.app.crash();
    }

    /// The app starts again on the same directories, after [`crash`](Self::crash).
    fn restart(&mut self) {
        let mut args = Vec::new();
        if self.app.config_file().exists() {
            args.push("--config".to_string());
            args.push(self.app.config_file().display().to_string());
        }
        args.push("--test-agent-port".to_string());
        args.push(self.agent_port.to_string());
        let down = std::mem::replace(&mut self.app, TestApp::not_running());
        self.app = down.restart(&args);
    }

    /// The path of the start record of `id`.
    fn start_record(&self, id: &str) -> PathBuf {
        self.app.state_dir().join("vms").join(format!("{id}.start-record"))
    }

    /// `GET /v1/status` as the app serves it. The app answers only after it has decided for the
    /// start records that it found.
    fn status(&self) -> Value {
        let (code, body) = raw_get(
            &self.app,
            "/v1/status",
            testanyware_app_protocol::version::SUPPORTED_RANGE,
        );
        assert_eq!(code, 200, "{body}");
        body
    }

    fn cloned(&self, id: &str) -> bool {
        self.fake_state().join(format!("{id}.cloned")).exists()
    }

    fn tart_run_pid(&self, id: &str) -> Option<i32> {
        std::fs::read_to_string(self.fake_state().join(format!("{id}.running")))
            .ok()?
            .trim()
            .parse()
            .ok()
    }

    /// The `tart run` of `id` has started and is alive.
    fn runs(&self, id: &str) -> bool {
        self.tart_run_pid(id)
            .is_some_and(testanyware_vm::process::process_alive)
    }

    /// The `tart run` of every start that the fake refused, as the framework would.
    fn refusals(&self) -> Vec<String> {
        std::fs::read_to_string(self.fake_state().join("refused.log"))
            .unwrap_or_default()
            .lines()
            .map(str::to_string)
            .collect()
    }

    /// Start a macOS VM through a CLI process that runs in the background.
    fn spawn_start(&self, id: &str, extra: &[&str]) -> Client {
        self.spawn(
            &[
                "vm",
                "start",
                "--platform",
                "macos",
                "--base",
                MACOS_BASE,
                "--id",
                id,
            ],
            extra,
        )
    }

    /// As [`Self::spawn_start`], from the working directory `dir`, as a shell in a worktree runs it.
    fn spawn_start_in(&self, dir: &std::path::Path, id: &str, extra: &[&str]) -> Client {
        let args = [
            "vm",
            "start",
            "--platform",
            "macos",
            "--base",
            MACOS_BASE,
            "--id",
            id,
        ];
        self.spawn_in(Some(dir), &args, extra)
    }

    fn spawn(&self, args: &[&str], extra: &[&str]) -> Client {
        self.spawn_in(None, args, extra)
    }

    fn spawn_in(&self, dir: Option<&std::path::Path>, args: &[&str], extra: &[&str]) -> Client {
        let mut command = Command::new(CLI);
        if let Some(dir) = dir {
            command.current_dir(dir);
        }
        command
            .args(args)
            .args(extra)
            .arg("--json")
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("HOME", self.app.root())
            .env("TMPDIR", self.app.tmp_dir())
            .env("XDG_STATE_HOME", self.app.xdg_state_home())
            .env("XDG_DATA_HOME", self.app.xdg_data_home())
            .env("TESTANYWARE_APP_SOCKET", self.app.socket())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        Client::spawn(command)
    }

    /// A guest that a person runs with `tart run` from a shell: a VM in the fake's state and a
    /// process that holds it. The app did not start it, and it has no meta.
    fn run_outside(&self, name: &str, os: &str) -> Child {
        let state = self.fake_state();
        let guest = Command::new("sleep")
            .arg("600")
            .spawn()
            .expect("run a fake guest");
        std::fs::write(state.join(format!("{name}.cloned")), "").unwrap();
        std::fs::write(state.join(format!("{name}.os")), os).unwrap();
        std::fs::write(
            state.join(format!("{name}.running")),
            guest.id().to_string(),
        )
        .unwrap();
        guest
    }

    /// The guest of [`run_outside`](Self::run_outside) ends, as when a person quits it.
    fn end_outside(&self, name: &str, mut guest: Child) {
        guest.kill().unwrap();
        guest.wait().unwrap();
        let state = self.fake_state();
        for extension in ["running", "cloned", "os"] {
            let _ = std::fs::remove_file(state.join(format!("{name}.{extension}")));
        }
    }

    /// Write a control file of the fake `tart` (see the header of the script).
    fn set_fake(&self, name: &str, value: &str) {
        std::fs::write(self.fake_state().join(name), value).unwrap();
    }

    fn clear_fake(&self, name: &str) {
        let _ = std::fs::remove_file(self.fake_state().join(name));
    }

    fn stop(&self, id: &str) {
        let out = self.app.cli(&["vm", "stop", id, "--json"]);
        assert_eq!(out.code(), 0, "stop {id}: {} {}", out.out(), out.err());
    }

    /// Start `id` and wait until the VM runs. The slot is then held by its guest.
    fn start_and_wait_until_running(&self, id: &str) {
        let out = self.app.cli(&[
            "vm",
            "start",
            "--platform",
            "macos",
            "--base",
            MACOS_BASE,
            "--id",
            id,
            "--wait",
            "--json",
        ]);
        assert_eq!(out.code(), 0, "start {id}: {} {}", out.out(), out.err());
    }
}

impl Drop for Fixture {
    /// The fake `tart run` is a `sleep` that outlives the app, as a real VM does. End each one.
    fn drop(&mut self) {
        let Ok(entries) = std::fs::read_dir(self.fake_state()) else {
            return;
        };
        for entry in entries.flatten() {
            if entry.path().extension().is_some_and(|ext| ext == "running") {
                if let Some(pid) = std::fs::read_to_string(entry.path())
                    .ok()
                    .and_then(|text| text.trim().parse::<i32>().ok())
                {
                    let _ = nix::sys::signal::kill(
                        nix::unistd::Pid::from_raw(pid),
                        nix::sys::signal::Signal::SIGKILL,
                    );
                }
            }
        }
    }
}

/// A CLI process that runs in the background, with its two streams collected as they arrive.
struct Client {
    child: Child,
    out: Arc<Mutex<String>>,
    err: Arc<Mutex<String>>,
}

impl Client {
    fn spawn(mut command: Command) -> Self {
        let mut child = command.spawn().expect("run the CLI");
        let out = Arc::new(Mutex::new(String::new()));
        let err = Arc::new(Mutex::new(String::new()));
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        let sink = Arc::clone(&out);
        std::thread::spawn(move || {
            let mut text = String::new();
            let _ = BufReader::new(stdout).read_to_string(&mut text);
            sink.lock().unwrap().push_str(&text);
        });
        let sink = Arc::clone(&err);
        std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                let mut sink = sink.lock().unwrap();
                sink.push_str(&line);
                sink.push('\n');
            }
        });
        Self { child, out, err }
    }

    fn stderr(&self) -> String {
        self.err.lock().unwrap().clone()
    }

    fn finished(&mut self) -> bool {
        self.child.try_wait().unwrap().is_some()
    }

    /// Wait until stderr holds `text`.
    fn until_stderr_has(&self, text: &str) {
        until(&format!("stderr to hold {text:?}"), || {
            self.stderr().contains(text)
        });
    }

    /// Wait for the process to end. Returns its exit code, its envelope and its stderr.
    fn finish(mut self) -> (i32, Value, String) {
        let deadline = Instant::now() + Duration::from_secs(60);
        let status = loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                break status;
            }
            assert!(
                Instant::now() < deadline,
                "the CLI did not end: {}",
                self.stderr()
            );
            std::thread::sleep(Duration::from_millis(50));
        };
        // The reader threads end with the pipes. Give them a moment to hand over the text.
        std::thread::sleep(Duration::from_millis(100));
        let out = self.out.lock().unwrap().clone();
        let envelope = serde_json::from_str(&out).unwrap_or(Value::Null);
        (status.code().unwrap_or(-1), envelope, self.stderr())
    }

    /// End the process as Ctrl-C or a timeout of a harness does.
    fn kill(mut self) {
        self.child.kill().unwrap();
        self.child.wait().unwrap();
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn until(what: &str, mut ready: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while !ready() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn the_third_of_three_starts_waits_and_gets_its_slot_in_order_after_a_stop() {
    let _serial = serial();
    let fixture = Fixture::healthy();
    let a = fixture.spawn_start("q-a", &["--wait"]);
    let b = fixture.spawn_start("q-b", &["--wait"]);
    let (a_code, a_json, _) = a.finish();
    let (b_code, _, _) = b.finish();
    assert_eq!((a_code, b_code), (0, 0), "{a_json}");
    assert!(fixture.runs("q-a") && fixture.runs("q-b"));

    // The third start waits. Its clone is ready, and no `tart run` has started for it.
    let mut c = fixture.spawn_start("q-c", &["--wait"]);
    c.until_stderr_has("place 1");
    let line = c.stderr();
    assert!(line.contains("request r-"), "{line}");
    assert!(line.contains("1 waiter"), "{line}");
    assert!(
        line.contains("q-a") && line.contains("q-b"),
        "the line names the holders: {line}"
    );
    assert!(
        fixture.cloned("q-c"),
        "the start joined when its clone was ready"
    );
    std::thread::sleep(Duration::from_millis(500));
    assert!(!c.finished(), "the third start waits");
    assert!(
        fixture.tart_run_pid("q-c").is_none(),
        "no tart run for a waiter"
    );

    // A stop frees a slot, and the app admits the waiter.
    fixture.stop("q-a");
    let (c_code, c_json, _) = c.finish();
    assert_eq!(c_code, 0, "{c_json}");
    assert_eq!(c_json["id"], "q-c");
    assert!(fixture.runs("q-c"));
    assert_eq!(
        fixture.refusals(),
        Vec::<String>::new(),
        "the framework refused no start: the queue held the third back"
    );
}

#[test]
fn the_app_admits_the_earlier_waiter_first() {
    let _serial = serial();
    let fixture = Fixture::healthy();
    fixture.start_and_wait_until_running("o-a");
    fixture.start_and_wait_until_running("o-b");

    let c = fixture.spawn_start("o-c", &["--wait"]);
    c.until_stderr_has("place 1");
    let mut d = fixture.spawn_start("o-d", &["--wait"]);
    d.until_stderr_has("place 2");

    fixture.stop("o-b");
    until("the earlier waiter to run", || fixture.runs("o-c"));
    std::thread::sleep(Duration::from_millis(500));
    assert!(
        !fixture.runs("o-d"),
        "the later waiter must not pass the earlier one"
    );
    assert!(!d.finished());

    fixture.stop("o-a");
    let (c_code, _, _) = c.finish();
    let (d_code, d_json, d_err) = d.finish();
    assert_eq!(c_code, 0);
    assert_eq!(d_code, 0, "{d_json} {d_err}");
    assert!(
        d_err.contains("place 1"),
        "the place of the later waiter moved up: {d_err}"
    );
    assert_eq!(fixture.refusals(), Vec::<String>::new());
}

#[test]
fn two_free_slots_admit_two_starts_at_once() {
    let _serial = serial();
    let fixture = Fixture::healthy();
    let a = fixture.spawn_start("t-a", &["--wait"]);
    let b = fixture.spawn_start("t-b", &["--wait"]);
    let (a_code, ..) = a.finish();
    let (b_code, ..) = b.finish();
    assert_eq!((a_code, b_code), (0, 0));
    assert!(fixture.runs("t-a") && fixture.runs("t-b"));
}

#[test]
fn with_macos_slots_1_a_second_start_waits_while_a_first_guest_runs_and_the_framework_never_refuses(
) {
    let _serial = serial();
    let fixture = Fixture::healthy_with_config("#{ macos_slots: 1 }");
    fixture.start_and_wait_until_running("s1-a");

    let mut second = fixture.spawn_start("s1-b", &["--wait"]);
    second.until_stderr_has("place 1");
    std::thread::sleep(Duration::from_secs(4));
    assert!(
        !second.finished(),
        "the framework has a free slot, but the app holds one of the two"
    );
    assert!(!fixture.runs("s1-b"), "no tart run for the waiting start");

    fixture.stop("s1-a");
    let (code, envelope, err) = second.finish();
    assert_eq!(code, 0, "{envelope} {err}");
    assert!(fixture.runs("s1-b"));
    assert_eq!(fixture.refusals(), Vec::<String>::new());
}

#[test]
fn with_macos_slots_0_every_macos_start_fails_at_once_with_limit_0() {
    let _serial = serial();
    let fixture = Fixture::healthy_with_config("#{ macos_slots: 0 }");
    for (id, extra) in [
        ("s0-a", &["--wait"][..]),
        ("s0-b", &["--wait=600"]),
        ("s0-c", &[]),
    ] {
        let began = Instant::now();
        let (code, envelope, err) = fixture.spawn_start(id, extra).finish();
        assert!(
            began.elapsed() < Duration::from_secs(30),
            "{id}: the start waited"
        );
        assert_eq!(code, 6, "{id}: {envelope} {err}");
        assert_eq!(envelope["code"], "VM_LIMIT_REACHED");
        let details = &envelope["details"];
        assert_eq!(details["limit"], 0, "{id}: {details}");
        assert_eq!(details["refused_by"], "app");
        assert_eq!(details["waited_secs"], 0);
        assert!(
            !fixture.cloned(id),
            "{id}: the failed start leaves no clone"
        );
        assert!(!fixture.runs(id));
    }
    assert_eq!(fixture.refusals(), Vec::<String>::new());
}

#[test]
fn a_wait_that_runs_out_fails_with_the_request_the_place_and_the_time_that_it_waited() {
    let _serial = serial();
    let fixture = Fixture::healthy();
    fixture.start_and_wait_until_running("w-a");
    fixture.start_and_wait_until_running("w-b");

    let began = Instant::now();
    let client = fixture.spawn_start("w-c", &["--wait=2"]);
    let (code, envelope, err) = client.finish();
    assert_eq!(code, 6, "{envelope} {err}");
    assert!(began.elapsed() >= Duration::from_secs(2));
    assert_eq!(envelope["code"], "VM_LIMIT_REACHED");
    let details = &envelope["details"];
    assert_eq!(details["refused_by"], "app");
    assert_eq!(details["place"], 1);
    assert_eq!(details["limit"], 2);
    assert!(details["waited_secs"].as_u64().unwrap() >= 2, "{details}");
    assert!(
        details["request_id"].as_str().unwrap().starts_with("r-"),
        "{details}"
    );
    assert_eq!(details["tart_error"], "", "no tart run took part");
    assert_eq!(details["holders"], json!(["w-a", "w-b"]));
    assert!(err.contains("place 1"), "{err}");
    assert!(!fixture.cloned("w-c"), "the failed start leaves no clone");
    assert_eq!(
        fixture.refusals(),
        Vec::<String>::new(),
        "no tart run for the refused start"
    );
}

#[test]
fn a_start_without_a_wait_never_passes_a_waiter_and_runs_no_tart_run() {
    let _serial = serial();
    let fixture = Fixture::healthy();
    fixture.start_and_wait_until_running("n-a");
    fixture.start_and_wait_until_running("n-b");
    let mut waiter = fixture.spawn_start("n-c", &["--wait"]);
    waiter.until_stderr_has("place 1");

    let began = Instant::now();
    let out = fixture.app.cli(&[
        "vm",
        "start",
        "--platform",
        "macos",
        "--base",
        MACOS_BASE,
        "--id",
        "n-d",
        "--json",
    ]);
    assert!(
        began.elapsed() < Duration::from_secs(10),
        "it fails at once"
    );
    assert_eq!(out.code(), 6, "{} {}", out.out(), out.err());
    let details = &out.json()["details"];
    assert_eq!(details["refused_by"], "app");
    assert_eq!(details["place"], 2, "the place that the start would take");
    assert_eq!(details["waited_secs"], 0);
    assert!(fixture.tart_run_pid("n-d").is_none());

    // The wait of 0 s is the same start.
    let out = fixture.app.cli(&[
        "vm",
        "start",
        "--platform",
        "macos",
        "--base",
        MACOS_BASE,
        "--id",
        "n-e",
        "--wait=0",
        "--json",
    ]);
    assert_eq!(out.code(), 6);
    assert_eq!(out.json()["details"]["place"], 2);

    assert!(!waiter.finished(), "the waiter still waits");
    waiter.kill();
}

#[test]
fn a_start_without_a_wait_takes_a_free_slot_when_no_waiter_stands_ahead() {
    let _serial = serial();
    let fixture = Fixture::for_golden("");
    fixture.start_and_wait_until_running("f-a");
    for (id, wait) in [("f-b", None), ("f-c", Some("--wait=0"))] {
        let mut args = vec![
            "vm",
            "start",
            "--platform",
            "macos",
            "--base",
            MACOS_BASE,
            "--id",
            id,
            "--json",
        ];
        args.extend(wait);
        let out = fixture.app.cli(&args);
        if id == "f-b" {
            assert_eq!(out.code(), 0, "{} {}", out.out(), out.err());
        } else {
            // Both slots are held by now.
            assert_eq!(out.code(), 6, "{} {}", out.out(), out.err());
            assert_eq!(out.json()["details"]["refused_by"], "app");
        }
    }
}

#[test]
fn a_tart_run_outside_the_app_holds_a_slot_and_the_start_waits_without_a_refused_try() {
    let _serial = serial();
    let fixture = Fixture::healthy();
    fixture.start_and_wait_until_running("x-a");
    let outside = fixture.run_outside("x-outside", "darwin");

    let mut waiting = fixture.spawn_start("x-b", &["--wait"]);
    waiting.until_stderr_has("place 1");
    // The first line comes at the join, from the last answer. The next observation sees the guest,
    // and the holders change, so a new line names the guest that the app did not start.
    waiting.until_stderr_has("x-outside");
    // The tick of 3 s passes twice. The count keeps the guest, and the app tries nothing.
    std::thread::sleep(Duration::from_secs(7));
    assert!(
        !waiting.finished(),
        "the external guest holds the other slot"
    );
    assert!(!fixture.runs("x-b"), "no tart run for the waiting start");
    assert_eq!(fixture.refusals(), Vec::<String>::new());

    fixture.end_outside("x-outside", outside);
    let (code, envelope, err) = waiting.finish();
    assert_eq!(code, 0, "{envelope} {err}");
    assert!(fixture.runs("x-b"));
    assert_eq!(
        fixture.refusals(),
        Vec::<String>::new(),
        "the framework refused no try"
    );
}

#[test]
fn the_app_never_stops_a_guest_that_it_did_not_start() {
    let _serial = serial();
    let fixture = Fixture::healthy();
    let outside = fixture.run_outside("y-outside", "darwin");
    fixture.start_and_wait_until_running("y-a");
    let (code, envelope, err) = fixture.spawn_start("y-b", &["--wait=1"]).finish();
    assert_eq!(code, 6, "{envelope} {err}");
    assert_eq!(envelope["details"]["refused_by"], "app");
    assert!(
        envelope["details"]["holders"]
            .to_string()
            .contains("y-outside"),
        "{envelope}"
    );
    assert!(
        fixture.fake_state().join("y-outside.running").exists(),
        "the app stopped no guest of a person"
    );
    fixture.end_outside("y-outside", outside);
}

#[test]
fn a_start_without_a_wait_fails_at_once_beside_an_own_guest_and_an_external_guest() {
    let _serial = serial();
    let fixture = Fixture::healthy();
    fixture.start_and_wait_until_running("z-a");
    let outside = fixture.run_outside("z-outside", "darwin");
    let out = fixture.app.cli(&[
        "vm",
        "start",
        "--platform",
        "macos",
        "--base",
        MACOS_BASE,
        "--id",
        "z-b",
        "--json",
    ]);
    assert_eq!(out.code(), 6, "{} {}", out.out(), out.err());
    let details = &out.json()["details"];
    assert_eq!(details["refused_by"], "app");
    let holders = details["holders"].to_string();
    assert!(
        holders.contains("z-outside") && holders.contains("z-a"),
        "{holders}"
    );
    assert!(fixture.tart_run_pid("z-b").is_none(), "no tart run");
    assert_eq!(fixture.refusals(), Vec::<String>::new());
    fixture.end_outside("z-outside", outside);
}

#[test]
fn a_linux_guest_beside_the_starts_takes_no_slot_of_the_count() {
    let _serial = serial();
    let fixture = Fixture::healthy();
    // The framework allows three running VMs here, so only the count of the app can hold a start
    // back. A Linux guest is not a macOS guest, so it holds no slot.
    std::fs::write(fixture.fake_state().join("capacity"), "3").unwrap();
    let linux = fixture.run_outside("w-linux", "linux");
    let a = fixture.spawn_start("w-a", &["--wait"]);
    let b = fixture.spawn_start("w-b", &["--wait"]);
    let (a_code, a_json, _) = a.finish();
    let (b_code, b_json, _) = b.finish();
    assert_eq!((a_code, b_code), (0, 0), "{a_json} {b_json}");
    assert!(fixture.runs("w-a") && fixture.runs("w-b"));
    fixture.end_outside("w-linux", linux);
}

#[test]
fn a_linux_start_never_joins_the_queue() {
    let _serial = serial();
    let fixture = Fixture::healthy();
    fixture.start_and_wait_until_running("l-a");
    fixture.start_and_wait_until_running("l-b");
    let mut waiter = fixture.spawn_start("l-c", &["--wait"]);
    waiter.until_stderr_has("place 1");

    // The fake tart has a capacity of 2, so a Linux VM would be refused by the framework.
    // The app asks no slot for it, and it starts at once, as it did before the queue.
    std::fs::write(fixture.fake_state().join("capacity"), "3").unwrap();
    let out = fixture.app.cli(&[
        "vm",
        "start",
        "--platform",
        "linux",
        "--base",
        LINUX_BASE,
        "--id",
        "l-lin",
        "--json",
    ]);
    // The agent of this fixture states macOS, and a Linux start accepts what answers.
    assert_eq!(out.code(), 0, "{} {}", out.out(), out.err());
    assert!(fixture.runs("l-lin"));
    assert!(!waiter.finished(), "the macOS waiter still waits");
    waiter.kill();
}

#[test]
fn a_cancel_while_the_start_waits_removes_its_clone_and_moves_the_places_up() {
    let _serial = serial();
    let fixture = Fixture::healthy();
    fixture.start_and_wait_until_running("c-a");
    fixture.start_and_wait_until_running("c-b");
    let first = fixture.spawn_start("c-c", &["--wait"]);
    first.until_stderr_has("place 1");
    let second = fixture.spawn_start("c-d", &["--wait"]);
    second.until_stderr_has("place 2");
    assert!(fixture.cloned("c-c") && fixture.cloned("c-d"));

    first.kill();
    until("the app to remove the clone of the cancelled start", || {
        !fixture.cloned("c-c")
    });
    second.until_stderr_has("place 1");
    assert!(
        fixture.tart_run_pid("c-c").is_none(),
        "the cancelled start ran no tart run"
    );
    let [spec, meta] = sidecars(&fixture, "c-c");
    assert!(!spec.exists() && !meta.exists());

    // No slot leaked: a stop admits the remaining waiter.
    fixture.stop("c-a");
    let (code, envelope, err) = second.finish();
    assert_eq!(code, 0, "{envelope} {err}");
    assert_eq!(fixture.refusals(), Vec::<String>::new());
}

#[test]
fn an_admitted_start_holds_its_slot_while_it_boots_and_a_cancel_releases_it() {
    let _serial = serial();
    // The agent never answers, so each start stays in its boot with its slot.
    let fixture = Fixture::starved();
    let first = fixture.spawn_start("b-a", &[]);
    let second = fixture.spawn_start("b-b", &[]);
    until("both starts to boot", || {
        fixture.runs("b-a") && fixture.runs("b-b")
    });

    // Both slots are held by starts that have no meta yet: only the admission holds them.
    let out = fixture.app.cli(&[
        "vm",
        "start",
        "--platform",
        "macos",
        "--base",
        MACOS_BASE,
        "--id",
        "b-c",
        "--json",
    ]);
    assert_eq!(out.code(), 6, "{} {}", out.out(), out.err());
    assert_eq!(out.json()["details"]["refused_by"], "app");
    assert_eq!(out.json()["details"]["holders"], json!(["b-a", "b-b"]));

    // The clients go. The app removes the VMs, and then the slots are free.
    let tart_a = fixture.tart_run_pid("b-a").unwrap();
    first.kill();
    second.kill();
    until("the app to remove both clones", || {
        !fixture.cloned("b-a") && !fixture.cloned("b-b")
    });
    until("tart run to end", || {
        !testanyware_vm::process::process_alive(tart_a)
    });

    let third = fixture.spawn_start("b-d", &[]);
    until("the freed slot to admit a start", || fixture.runs("b-d"));
    third.kill();
    assert_eq!(fixture.refusals(), Vec::<String>::new());
}

#[test]
fn a_refusal_by_the_framework_after_the_admission_is_named_and_releases_the_slot() {
    let _serial = serial();
    let fixture = Fixture::healthy();
    // The count shows free slots, and the framework refuses anyway, as for a guest of another
    // user that the app cannot see.
    std::fs::write(fixture.fake_state().join("full"), "").unwrap();
    let out = fixture.app.cli(&[
        "vm",
        "start",
        "--platform",
        "macos",
        "--base",
        MACOS_BASE,
        "--id",
        "fw-a",
        "--json",
    ]);
    assert_eq!(out.code(), 6, "{} {}", out.out(), out.err());
    let envelope = out.json();
    let details = &envelope["details"];
    assert_eq!(details["refused_by"], "framework", "{envelope}");
    assert_eq!(details["limit"], 2);
    assert_eq!(
        details["place"], 1,
        "the place that the start would take again"
    );
    assert!(
        details["request_id"].as_str().unwrap().starts_with("r-"),
        "{details}"
    );
    assert!(
        details["tart_error"]
            .as_str()
            .unwrap()
            .contains("exceeds the system limit"),
        "{details}"
    );
    assert_eq!(details["holders"], json!(["holder-a", "holder-b"]));
    assert!(!fixture.cloned("fw-a"));
    // The count showed a free slot, so the remedy names both causes.
    let remedy = envelope["remediation"].as_str().unwrap_or_default();
    assert!(remedy.contains("another user or tool"), "{remedy}");
    assert!(remedy.contains("restart of the Mac"), "{remedy}");

    // The refused start released its slot. The next start is admitted at once, and so is a
    // second one: neither waits behind a hold that the refusal left behind.
    std::fs::remove_file(fixture.fake_state().join("full")).unwrap();
    fixture.start_and_wait_until_running("fw-b");
    fixture.start_and_wait_until_running("fw-c");
}

#[test]
fn a_refusal_at_the_deadline_of_a_waiting_start_is_the_verdict_of_the_framework() {
    let _serial = serial();
    let fixture = Fixture::healthy();
    std::fs::write(fixture.fake_state().join("full"), "").unwrap();
    let began = Instant::now();
    let client = fixture.spawn_start("rt-a", &["--wait=2"]);
    let (code, envelope, err) = client.finish();
    assert_eq!(code, 6, "{envelope} {err}");
    assert!(
        began.elapsed() >= Duration::from_secs(2),
        "it waited its two seconds"
    );
    let details = &envelope["details"];
    assert_eq!(details["refused_by"], "framework");
    assert!(details["waited_secs"].as_u64().unwrap() >= 2, "{details}");
    // The queue took the start back after its first refusal, and the pause ended at the deadline
    // of the head: the last try came at the deadline, and the framework refused it too.
    assert!(err.contains("the framework refused request r-"), "{err}");
    assert!(err.contains("pause of 3 s"), "{err}");
    assert!(fixture.refusals().len() >= 2, "{:?}", fixture.refusals());
    assert!(!fixture.cloned("rt-a"));

    // The slot that the failed start held is free again.
    std::fs::remove_file(fixture.fake_state().join("full")).unwrap();
    fixture.start_and_wait_until_running("rt-b");
    fixture.start_and_wait_until_running("rt-c");
}

/// The refusals of `id` in the log of the fake.
fn refusals_of(fixture: &Fixture, id: &str) -> usize {
    fixture.refusals().iter().filter(|name| *name == id).count()
}

#[test]
fn three_starts_beside_a_guest_that_the_app_cannot_see_get_their_slots_in_the_order_of_their_join()
{
    let _serial = serial();
    let fixture = Fixture::healthy();
    // The fake framework holds one VM, although the count of the app shows two free slots. This
    // stands for a guest of another user, which the app cannot see.
    std::fs::write(fixture.fake_state().join("capacity"), "1").unwrap();

    let a = fixture.spawn_start("u-a", &["--wait"]);
    let (a_code, a_json, a_err) = a.finish();
    assert_eq!(a_code, 0, "{a_json} {a_err}");
    assert!(fixture.runs("u-a"));

    // The count shows a free slot, so the app admits b, and the framework refuses it.
    let mut b = fixture.spawn_start("u-b", &["--wait"]);
    b.until_stderr_has("the framework refused request r-");
    let line = b.stderr();
    assert!(line.contains("pause of 3 s"), "{line}");
    assert!(line.contains("exceeds the system limit"), "{line}");
    let mut c = fixture.spawn_start("u-c", &["--wait"]);
    c.until_stderr_has("place 2");

    // A start without `--wait` fails at once: a waiter stands ahead, and contention holds.
    let began = Instant::now();
    let out = fixture.app.cli(&[
        "vm",
        "start",
        "--platform",
        "macos",
        "--base",
        MACOS_BASE,
        "--id",
        "u-d",
        "--json",
    ]);
    assert_eq!(out.code(), 6, "{} {}", out.out(), out.err());
    assert!(began.elapsed() < Duration::from_secs(2), "it fails at once");
    let envelope = out.json();
    assert_eq!(envelope["details"]["refused_by"], "app", "{envelope}");
    let remedy = envelope["remediation"].as_str().unwrap_or_default();
    assert!(remedy.contains("stuck"), "{remedy}");

    // The app tries b alone, every 3 s, and never c first. Wait for two more tries of b.
    until("two refusals of b", || refusals_of(&fixture, "u-b") >= 2);
    assert_eq!(refusals_of(&fixture, "u-c"), 0, "c was never tried first");
    assert_eq!(
        refusals_of(&fixture, "u-d"),
        0,
        "the start without a wait ran no tart run"
    );
    assert!(fixture.tart_run_pid("u-c").is_none());
    assert!(!b.finished() && !c.finished());

    // The unseen guest goes: a stops. The next try of b gets the slot, and c is still behind it.
    fixture.stop("u-a");
    let (b_code, b_json, b_err) = b.finish();
    assert_eq!(b_code, 0, "{b_json} {b_err}");
    assert!(fixture.runs("u-b"));
    assert!(!c.finished(), "c waits for the slot of b");

    // The acceptance of b ended contention, so the app admits c by the count. The framework
    // refuses it while b runs, and c gets its slot after b stops.
    until("the first refusal of c", || {
        refusals_of(&fixture, "u-c") >= 1
    });
    assert!(fixture.tart_run_pid("u-c").is_none() || !fixture.runs("u-c"));
    fixture.stop("u-b");
    let (c_code, c_json, c_err) = c.finish();
    assert_eq!(c_code, 0, "{c_json} {c_err}");
    assert!(fixture.runs("u-c"));
}

#[test]
fn a_quit_of_the_app_ends_a_waiting_start_with_app_unreachable_and_the_cause_quitting() {
    let _serial = serial();
    let fixture = Fixture::healthy();
    fixture.start_and_wait_until_running("k-a");
    fixture.start_and_wait_until_running("k-b");
    let mut waiter = fixture.spawn_start("k-c", &["--wait"]);
    waiter.until_stderr_has("place 1");

    nix::sys::signal::kill(
        nix::unistd::Pid::from_raw(fixture.app.pid() as i32),
        nix::sys::signal::Signal::SIGTERM,
    )
    .unwrap();
    assert!(!waiter.finished() || true);
    let (code, envelope, err) = waiter.finish();
    assert_ne!(code, 0, "{envelope} {err}");
    assert_eq!(envelope["code"], "APP_UNREACHABLE", "{envelope}");
    assert_eq!(envelope["details"]["cause"], "quitting", "{envelope}");
}

#[test]
fn after_a_restart_the_queue_is_empty_and_the_adopted_guests_count() {
    let _serial = serial();
    // Two guests that a run of the app before left running, each with a meta and no identity.
    // The app that starts now adopts them, and they hold both slots.
    let fixture = Fixture::healthy_after(|app| {
        app.add_sidecars("adopted-1", "macos", "tart", 5900);
        app.add_sidecars("adopted-2", "macos", "tart", 5900);
    });
    let out = fixture.app.cli(&[
        "vm",
        "start",
        "--platform",
        "macos",
        "--base",
        MACOS_BASE,
        "--id",
        "r-a",
        "--wait=1",
        "--json",
    ]);
    assert_eq!(out.code(), 6, "{} {}", out.out(), out.err());
    let details = &out.json()["details"];
    assert_eq!(details["refused_by"], "app");
    assert_eq!(details["place"], 1, "the queue starts empty");
    assert_eq!(details["holders"], json!(["adopted-1", "adopted-2"]));

    // One adopted guest goes away without an event. The tick of the queue finds the free slot.
    let waiter = fixture.spawn_start("r-b", &["--wait"]);
    let state = fixture.app.state_dir().join("vms");
    until("the waiter to join", || waiter.stderr().contains("place 1"));
    for suffix in ["json", "meta.json"] {
        std::fs::remove_file(state.join(format!("adopted-1.{suffix}"))).unwrap();
    }
    let (code, envelope, err) = waiter.finish();
    assert_eq!(code, 0, "{envelope} {err}");
}

// A slow tart and a tart that does not answer (spec "admission-queue": "The count": "When tart does
// not answer", and "Test seams").

#[test]
fn a_wait_that_runs_out_during_a_slow_observation_fails_at_its_deadline_and_a_slot_that_frees_later_admits_nobody(
) {
    let _serial = serial();
    let fixture = Fixture::healthy();
    let first = fixture.run_outside("s-ext-1", "darwin");
    let second = fixture.run_outside("s-ext-2", "darwin");
    // Each `tart get` answers after 4 s, inside the bound of 5 s. So no call fails, and one
    // observation takes about 8 s.
    fixture.set_fake("get-delay", "4");

    let began = Instant::now();
    let client = fixture.spawn_start("s-a", &["--wait=2"]);
    let (code, envelope, err) = client.finish();
    let waited = began.elapsed();
    assert_eq!(code, 6, "{envelope} {err}");
    assert!(
        waited >= Duration::from_secs(2) && waited < Duration::from_secs(6),
        "the deadline decided at once, and not after the observation: {waited:?}"
    );
    let details = &envelope["details"];
    assert_eq!(details["refused_by"], "app");
    assert_eq!(details["place"], 1);
    let remedy = envelope["remediation"].as_str().unwrap_or_default();
    assert!(
        remedy.contains("tart does not answer or answers late"),
        "{remedy}"
    );

    // A slot frees only after the deadline. The start is gone, so nobody takes the slot.
    fixture.end_outside("s-ext-1", first);
    fixture.clear_fake("get-delay");
    fixture.start_and_wait_until_running("s-b");
    assert!(!fixture.cloned("s-a"), "the failed start leaves no clone");
    assert!(
        !fixture.runs("s-a"),
        "the app never admitted the failed start later"
    );
    assert_eq!(fixture.refusals(), Vec::<String>::new());
    fixture.end_outside("s-ext-2", second);
}

#[test]
fn a_start_without_a_wait_behind_a_waiter_fails_at_once_during_a_slow_observation() {
    let _serial = serial();
    let fixture = Fixture::healthy();
    let first = fixture.run_outside("p-ext-1", "darwin");
    let second = fixture.run_outside("p-ext-2", "darwin");
    fixture.set_fake("get-delay", "4");
    let mut waiter = fixture.spawn_start("p-w", &["--wait"]);
    waiter.until_stderr_has("place 1");

    let began = Instant::now();
    let out = fixture.app.cli(&[
        "vm",
        "start",
        "--platform",
        "macos",
        "--base",
        MACOS_BASE,
        "--id",
        "p-d",
        "--json",
    ]);
    assert!(
        began.elapsed() < Duration::from_secs(4),
        "a refusal that needs no answer waits for no observation: {:?}",
        began.elapsed()
    );
    assert_eq!(out.code(), 6, "{} {}", out.out(), out.err());
    let details = &out.json()["details"];
    assert_eq!(details["refused_by"], "app");
    assert_eq!(details["place"], 2);
    assert!(!waiter.finished());
    waiter.kill();
    fixture.end_outside("p-ext-1", first);
    fixture.end_outside("p-ext-2", second);
}

#[test]
fn a_tart_that_does_not_answer_is_said_in_the_lines_of_the_wait_and_a_later_read_frees_the_slot() {
    let _serial = serial();
    let fixture = Fixture::healthy();
    fixture.start_and_wait_until_running("t-a");
    fixture.start_and_wait_until_running("t-b");
    let waiter = fixture.spawn_start("t-w", &["--wait"]);
    waiter.until_stderr_has("place 1");
    assert!(!waiter.stderr().contains("does not answer"));
    // An answer exists by now. Then `tart list` stops answering. The bound of 5 s ends the
    // call that hangs, and the observation fails.
    std::thread::sleep(Duration::from_secs(1));
    fixture.set_fake("list-hang", "");
    waiter.until_stderr_has("tart does not answer");
    assert!(
        waiter.stderr().contains("the last answer came"),
        "{}",
        waiter.stderr()
    );

    // tart answers again, and a stop frees a slot.
    fixture.clear_fake("list-hang");
    fixture.stop("t-a");
    let (code, envelope, err) = waiter.finish();
    assert_eq!(code, 0, "{envelope} {err}");
    assert!(fixture.runs("t-w"));
    assert_eq!(fixture.refusals(), Vec::<String>::new());
}

#[test]
fn a_start_that_fails_while_tart_does_not_answer_says_so_in_its_remedy() {
    let _serial = serial();
    let fixture = Fixture::healthy();
    fixture.start_and_wait_until_running("u-a");
    fixture.start_and_wait_until_running("u-b");
    // The start asks `tart list` once before it joins, and so does its cleanup after a refusal.
    // Those two calls have no bound. So the hang begins after the join, and it ends after the
    // deadline. The wait is long enough for one failed observation.
    let client = fixture.spawn_start("u-c", &["--wait=14"]);
    client.until_stderr_has("place 1");
    std::thread::sleep(Duration::from_secs(1));
    fixture.set_fake("list-hang", "");
    client.until_stderr_has("tart does not answer");
    std::thread::sleep(Duration::from_secs(8));
    fixture.clear_fake("list-hang");
    let (code, envelope, err) = client.finish();
    assert_eq!(code, 6, "{envelope} {err}");
    assert_eq!(envelope["details"]["refused_by"], "app");
    let remedy = envelope["remediation"].as_str().unwrap_or_default();
    assert!(remedy.contains("tart does not answer"), "{remedy}");
    assert!(remedy.contains("The last answer of tart came"), "{remedy}");
    assert!(err.contains("tart does not answer"), "{err}");
    fixture.clear_fake("list-hang");
}

fn sidecars(fixture: &Fixture, id: &str) -> [PathBuf; 2] {
    let vms = fixture.app.state_dir().join("vms");
    [
        vms.join(format!("{id}.json")),
        vms.join(format!("{id}.meta.json")),
    ]
}

// ---- `vm slots` ----

/// `vm slots --json` through the real CLI, as the envelope.
fn slots(fixture: &Fixture) -> Value {
    let out = fixture.app.cli(&["vm", "slots", "--json"]);
    assert_eq!(out.code(), 0, "{} {}", out.out(), out.err());
    out.json()
}

/// A directory that stands for the worktree of a grove, in its canonical form: the CLI states
/// what `getcwd` returns, and a temporary directory of macOS is a link.
fn worktree(parent: &std::path::Path, name: &str) -> PathBuf {
    let dir = parent.join(name);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::canonicalize(dir).unwrap()
}

fn holder<'a>(view: &'a Value, vm: &str) -> &'a Value {
    view["holders"]
        .as_array()
        .unwrap()
        .iter()
        .find(|holder| holder["vms"] == json!([vm]))
        .unwrap_or_else(|| panic!("{vm} is no holder in {view}"))
}

#[test]
fn vm_slots_lists_the_waiters_of_two_worktrees_in_order_with_their_pids_and_directories() {
    let _serial = serial();
    let fixture = Fixture::healthy();
    fixture.start_and_wait_until_running("v-a");
    fixture.start_and_wait_until_running("v-b");

    let trees = tempfile::tempdir().unwrap();
    let (one, two) = (worktree(trees.path(), "one"), worktree(trees.path(), "two"));
    let c = fixture.spawn_start_in(&one, "v-c", &["--wait=300"]);
    c.until_stderr_has("place 1");
    let d = fixture.spawn_start_in(&two, "v-d", &["--wait=300"]);
    d.until_stderr_has("place 2");
    let (c_pid, d_pid) = (c.child.id(), d.child.id());

    let view = slots(&fixture);
    assert_eq!(view["ok"], true, "{view}");
    assert_eq!((view["slots"].as_u64(), view["limit"].as_u64()), (Some(2), Some(2)));
    assert_eq!(view["contention"], Value::Null, "{view}");
    assert!(view["observed_at"].is_u64(), "the app observed tart first: {view}");
    assert_eq!(view["unanswered_since"], Value::Null, "{view}");

    // Both waiters, in the order of their join, each with the pid of its process and the
    // directory it ran in.
    let queue = view["queue"].as_array().unwrap();
    assert_eq!(queue.len(), 2, "{view}");
    for (waiter, place, vm, pid, cwd) in [
        (&queue[0], 1, "v-c", c_pid, &one),
        (&queue[1], 2, "v-d", d_pid, &two),
    ] {
        assert_eq!(waiter["place"], place, "{view}");
        assert_eq!(waiter["kind"], "vm", "{view}");
        assert_eq!(waiter["vms"], json!([vm]), "{view}");
        assert_eq!(waiter["requested_by"]["pid"], pid, "{view}");
        assert_eq!(waiter["requested_by"]["cwd"], cwd.to_str().unwrap(), "{view}");
        assert!(waiter["deadline"].as_u64() > waiter["since"].as_u64(), "{view}");
    }
    // The request id is the one that the waiting start names on its own stderr.
    let id = queue[0]["request_id"].as_str().unwrap();
    assert!(c.stderr().contains(id), "{id} missing from {}", c.stderr());

    // Both slots are held by guests that the app started. This test process ran those CLIs.
    let here = std::env::current_dir().unwrap();
    for vm in ["v-a", "v-b"] {
        let held = holder(&view, vm);
        assert_eq!((held["kind"].as_str(), held["state"].as_str()), (Some("vm"), Some("running")));
        assert!(held["request_id"].as_str().unwrap().starts_with("r-"), "{view}");
        assert!(held["requested_by"]["pid"].is_u64(), "{view}");
        assert_eq!(held["requested_by"]["cwd"], here.to_str().unwrap(), "{view}");
        assert!(held["since"].is_u64(), "{view}");
    }

    // A stop admits the head. It becomes a holder with the pid and the directory it asked from,
    // and the place of the other waiter moves up.
    fixture.stop("v-a");
    let (code, envelope, err) = c.finish();
    assert_eq!(code, 0, "{envelope} {err}");
    let view = slots(&fixture);
    let admitted = holder(&view, "v-c");
    assert_eq!(admitted["request_id"], id, "{view}");
    assert_eq!(admitted["requested_by"]["pid"], c_pid, "{view}");
    assert_eq!(admitted["requested_by"]["cwd"], one.to_str().unwrap(), "{view}");
    let queue = view["queue"].as_array().unwrap();
    assert_eq!(queue.len(), 1, "{view}");
    assert_eq!((queue[0]["place"].as_u64(), &queue[0]["vms"]), (Some(1), &json!(["v-d"])));

    fixture.stop("v-b");
    let (code, envelope, err) = d.finish();
    assert_eq!(code, 0, "{envelope} {err}");
    assert_eq!(fixture.refusals(), Vec::<String>::new());
}

#[test]
fn vm_slots_shows_a_guest_outside_the_app_as_external_with_no_requester() {
    let _serial = serial();
    let fixture = Fixture::healthy();
    let outside = fixture.run_outside("ext-guest", "darwin");
    fixture.start_and_wait_until_running("e-a");

    let view = slots(&fixture);
    let external = holder(&view, "ext-guest");
    assert_eq!(external["kind"], "external", "{view}");
    assert_eq!(external["state"], "running", "{view}");
    assert_eq!(external["request_id"], Value::Null, "{view}");
    assert_eq!(external["requested_by"], Value::Null, "{view}");
    assert_eq!(holder(&view, "e-a")["kind"], "vm", "{view}");
    assert_eq!(view["holders"].as_array().unwrap().len(), 2, "{view}");

    fixture.end_outside("ext-guest", outside);
}

#[test]
fn vm_slots_in_text_names_the_holders_the_waiters_and_who_asked() {
    let _serial = serial();
    let fixture = Fixture::healthy();
    fixture.start_and_wait_until_running("x-a");
    fixture.start_and_wait_until_running("x-b");
    let trees = tempfile::tempdir().unwrap();
    let one = worktree(trees.path(), "text-tree");
    let waiter = fixture.spawn_start_in(&one, "x-c", &["--wait=300"]);
    waiter.until_stderr_has("place 1");

    let out = fixture.app.cli(&["vm", "slots"]);
    assert_eq!(out.code(), 0, "{}", out.err());
    let text = out.out();
    for needle in ["x-a", "x-b", "x-c", "Waiting:", one.to_str().unwrap(), "pid"] {
        assert!(text.contains(needle), "{needle} missing from {text}");
    }
    waiter.kill();
}

/// One raw request on the socket of the app, as a client of the given range sends it.
fn raw_get(
    app: &TestApp,
    path: &str,
    range: testanyware_app_protocol::ProtocolRange,
) -> (u16, Value) {
    use std::io::Write;
    use testanyware_app_protocol::{ClientHello, Directories};
    let hello = ClientHello::new(
        range,
        env!("CARGO_PKG_VERSION"),
        Directories {
            state_dir: app.state_dir(),
            data_dir: app.data_dir(),
        },
    );
    let mut request = format!("GET {path} HTTP/1.1\r\nHost: test\r\nConnection: close\r\n");
    for (name, value) in hello.to_headers() {
        request.push_str(&format!("{name}: {value}\r\n"));
    }
    request.push_str("\r\n");
    let mut stream = std::os::unix::net::UnixStream::connect(app.socket()).unwrap();
    stream.write_all(request.as_bytes()).unwrap();
    let mut reply = String::new();
    stream.read_to_string(&mut reply).unwrap();
    let (head, body) = reply.split_once("\r\n\r\n").expect("a response");
    let status = head.split(' ').nth(1).unwrap().parse().unwrap();
    (status, serde_json::from_str(body).unwrap_or(Value::Null))
}

#[test]
fn the_slots_request_is_one_of_version_2_and_a_client_of_version_1_has_no_such_route() {
    let _serial = serial();
    use testanyware_app_protocol::ProtocolRange;
    let fixture = Fixture::healthy();

    let (status, body) = raw_get(&fixture.app, "/v1/slots", ProtocolRange::new(1, 2));
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["slots"], 2, "{body}");

    let (status, body) = raw_get(&fixture.app, "/v1/slots", ProtocolRange::new(2, 2));
    assert_eq!(status, 200, "a client of version 2 only: {body}");

    // The pair agrees on version 1, so the request has no route. Nothing changed in the queue.
    let (status, body) = raw_get(&fixture.app, "/v1/slots", ProtocolRange::new(1, 1));
    assert_eq!(status, 404, "{body}");
    assert_eq!(body["code"], "USAGE_ERROR", "{body}");
    assert_eq!(body["details"]["version"], 1, "{body}");

    // The other requests of version 1 keep their form for a client of version 1.
    let (status, body) = raw_get(&fixture.app, "/v1/vms?limit=10", ProtocolRange::new(1, 1));
    assert_eq!(status, 200, "{body}");
}

#[test]
fn a_start_states_its_directory_and_an_older_start_without_one_still_queues() {
    let _serial = serial();
    let fixture = Fixture::healthy();
    fixture.start_and_wait_until_running("w-a");
    fixture.start_and_wait_until_running("w-b");

    // An older Host CLI states no `cwd`: the body has none. The app takes the start and the
    // waiter shows a pid and a `cwd` of `null`.
    let body = json!({
        "platform": "macos",
        "base": MACOS_BASE,
        "id": "w-old",
        "wait_secs": 300,
    });
    let mut stream = std::os::unix::net::UnixStream::connect(fixture.app.socket()).unwrap();
    let hello = testanyware_app_protocol::ClientHello::new(
        testanyware_app_protocol::ProtocolRange::new(1, 1),
        env!("CARGO_PKG_VERSION"),
        testanyware_app_protocol::Directories {
            state_dir: fixture.app.state_dir(),
            data_dir: fixture.app.data_dir(),
        },
    );
    let text = body.to_string();
    let mut request = format!(
        "POST /v1/vms HTTP/1.1\r\nHost: test\r\nContent-Type: application/json\r\nContent-Length: {}\r\n",
        text.len()
    );
    for (name, value) in hello.to_headers() {
        request.push_str(&format!("{name}: {value}\r\n"));
    }
    request.push_str("\r\n");
    request.push_str(&text);
    use std::io::Write;
    stream.write_all(request.as_bytes()).unwrap();

    until("the old start to wait", || {
        slots(&fixture)["queue"].as_array().is_some_and(|q| q.len() == 1)
    });
    let view = slots(&fixture);
    let waiter = &view["queue"][0];
    assert_eq!(waiter["vms"], json!(["w-old"]), "{view}");
    assert_eq!(waiter["requested_by"]["pid"], std::process::id(), "this process connected: {view}");
    assert_eq!(waiter["requested_by"]["cwd"], Value::Null, "{view}");
    drop(stream);
}

// ---- golden creation holds a slot (spec "admission-queue": "Golden creation") ----
//
// The hold request runs through the library of the Host CLI, as the command runs it. No test runs
// the real pipeline of golden creation, which takes 10 to 40 minutes: the hold is the open
// connection, and the pipeline only has to keep the lease until its setup VM has stopped.

/// The process states the directories that the app reads, as the Host CLI states its own. The
/// serial lock keeps another test from changing them.
fn state_directories_of(fixture: &Fixture) {
    std::env::set_var("XDG_STATE_HOME", fixture.app.xdg_state_home());
    std::env::set_var("XDG_DATA_HOME", fixture.app.xdg_data_home());
}

fn runtime() -> tokio::runtime::Runtime {
    // The lease owns tasks that keep its connection open, so the runtime runs them on its own
    // threads while the test thread waits.
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("a tokio runtime")
}

type Lines = Arc<Mutex<Vec<String>>>;
type Lost = Arc<std::sync::atomic::AtomicBool>;
type Held = tokio::task::JoinHandle<Result<testanyware_app_client::SlotLease, testanyware_app_protocol::AppError>>;

/// Ask for the slot of a golden creation on `runtime`, as the command does. The progress lines
/// land in the first value, and the second becomes true if the app closes the connection of an
/// admitted hold.
fn ask_for_the_hold(
    runtime: &tokio::runtime::Runtime,
    fixture: &Fixture,
    vms: &[&str],
) -> (Held, Lines, Lost) {
    let lines: Lines = Arc::default();
    let lost: Lost = Arc::default();
    let socket = fixture.app.socket();
    let vms: Vec<String> = vms.iter().map(|vm| vm.to_string()).collect();
    let task = runtime.spawn({
        let lines = Arc::clone(&lines);
        let lost = Arc::clone(&lost);
        async move {
            testanyware_cli::app_route::hold_slot(
                socket,
                vms,
                move |line| lines.lock().unwrap().push(line.to_string()),
                move || lost.store(true, std::sync::atomic::Ordering::SeqCst),
            )
            .await
        }
    });
    (task, lines, lost)
}

fn lines_have(lines: &Lines, text: &str) -> bool {
    lines.lock().unwrap().iter().any(|line| line.contains(text))
}

fn golden_holders(view: &Value) -> Vec<&Value> {
    view["holders"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|holder| holder["kind"] == "golden")
        .collect()
}

#[test]
fn golden_creation_behind_two_vms_waits_reports_its_place_and_holds_one_slot_for_its_connection() {
    let _serial = serial();
    let fixture = Fixture::healthy();
    state_directories_of(&fixture);
    fixture.start_and_wait_until_running("h-a");
    fixture.start_and_wait_until_running("h-b");
    let runtime = runtime();

    let (held, lines, _) = ask_for_the_hold(&runtime, &fixture, &["h-setup", "h-base-building"]);
    until("the golden creation to report its place", || {
        lines_have(&lines, "place 1")
    });
    let line = lines.lock().unwrap()[0].clone();
    assert!(line.contains("request r-") && line.contains("h-a") && line.contains("h-b"), "{line}");

    // The queue shows the waiter as a golden creation, with who asked and no bound.
    let view = slots(&fixture);
    let waiter = &view["queue"][0];
    assert_eq!(waiter["kind"], "golden", "{view}");
    assert_eq!(waiter["vms"], json!(["h-base-building", "h-setup"]), "{view}");
    assert_eq!(waiter["deadline"], Value::Null, "the wait has no bound: {view}");
    assert_eq!(waiter["requested_by"]["pid"], std::process::id(), "{view}");
    assert_eq!(
        waiter["requested_by"]["cwd"],
        std::fs::canonicalize(std::env::current_dir().unwrap()).unwrap().display().to_string(),
        "{view}"
    );
    std::thread::sleep(Duration::from_millis(500));
    assert!(!held.is_finished(), "both slots are held");

    // A start that joins later stays behind the golden creation.
    let mut later = fixture.spawn_start("h-later", &["--wait"]);
    later.until_stderr_has("place 2");

    // A stop frees one slot, and the app admits the golden creation first.
    fixture.stop("h-a");
    let lease = runtime
        .block_on(held)
        .unwrap()
        .expect("the stop admits the golden creation");
    let view = slots(&fixture);
    let holders = golden_holders(&view);
    assert_eq!(holders.len(), 1, "{view}");
    assert_eq!(holders[0]["state"], "held", "{view}");
    assert_eq!(holders[0]["request_id"], lease.request_id(), "{view}");
    assert_eq!(holders[0]["vms"], json!(["h-base-building", "h-setup"]), "{view}");
    std::thread::sleep(Duration::from_millis(500));
    assert!(!later.finished(), "the golden creation holds the slot that the stop freed");
    assert!(fixture.tart_run_pid("h-later").is_none());

    // The end of the golden creation closes the connection, and the app admits the start.
    drop(lease);
    let (code, envelope, err) = later.finish();
    assert_eq!(code, 0, "{envelope} {err}");
    assert_eq!(fixture.refusals(), Vec::<String>::new(), "the framework refused no start");
}

#[test]
fn a_start_beside_the_hold_takes_the_other_slot_and_the_next_start_waits() {
    let _serial = serial();
    let fixture = Fixture::healthy();
    state_directories_of(&fixture);
    let runtime = runtime();

    // Nothing runs, and the golden creation boots nothing yet: it holds a slot between boots.
    let (held, _, _) = ask_for_the_hold(&runtime, &fixture, &["g-setup"]);
    let lease = runtime.block_on(held).unwrap().expect("a free slot admits it at once");
    assert!(lease.request_id().starts_with("r-"));

    fixture.start_and_wait_until_running("g-one");
    let mut second = fixture.spawn_start("g-two", &["--wait"]);
    second.until_stderr_has("place 1");
    assert!(
        second.stderr().contains("g-one") && second.stderr().contains("g-setup"),
        "the line names the start and the golden creation: {}",
        second.stderr()
    );
    std::thread::sleep(Duration::from_millis(500));
    assert!(!second.finished(), "the hold and the first start fill both slots");
    assert!(fixture.tart_run_pid("g-two").is_none());

    // The setup VM does not run, so tart shows nothing of the golden creation. The count keeps it.
    let view = slots(&fixture);
    assert_eq!(golden_holders(&view).len(), 1, "{view}");
    assert!(holder(&view, "g-one")["kind"] == "vm", "{view}");

    drop(lease);
    let (code, envelope, err) = second.finish();
    assert_eq!(code, 0, "{envelope} {err}");
    let view = slots(&fixture);
    assert!(golden_holders(&view).is_empty(), "{view}");
    assert_eq!(fixture.refusals(), Vec::<String>::new());
}

/// POST `/v1/slots` by hand, as a client that the test controls. The reply is read up to the end
/// of the head and the first event.
fn raw_hold(fixture: &Fixture, vms: &[&str]) -> (std::os::unix::net::UnixStream, String) {
    use std::io::Write;
    use testanyware_app_protocol::{ClientHello, Directories, ProtocolRange};
    let hello = ClientHello::new(
        ProtocolRange::new(1, 2),
        env!("CARGO_PKG_VERSION"),
        Directories {
            state_dir: fixture.app.state_dir(),
            data_dir: fixture.app.data_dir(),
        },
    );
    let text = json!({ "vms": vms }).to_string();
    let mut request = format!(
        "POST /v1/slots HTTP/1.1\r\nHost: test\r\nContent-Type: application/json\r\nContent-Length: {}\r\n",
        text.len()
    );
    for (name, value) in hello.to_headers() {
        request.push_str(&format!("{name}: {value}\r\n"));
    }
    request.push_str("\r\n");
    request.push_str(&text);
    let mut stream = std::os::unix::net::UnixStream::connect(fixture.app.socket()).unwrap();
    stream.write_all(request.as_bytes()).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(20))).unwrap();
    let mut head = String::new();
    let mut byte = [0u8; 1];
    while !head.contains("\"admitted\"") && !head.contains("\"error\"") {
        if stream.read(&mut byte).unwrap_or(0) == 0 {
            break;
        }
        head.push(byte[0] as char);
    }
    (stream, head)
}

#[test]
fn the_crash_of_a_golden_creation_closes_its_connection_and_ends_the_hold() {
    let _serial = serial();
    let fixture = Fixture::healthy();
    let (stream, head) = raw_hold(&fixture, &["c-setup"]);
    assert!(head.contains("\"event\":\"admitted\""), "{head}");
    assert_eq!(golden_holders(&slots(&fixture)).len(), 1);

    // The kernel closes the connection of a process that crashes, which is what this does.
    drop(stream);
    until("the hold to end", || golden_holders(&slots(&fixture)).is_empty());
}

#[test]
fn a_setup_vm_that_still_runs_after_the_hold_ended_counts_as_a_seen_holder_and_is_never_stopped() {
    let _serial = serial();
    let fixture = Fixture::healthy();
    let (stream, head) = raw_hold(&fixture, &["r-setup"]);
    assert!(head.contains("admitted"), "{head}");
    // The golden creation booted its setup VM outside the app, as it does on the direct path.
    let setup = fixture.run_outside("r-setup", "darwin");
    until("the app to see the VM beside the hold", || {
        let view = slots(&fixture);
        golden_holders(&view).len() == 1 && view["holders"].as_array().unwrap().len() == 1
    });

    drop(stream);
    until("the hold to end", || golden_holders(&slots(&fixture)).is_empty());
    let view = slots(&fixture);
    let seen = holder(&view, "r-setup");
    assert_eq!(seen["kind"], "external", "{view}");
    assert!(fixture.runs("r-setup"), "the app never stops a VM that it did not start");
    fixture.end_outside("r-setup", setup);
}

#[test]
fn with_macos_slots_0_golden_creation_fails_with_vm_limit_reached() {
    let _serial = serial();
    let fixture = Fixture::healthy_with_config("#{ macos_slots: 0 }");
    state_directories_of(&fixture);
    let runtime = runtime();
    let (held, _, _) = ask_for_the_hold(&runtime, &fixture, &["z-setup"]);
    let error = runtime.block_on(held).unwrap().expect_err("no slot to hold");
    assert_eq!(error.code, "VM_LIMIT_REACHED", "{error:?}");
    assert_eq!(error.details["limit"], 0, "{error:?}");
    assert_eq!(error.details["refused_by"], "app", "{error:?}");
    assert!(slots(&fixture)["queue"].as_array().unwrap().is_empty());
}

#[test]
fn a_name_that_is_no_plain_vm_id_fails_with_usage_error_before_the_stream() {
    let _serial = serial();
    let fixture = Fixture::healthy();
    state_directories_of(&fixture);
    let runtime = runtime();
    for bad in [vec!["../escape"], vec!["ok", "a/b"], vec![]] {
        let (held, _, _) = ask_for_the_hold(&runtime, &fixture, &bad);
        let error = runtime.block_on(held).unwrap().expect_err("a usage error");
        assert_eq!(error.code, "USAGE_ERROR", "{bad:?}: {error:?}");
    }
    assert!(slots(&fixture)["queue"].as_array().unwrap().is_empty());
}

#[test]
fn a_quit_of_the_app_while_golden_creation_waits_fails_it_with_the_cause_quitting() {
    let _serial = serial();
    let fixture = Fixture::healthy();
    state_directories_of(&fixture);
    fixture.start_and_wait_until_running("q2-a");
    fixture.start_and_wait_until_running("q2-b");
    let runtime = runtime();
    let (held, lines, _) = ask_for_the_hold(&runtime, &fixture, &["q2-setup"]);
    until("the golden creation to wait", || lines_have(&lines, "place 1"));

    nix::sys::signal::kill(
        nix::unistd::Pid::from_raw(fixture.app.pid() as i32),
        nix::sys::signal::Signal::SIGTERM,
    )
    .unwrap();
    let error = runtime.block_on(held).unwrap().expect_err("the wait fails");
    assert_eq!(error.code, "APP_UNREACHABLE", "{error:?}");
    assert_eq!(error.details["cause"], "quitting", "{error:?}");
}

#[test]
fn a_quit_of_the_app_while_golden_creation_holds_its_slot_ends_the_hold_and_says_so_once() {
    let _serial = serial();
    let fixture = Fixture::healthy();
    state_directories_of(&fixture);
    let runtime = runtime();
    let (held, _, lost) = ask_for_the_hold(&runtime, &fixture, &["q3-setup"]);
    let lease = runtime.block_on(held).unwrap().expect("admitted");
    assert!(!lost.load(std::sync::atomic::Ordering::SeqCst));

    nix::sys::signal::kill(
        nix::unistd::Pid::from_raw(fixture.app.pid() as i32),
        nix::sys::signal::Signal::SIGTERM,
    )
    .unwrap();
    until("the lease to learn that the app is gone", || {
        lost.load(std::sync::atomic::Ordering::SeqCst)
    });
    // The golden creation goes on: dropping the lease afterwards is harmless.
    drop(lease);
}

#[test]
fn a_lease_that_the_golden_creation_drops_does_not_say_that_the_hold_was_lost() {
    let _serial = serial();
    let fixture = Fixture::healthy();
    state_directories_of(&fixture);
    let runtime = runtime();
    let (held, _, lost) = ask_for_the_hold(&runtime, &fixture, &["d-setup"]);
    drop(runtime.block_on(held).unwrap().expect("admitted"));
    until("the hold to end", || golden_holders(&slots(&fixture)).is_empty());
    std::thread::sleep(Duration::from_millis(300));
    assert!(!lost.load(std::sync::atomic::Ordering::SeqCst));
}

#[test]
fn the_hold_request_is_one_of_version_2_and_a_client_of_version_1_has_no_such_route() {
    let _serial = serial();
    use std::io::Write;
    use testanyware_app_protocol::{ClientHello, Directories, ProtocolRange};
    let fixture = Fixture::healthy();
    let hello = ClientHello::new(
        ProtocolRange::new(1, 1),
        env!("CARGO_PKG_VERSION"),
        Directories {
            state_dir: fixture.app.state_dir(),
            data_dir: fixture.app.data_dir(),
        },
    );
    let text = json!({ "vms": ["v1-setup"] }).to_string();
    let mut request = format!(
        "POST /v1/slots HTTP/1.1\r\nHost: test\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n",
        text.len()
    );
    for (name, value) in hello.to_headers() {
        request.push_str(&format!("{name}: {value}\r\n"));
    }
    request.push_str("\r\n");
    request.push_str(&text);
    let mut stream = std::os::unix::net::UnixStream::connect(fixture.app.socket()).unwrap();
    stream.write_all(request.as_bytes()).unwrap();
    let mut reply = String::new();
    stream.read_to_string(&mut reply).unwrap();
    assert!(reply.starts_with("HTTP/1.1 404"), "{reply}");
    assert!(reply.contains("\"USAGE_ERROR\""), "{reply}");
    assert!(golden_holders(&slots(&fixture)).is_empty());
}

// ---- the command `vm create-golden` ----

impl Fixture {
    /// An app whose `config.rhai` names the directories that the CLI of the test resolves, so
    /// that the directory check of golden creation passes. `extra` adds keys such as
    /// `macos_slots`.
    fn for_golden(extra: &str) -> Self {
        Self::healthy_with_config(&format!(
            r#"#{{ state_dir: "$STATE", data_dir: "$DATA"{extra} }}"#
        ))
    }

    /// A host that passes the fast checks of golden creation: an SSH keypair under `HOME`, and an
    /// agent binary named by the override.
    fn golden_host(&self) -> PathBuf {
        let ssh = self.app.root().join(".ssh");
        std::fs::create_dir_all(&ssh).unwrap();
        std::fs::write(ssh.join("id_ed25519"), "key").unwrap();
        std::fs::write(ssh.join("id_ed25519.pub"), "pub").unwrap();
        let agent = self.app.root().join("agent-binary");
        std::fs::write(&agent, "agent").unwrap();
        agent
    }

    /// The real `vm create-golden` of a macOS guest, in the background. The tools of the CLI are
    /// the fake `tart` first. The vanilla image `tahoe` is one that the fake cannot clone, so the
    /// pipeline stops at its first `tart clone`, after the slot. No test runs the real pipeline.
    fn spawn_golden(&self, agent: &std::path::Path) -> Client {
        let mut path = std::ffi::OsString::from(self.app.tool_dir());
        path.push(":");
        path.push(std::env::var_os("PATH").unwrap_or_default());
        let mut command = Command::new(CLI);
        command
            .args(["vm", "create-golden", "--platform", "macos", "--version", "tahoe", "--json"])
            .env_clear()
            .env("PATH", path)
            .env("HOME", self.app.root())
            .env("TMPDIR", self.app.tmp_dir())
            .env("XDG_STATE_HOME", self.app.xdg_state_home())
            .env("XDG_DATA_HOME", self.app.xdg_data_home())
            .env("TESTANYWARE_APP_SOCKET", self.app.socket())
            .env("TESTANYWARE_AGENT_BIN_OVERRIDE", agent)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        Client::spawn(command)
    }
}

#[test]
fn the_command_waits_for_its_slot_after_its_fast_checks_and_closes_the_hold_when_it_ends() {
    let _serial = serial();
    let fixture = Fixture::for_golden("");
    let agent = fixture.golden_host();
    fixture.start_and_wait_until_running("e-a");
    fixture.start_and_wait_until_running("e-b");

    let mut golden = fixture.spawn_golden(&agent);
    golden.until_stderr_has("place 1");
    let err = golden.stderr();
    assert!(err.contains("Using SSH key"), "the fast checks ran first: {err}");
    assert!(err.contains("e-a") && err.contains("e-b"), "{err}");
    assert!(!err.contains("Cloning"), "nothing changed before the slot: {err}");
    let view = slots(&fixture);
    let waiter = &view["queue"][0];
    assert_eq!(waiter["kind"], "golden", "{view}");
    assert_eq!(waiter["vms"].as_array().unwrap().len(), 1, "a vanilla base builds nothing: {view}");
    assert!(waiter["vms"][0].as_str().unwrap().starts_with("testanyware-setup-"), "{view}");
    assert_eq!(waiter["requested_by"]["cwd"], std::fs::canonicalize(std::env::current_dir().unwrap()).unwrap().display().to_string());
    std::thread::sleep(Duration::from_millis(500));
    assert!(!golden.finished());

    // A stop admits it. The fake cannot clone the vanilla image, so the pipeline fails there.
    fixture.stop("e-a");
    let (code, envelope, err) = golden.finish();
    assert_ne!(code, 0, "{envelope} {err}");
    assert!(err.contains("admitted this golden creation"), "{err}");
    assert!(err.contains("Cloning"), "the pipeline ran after the admission: {err}");
    assert!(
        err.find("admitted this golden creation") < err.find("Cloning"),
        "the slot came before the first change: {err}"
    );
    // The end of the process closed the connection, and the hold ended.
    until("the hold to end", || golden_holders(&slots(&fixture)).is_empty());
}

#[test]
fn a_host_that_fails_its_fast_checks_never_joins_the_queue() {
    let _serial = serial();
    let fixture = Fixture::for_golden("");
    fixture.start_and_wait_until_running("f-a");
    fixture.start_and_wait_until_running("f-b");
    // `HOME` holds no SSH key, so the check fails before any request for a slot.
    let golden = fixture.spawn_golden(&fixture.app.root().join("no-agent"));
    let (code, envelope, err) = golden.finish();
    assert_ne!(code, 0, "{envelope} {err}");
    assert_eq!(envelope["code"], "GOLDEN_CREATE_FAILED", "{envelope}");
    assert!(slots(&fixture)["queue"].as_array().unwrap().is_empty());
}

#[test]
fn with_macos_slots_0_the_command_fails_with_vm_limit_reached_before_it_changes_anything() {
    let _serial = serial();
    let fixture = Fixture::for_golden(", macos_slots: 0");
    let agent = fixture.golden_host();
    let golden = fixture.spawn_golden(&agent);
    let (code, envelope, err) = golden.finish();
    assert_eq!(code, 6, "{envelope} {err}");
    assert_eq!(envelope["code"], "VM_LIMIT_REACHED", "{envelope}");
    assert!(!err.contains("Cloning"), "{err}");
}

// ---- The start record (spec "admission-queue": "A quit, a crash and a restart") ----

#[test]
fn the_record_of_a_start_lives_while_the_start_runs_and_is_gone_after_each_end() {
    let _serial = serial();
    let fixture = Fixture::healthy();

    // A success: the meta describes the VM, and the record is gone before the result.
    fixture.start_and_wait_until_running("rec-a");
    assert!(!fixture.start_record("rec-a").exists(), "a success removes the record");
    assert!(sidecars(&fixture, "rec-a")[1].exists());

    // A failure: tart fails for any reason but the limit, and the cleanup has run.
    fixture.set_fake("run-fails", "");
    let out = fixture.app.cli(&[
        "vm", "start", "--platform", "macos", "--base", MACOS_BASE, "--id", "rec-b", "--json",
    ]);
    assert_ne!(out.code(), 0, "{} {}", out.out(), out.err());
    fixture.clear_fake("run-fails");
    assert!(!fixture.start_record("rec-b").exists(), "a failure removes the record");
    assert!(!fixture.cloned("rec-b"));
    assert!(fixture.status()["adoption"]["unfinished_starts"].as_array().unwrap().is_empty());
}

#[test]
fn a_cancelled_start_leaves_no_record_and_a_running_start_has_one_that_names_its_processes() {
    let _serial = serial();
    // The agent never answers, so the start stays in its boot.
    let fixture = Fixture::starved();
    let client = fixture.spawn_start("rec-c", &[]);
    until("the start to boot", || fixture.runs("rec-c"));
    let record_path = fixture.start_record("rec-c");
    until("the record to name the tart run and the keeper", || {
        std::fs::read_to_string(&record_path)
            .ok()
            .and_then(|text| serde_json::from_str::<Value>(&text).ok())
            .is_some_and(|record| !record["main"].is_null() && !record["keeper"].is_null())
    });
    let record: Value =
        serde_json::from_str(&std::fs::read_to_string(&record_path).unwrap()).unwrap();
    assert_eq!(record["id"], "rec-c");
    assert_eq!(record["backend"], "tart");
    assert_eq!(
        record["main"]["pid"].as_i64().map(|pid| pid as i32),
        fixture.tart_run_pid("rec-c")
    );
    assert!(record["main"]["identity"]["boot_session"].is_string(), "{record}");

    client.kill();
    until("the app to clean up the cancelled start", || !fixture.cloned("rec-c"));
    until("the record to go", || !record_path.exists());
}

#[test]
fn an_app_killed_during_a_boot_removes_that_vm_its_clone_and_its_sidecars_before_it_serves_again() {
    let _serial = serial();
    let mut fixture = Fixture::starved();
    let client = fixture.spawn_start("crash-a", &[]);
    until("the start to boot", || fixture.runs("crash-a"));
    let record_path = fixture.start_record("crash-a");
    until("the record to name the keeper", || {
        std::fs::read_to_string(&record_path).is_ok_and(|text| text.contains("keeper"))
    });
    let record: Value =
        serde_json::from_str(&std::fs::read_to_string(&record_path).unwrap()).unwrap();
    let tart_run = record["main"]["pid"].as_i64().unwrap() as i32;
    let keeper = record["keeper"]["pid"].as_i64().unwrap() as i32;

    fixture.crash();
    let (code, envelope, err) = client.finish();
    assert_ne!(code, 0, "{envelope} {err}");
    // The crash left the VM running, with its clone and its record.
    assert!(testanyware_vm::process::process_alive(tart_run));
    assert!(fixture.cloned("crash-a") && record_path.exists());

    fixture.restart();
    let status = fixture.status();

    assert_eq!(
        status["adoption"]["unfinished_starts"][0]["id"], "crash-a", "{status}"
    );
    assert_eq!(status["adoption"]["unfinished_starts"][0]["removed"], true, "{status}");
    until("the tart run to end", || !testanyware_vm::process::process_alive(tart_run));
    until("the keeper to end", || !testanyware_vm::process::process_alive(keeper));
    assert!(!fixture.cloned("crash-a"));
    assert!(!record_path.exists());
    assert!(!sidecars(&fixture, "crash-a")[0].exists() && !sidecars(&fixture, "crash-a")[1].exists());
    // Nothing holds a slot now, so the next start boots at once.
    let next = fixture.spawn_start("crash-b", &[]);
    until("the freed slot to admit a start", || fixture.runs("crash-b"));
    next.kill();
    assert_eq!(fixture.refusals(), Vec::<String>::new());
}

#[test]
fn a_vm_that_replaced_the_clone_of_a_record_is_never_stopped_and_counts_as_an_external_holder() {
    let _serial = serial();
    let mut fixture = Fixture::starved();
    let client = fixture.spawn_start("crash-c", &[]);
    until("the start to boot", || fixture.runs("crash-c"));
    until("the record to name the keeper", || {
        std::fs::read_to_string(fixture.start_record("crash-c"))
            .is_ok_and(|text| text.contains("keeper"))
    });
    let tart_run = fixture.tart_run_pid("crash-c").unwrap();

    fixture.crash();
    let (code, envelope, err) = client.finish();
    assert_ne!(code, 0, "{envelope} {err}");
    // A person stops and removes the VM with tart, and runs a new one under the same name.
    nix::sys::signal::kill(
        nix::unistd::Pid::from_raw(tart_run),
        nix::sys::signal::Signal::SIGKILL,
    )
    .unwrap();
    until("the old tart run to end", || !testanyware_vm::process::process_alive(tart_run));
    let person_s_vm = fixture.run_outside("crash-c", "darwin");

    fixture.restart();
    let status = fixture.status();

    let result = &status["adoption"]["unfinished_starts"][0];
    assert_eq!(result["id"], "crash-c", "{status}");
    assert_eq!(result["removed"], false, "{status}");
    let detail = result["detail"].as_str().unwrap();
    assert!(detail.contains("tart stop crash-c") && detail.contains("tart delete crash-c"), "{detail}");
    // The app signalled nothing and deleted nothing.
    assert!(testanyware_vm::process::process_alive(person_s_vm.id() as i32));
    assert!(fixture.cloned("crash-c"));
    assert!(!fixture.start_record("crash-c").exists());
    assert!(!sidecars(&fixture, "crash-c")[1].exists());
    // No sidecar names it, so `vm stop` finds no VM, and `vm slots` shows an external holder.
    let out = fixture.app.cli(&["vm", "stop", "crash-c", "--json"]);
    assert_eq!(out.json()["code"], "VM_NOT_FOUND", "{} {}", out.out(), out.err());
    assert!(testanyware_vm::process::process_alive(person_s_vm.id() as i32));
    let view = slots(&fixture);
    assert_eq!(holder(&view, "crash-c")["kind"], "external", "{view}");
    fixture.end_outside("crash-c", person_s_vm);
}

// ---- A start record that tart did not decide (spec "admission-queue": "A record that tart did
// not decide") ----

impl Fixture {
    /// A VM `id` that a start made, a crash of the app after the meta and before the removal of
    /// the record, and a person who replaces the VM with another one under the same name. The app
    /// is down afterwards, and the record of the start is there.
    fn crash_with_a_replaced_vm(&mut self, id: &str) -> Child {
        self.start_and_wait_until_running(id);
        let tart_run = self.tart_run_pid(id).unwrap();
        self.crash();
        nix::sys::signal::kill(
            nix::unistd::Pid::from_raw(tart_run),
            nix::sys::signal::Signal::SIGKILL,
        )
        .unwrap();
        until("the old tart run to end", || !testanyware_vm::process::process_alive(tart_run));
        let person_s_vm = self.run_outside(id, "darwin");
        // The start wrote its meta and the app died before it removed the record.
        std::fs::write(
            self.start_record(id),
            json!({ "id": id, "backend": "tart" }).to_string(),
        )
        .unwrap();
        person_s_vm
    }

    /// The next start of the app finds a `tart list` that does not answer.
    fn restart_while_tart_does_not_answer(&mut self) {
        self.set_fake("list-hang", "");
        self.restart();
    }

    /// The result of `id` in the adoption results of `GET /v1/status`.
    fn unfinished(&self, id: &str) -> Value {
        self.status()["adoption"]["unfinished_starts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|start| start["id"] == id)
            .cloned()
            .unwrap_or(Value::Null)
    }
}

#[test]
fn a_record_that_tart_did_not_decide_is_decided_when_the_tick_observes_and_the_running_vm_stays() {
    let _serial = serial();
    let mut fixture = Fixture::healthy();
    let person_s_vm = fixture.crash_with_a_replaced_vm("retry-a");
    fixture.restart_while_tart_does_not_answer();

    // The fourth row: the app keeps the VM and the record, and removes the sidecars that prove
    // nothing.
    let first = fixture.unfinished("retry-a");
    assert_eq!(first["removed"], false, "{first}");
    assert!(first["detail"].as_str().unwrap().contains("tart does not answer"), "{first}");
    assert!(fixture.start_record("retry-a").exists());
    assert!(!sidecars(&fixture, "retry-a")[1].exists());

    // No client asks for an observation. The tick makes it, and the app decides again.
    fixture.clear_fake("list-hang");
    until("the app to decide again", || {
        fixture.unfinished("retry-a")["detail"]
            .as_str()
            .is_some_and(|detail| detail.contains("tart reports a running VM"))
    });
    let result = fixture.unfinished("retry-a");
    assert_eq!(result["removed"], false, "{result}");
    assert!(!fixture.start_record("retry-a").exists());
    assert!(testanyware_vm::process::process_alive(person_s_vm.id() as i32));
    assert!(fixture.cloned("retry-a"));
    let out = fixture.app.cli(&["vm", "stop", "retry-a", "--json"]);
    assert_eq!(out.json()["code"], "VM_NOT_FOUND", "{} {}", out.out(), out.err());
    assert!(testanyware_vm::process::process_alive(person_s_vm.id() as i32));
    let view = slots(&fixture);
    assert_eq!(holder(&view, "retry-a")["kind"], "external", "{view}");

    // The VM stops later. No decision for the record is left, so the app removes nothing.
    nix::sys::signal::kill(
        nix::unistd::Pid::from_raw(person_s_vm.id() as i32),
        nix::sys::signal::Signal::SIGKILL,
    )
    .unwrap();
    std::thread::sleep(Duration::from_secs(4));
    assert!(fixture.cloned("retry-a"), "the app deletes no VM that it did not start");
    fixture.end_outside("retry-a", person_s_vm);
}

#[test]
fn a_list_that_answers_decides_the_record_although_each_later_call_to_tart_fails() {
    let _serial = serial();
    let mut fixture = Fixture::healthy();
    let person_s_vm = fixture.crash_with_a_replaced_vm("retry-b");
    fixture.restart_while_tart_does_not_answer();
    assert_eq!(fixture.unfinished("retry-b")["removed"], false);

    // The list answers and reports the VM running, and `tart get` fails: the observation fails.
    fixture.set_fake("get-fails", "");
    fixture.clear_fake("list-hang");
    until("the app to decide again", || !fixture.start_record("retry-b").exists());
    assert!(
        fixture.unfinished("retry-b")["detail"]
            .as_str()
            .unwrap()
            .contains("tart reports a running VM"),
        "{}",
        fixture.unfinished("retry-b")
    );

    // The VM stops, tart answers again, and the VM stays.
    nix::sys::signal::kill(
        nix::unistd::Pid::from_raw(person_s_vm.id() as i32),
        nix::sys::signal::Signal::SIGKILL,
    )
    .unwrap();
    fixture.clear_fake("get-fails");
    fixture.app.cli(&["vm", "slots", "--json"]);
    assert!(fixture.cloned("retry-b"), "the app deletes no VM that it did not start");
    assert!(!fixture.start_record("retry-b").exists());
    fixture.end_outside("retry-b", person_s_vm);
}

#[test]
fn a_start_of_the_same_id_joins_the_queue_while_the_record_stays_and_the_queue_serves_its_waiters() {
    let _serial = serial();
    let mut fixture = Fixture::starved();
    let client = fixture.spawn_start("retry-c", &[]);
    until("the start to boot", || fixture.runs("retry-c"));
    until("the record to name the keeper", || {
        std::fs::read_to_string(fixture.start_record("retry-c"))
            .is_ok_and(|text| text.contains("keeper"))
    });
    let tart_run = fixture.tart_run_pid("retry-c").unwrap();
    fixture.crash();
    let _ = client.finish();
    nix::sys::signal::kill(
        nix::unistd::Pid::from_raw(tart_run),
        nix::sys::signal::Signal::SIGKILL,
    )
    .unwrap();
    until("the tart run to end", || !testanyware_vm::process::process_alive(tart_run));

    fixture.restart_while_tart_does_not_answer();
    assert_eq!(fixture.unfinished("retry-c")["removed"], false);
    assert!(fixture.start_record("retry-c").exists(), "the record stays while tart is silent");

    // A start of the id takes the gate of the id, and its clone step waits for the tart that does
    // not answer. Two more starts follow it. The app never waits for the gate, so the record stays.
    let same = fixture.spawn_start("retry-c", &["--wait=60"]);
    until("the start of the id to replace the record", || {
        std::fs::read_to_string(fixture.start_record("retry-c"))
            .is_ok_and(|text| !text.contains("keeper"))
    });
    let second = fixture.spawn_start("retry-d", &["--wait=60"]);
    let third = fixture.spawn_start("retry-e", &["--wait=60"]);
    let view = slots(&fixture);
    assert!(view["queue"].is_array(), "`vm slots` answers: {view}");

    // tart answers. The record stays, because the start of the id holds its gate while it waits.
    fixture.clear_fake("list-hang");
    until("two starts to hold both slots and one to wait", || {
        slots(&fixture)["queue"].as_array().is_some_and(|queue| queue.len() == 1)
            && ["retry-c", "retry-d", "retry-e"]
                .iter()
                .filter(|id| fixture.runs(id))
                .count()
                == 2
    });
    assert_eq!(fixture.refusals(), Vec::<String>::new());
    assert!(fixture.start_record("retry-c").exists());
    assert!(
        fixture.unfinished("retry-c")["detail"].as_str().unwrap().contains("tart does not answer"),
        "no decision again while the start of the id holds the gate"
    );

    // A slot frees, and the queue serves the waiter that stands in its place.
    second.kill();
    until("the third start to boot", || fixture.runs("retry-e"));

    // The start of the id ends and takes its record along. The next tick finds that a start
    // replaced the record, and the app decides no more for the old one.
    same.kill();
    until("the app to note that a start replaced the record", || {
        fixture.unfinished("retry-c")["detail"]
            .as_str()
            .is_some_and(|detail| detail.contains("replaced the start record"))
    });
    assert!(!fixture.start_record("retry-c").exists());
    third.kill();
}
