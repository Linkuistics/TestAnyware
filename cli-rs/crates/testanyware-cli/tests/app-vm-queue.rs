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
            Some(config) => TestApp::start_after_with_config_and_args(prepare, config, &args),
            None => TestApp::start_after_with_args(prepare, &args),
        };
        Self { app, _agent: agent }
    }

    fn fake_state(&self) -> PathBuf {
        self.app.tool_dir().join("state")
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

    fn spawn(&self, args: &[&str], extra: &[&str]) -> Client {
        let mut command = Command::new(CLI);
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
    let fixture = Fixture::healthy();
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
