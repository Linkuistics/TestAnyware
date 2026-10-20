//! `vm start` through the TestAnyware app (spec: "The protocol", "Lifecycle and adoption").
//!
//! The real Host CLI binary runs against the real app executable in test mode. `tart` is a
//! fake script (`testanyware-vm/tests/fixtures/fake-tart`): a clone is a file, and a running
//! VM is a `sleep` process. The agent is a wiremock server on loopback, and the app probes it
//! because `--test-agent-port` moves the agent port of a start. A plain listener stands for the
//! VNC port that the keeper holds.
//!
//! The tart backend exists on a macOS host only, and so does the direct path's absence: a macOS
//! host has no direct `vm start` to compare with. The expected output is written out below.

#![cfg(target_os = "macos")]

mod common;

use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use common::{app_binary, serial, FakeAgent, OutputExt, TestApp, CLI};
use serde_json::{json, Value};
use wiremock::ResponseTemplate;

const FAKE_TART: &str = include_str!("../../testanyware-vm/tests/fixtures/fake-tart");
const BASE: &str = "testanyware-golden-linux-24.04";

/// The fakes, then the app. The fakes start first and the app spawns once (see `common`).
struct Fixture {
    app: TestApp,
    agent: FakeAgent,
    vnc_port: u16,
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
    /// An app whose starts probe a healthy fake agent.
    fn healthy() -> Self {
        let agent = FakeAgent::start();
        agent.mount(
            "GET",
            "/health",
            ResponseTemplate::new(200)
                .set_body_json(json!({ "accessible": true, "platform": "linux" })),
        );
        let port = agent.addr().port();
        Self::with_agent_port(agent, port)
    }

    /// An app whose starts probe a port that refuses, so a start stays in its agent wait.
    fn starved() -> Self {
        Self::with_agent_port(FakeAgent::start(), closed_port())
    }

    fn with_agent_port(agent: FakeAgent, agent_port: u16) -> Self {
        let vnc_port = silent_vnc_server();
        let app = TestApp::start_after_with_args(
            |app| {
                let tart = app.tool_dir().join("tart");
                std::fs::write(&tart, FAKE_TART).unwrap();
                std::fs::set_permissions(
                    &tart,
                    std::os::unix::fs::PermissionsExt::from_mode(0o755),
                )
                .unwrap();
                let state = app.tool_dir().join("state");
                std::fs::create_dir_all(&state).unwrap();
                std::fs::write(state.join("vnc-port"), vnc_port.to_string()).unwrap();
            },
            &["--test-agent-port".to_string(), agent_port.to_string()],
        );
        Self {
            app,
            agent,
            vnc_port,
        }
    }

    fn fake_state(&self) -> PathBuf {
        self.app.tool_dir().join("state")
    }

    fn start(&self, id: &str, extra: &[&str]) -> std::process::Output {
        let mut args = vec![
            "vm",
            "start",
            "--platform",
            "linux",
            "--base",
            BASE,
            "--id",
            id,
        ];
        args.extend_from_slice(extra);
        self.app.cli(&args)
    }

    fn tart_run_pid(&self, id: &str) -> Option<i32> {
        std::fs::read_to_string(self.fake_state().join(format!("{id}.running")))
            .ok()?
            .trim()
            .parse()
            .ok()
    }

    fn sidecars(&self, id: &str) -> [PathBuf; 2] {
        let vms = self.app.state_dir().join("vms");
        [
            vms.join(format!("{id}.json")),
            vms.join(format!("{id}.meta.json")),
        ]
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

fn until(what: &str, mut ready: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while !ready() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn alive(pid: i32) -> bool {
    testanyware_vm::process::process_alive(pid)
}

/// The command line and the state of `pid`, as `ps` prints them.
fn ps(pid: u32, column: &str) -> String {
    let out = Command::new("ps")
        .args(["-o", &format!("{column}="), "-p", &pid.to_string()])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// The kernel addresses of the Unix-domain sockets that `pid` holds open. The same socket has
/// the same address in every process that holds it, so a socket that a child inherited from the
/// app shows up in both sets.
fn unix_socket_addresses(pid: u32) -> std::collections::HashSet<String> {
    let out = Command::new("lsof")
        .args(["-a", "-U", "-p", &pid.to_string(), "-Fd"])
        .output()
        .expect("run lsof");
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| l.strip_prefix('d'))
        .map(str::to_string)
        .collect()
}

/// The pids of the zombies among the children of `parent`.
fn zombie_children_of(parent: u32) -> Vec<u32> {
    let out = Command::new("ps")
        .args(["-A", "-o", "pid=,ppid=,stat="])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let pid: u32 = fields.next()?.parse().ok()?;
            let ppid: u32 = fields.next()?.parse().ok()?;
            let stat = fields.next()?;
            (ppid == parent && stat.starts_with('Z')).then_some(pid)
        })
        .collect()
}

#[test]
fn a_start_prints_the_json_of_the_direct_command_and_leaves_the_vm_running() {
    let _serial = serial();
    let fixture = Fixture::healthy();
    let out = fixture.start("start-1", &["--json"]);
    assert_eq!(out.code(), 0, "stdout {} stderr {}", out.out(), out.err());
    assert_eq!(
        out.err(),
        "",
        "a start with nothing to say prints nothing on stderr"
    );

    let envelope = out.json();
    assert_eq!(envelope["ok"], true);
    assert_eq!(envelope["schema_version"], "1.0");
    assert_eq!(envelope["id"], "start-1");
    assert_eq!(envelope["platform"], "linux");
    assert_eq!(envelope["base"], BASE);
    assert_eq!(
        envelope["vnc"],
        json!({ "host": "127.0.0.1", "port": fixture.vnc_port })
    );
    assert_eq!(
        envelope["agent"],
        json!({ "host": "127.0.0.1", "port": fixture.agent.addr().port() })
    );
    let [spec, meta] = fixture.sidecars("start-1");
    assert_eq!(envelope["spec_path"], spec.display().to_string());
    assert_eq!(envelope["meta_path"], meta.display().to_string());
    assert!(
        envelope.get("tpm").is_none(),
        "a tart start has no TPM decision: {envelope}"
    );
    assert!(spec.is_file() && meta.is_file());

    // The VM outlives the call: its `tart run` is still there, and the VM is listed.
    let tart_pid = fixture.tart_run_pid("start-1").expect("tart run ran");
    assert!(alive(tart_pid));
    let listing = fixture.app.cli(&["vm", "list", "--json"]).json();
    assert!(
        listing["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["name"] == "start-1"),
        "{listing}"
    );
}

#[test]
fn a_start_prints_only_the_id_in_text_mode() {
    let _serial = serial();
    let fixture = Fixture::healthy();
    let out = fixture.start("start-2", &[]);
    assert_eq!(out.code(), 0, "{}", out.err());
    assert_eq!(out.out(), "start-2\n");
    assert_eq!(out.err(), "");
}

#[test]
fn the_keeper_runs_from_the_apps_executable_and_the_detached_children_hold_no_app_socket() {
    let _serial = serial();
    let fixture = Fixture::healthy();
    let out = fixture.start("start-3", &["--json"]);
    assert_eq!(out.code(), 0, "{}", out.err());

    let [_, meta_path] = fixture.sidecars("start-3");
    let meta: Value = serde_json::from_slice(&std::fs::read(meta_path).unwrap()).unwrap();
    let keeper = meta["keeper_pid"]
        .as_u64()
        .expect("the meta names the keeper") as u32;
    let tart = fixture.tart_run_pid("start-3").unwrap() as u32;

    let command = ps(keeper, "command");
    assert!(
        command.starts_with(&app_binary().display().to_string()),
        "the keeper runs from the app's own executable: {command}"
    );
    assert!(command.contains("_vnc-keeper --id start-3"), "{command}");
    // The keeper holds its connection to the VNC port: the app's executable ran the command.
    until("the keeper to be in its session", || {
        nix_sid(keeper) == keeper as i32 && nix_sid(tart) == tart as i32
    });

    // The control: the app holds Unix sockets (its listener, and the connection of the start
    // that just ended), so the instrument finds sockets where there are some. A child may hold
    // sockets of its own, such as the pair of its signal driver, and those are not the app's.
    let app = unix_socket_addresses(fixture.app.pid());
    assert!(!app.is_empty(), "the instrument finds no socket of the app");
    for (name, pid) in [("tart run", tart), ("the keeper", keeper)] {
        let shared: Vec<_> = unix_socket_addresses(pid)
            .intersection(&app)
            .cloned()
            .collect();
        assert_eq!(
            shared,
            Vec::<String>::new(),
            "{name} holds a socket of the app"
        );
    }
}

fn nix_sid(pid: u32) -> i32 {
    nix::unistd::getsid(Some(nix::unistd::Pid::from_raw(pid as i32)))
        .map(|sid| sid.as_raw())
        .unwrap_or(-1)
}

#[test]
fn a_child_of_the_app_that_exits_is_reaped() {
    let _serial = serial();
    let fixture = Fixture::healthy();
    let out = fixture.start("start-4", &["--json"]);
    assert_eq!(out.code(), 0, "{}", out.err());
    let tart = fixture.tart_run_pid("start-4").unwrap();

    // `tart run` dies on its own, and the app never calls `vm stop`.
    nix::sys::signal::kill(
        nix::unistd::Pid::from_raw(tart),
        nix::sys::signal::Signal::SIGKILL,
    )
    .unwrap();
    until("tart run to be gone from the process table", || {
        ps(tart as u32, "stat").is_empty()
    });
    assert_eq!(zombie_children_of(fixture.app.pid()), Vec::<u32>::new());
}

#[test]
fn a_client_that_goes_away_during_the_start_cancels_it_and_the_app_cleans_up() {
    let _serial = serial();
    let fixture = Fixture::starved();
    let mut cli = Command::new(CLI)
        .args([
            "vm",
            "start",
            "--platform",
            "linux",
            "--base",
            BASE,
            "--id",
            "gone-1",
            "--json",
        ])
        .env_clear()
        .env("HOME", fixture.app.root())
        .env("XDG_STATE_HOME", fixture.app.xdg_state_home())
        .env("XDG_DATA_HOME", fixture.app.xdg_data_home())
        .env("TMPDIR", fixture.app.tmp_dir())
        .env("TESTANYWARE_APP_SOCKET", fixture.app.socket())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    // The VM runs, and the start waits for an agent that never answers.
    until("tart run", || {
        fixture.tart_run_pid("gone-1").is_some_and(alive)
    });
    let tart = fixture.tart_run_pid("gone-1").unwrap();
    assert!(fixture.fake_state().join("gone-1.cloned").exists());

    // The CLI dies, as it does under Ctrl-C or a timeout of a harness.
    cli.kill().unwrap();
    cli.wait().unwrap();

    until("the app to remove the clone", || {
        !fixture.fake_state().join("gone-1.cloned").exists()
    });
    until("tart run to end", || !alive(tart));
    let [spec, meta] = fixture.sidecars("gone-1");
    assert!(!spec.exists() && !meta.exists(), "the sidecars are gone");
    assert_eq!(zombie_children_of(fixture.app.pid()), Vec::<u32>::new());
    // The app serves on, for the next client.
    assert_eq!(fixture.app.cli(&["vm", "list", "--json"]).code(), 0);
}

#[test]
fn a_start_that_fails_prints_the_error_of_the_direct_command() {
    let _serial = serial();
    let fixture = Fixture::healthy();
    let out = fixture.app.cli(&[
        "vm",
        "start",
        "--platform",
        "macos",
        "--base",
        "no-such-golden",
        "--id",
        "bad-1",
        "--json",
    ]);
    assert_eq!(out.code(), 1, "stdout {} stderr {}", out.out(), out.err());
    let envelope = out.json();
    assert_eq!(envelope["ok"], false);
    assert_eq!(envelope["code"], "TART_FAILED");
    assert!(
        envelope["message"]
            .as_str()
            .unwrap()
            .starts_with("tart failed: tart clone no-such-golden bad-1"),
        "{envelope}"
    );
    assert!(envelope["details"]["tart_error"]
        .as_str()
        .unwrap()
        .contains("no such VM"));
    // The error of the direct command has no remedy, and the app adds none.
    assert!(envelope.get("remediation").is_none(), "{envelope}");

    let text = fixture.app.cli(&[
        "vm",
        "start",
        "--platform",
        "macos",
        "--base",
        "no-such-golden",
        "--id",
        "bad-2",
    ]);
    assert_eq!(text.code(), 1);
    assert_eq!(text.out(), "");
    assert!(
        text.err()
            .starts_with("error: tart failed: tart clone no-such-golden bad-2"),
        "{}",
        text.err()
    );
}

#[test]
fn a_refusal_by_the_guest_limit_has_the_exit_code_and_details_of_the_direct_command() {
    let _serial = serial();
    let fixture = Fixture::healthy();
    std::fs::write(fixture.fake_state().join("full"), "").unwrap();
    let out = fixture.start("limit-1", &["--json"]);
    assert_eq!(out.code(), 6, "stdout {} stderr {}", out.out(), out.err());
    let envelope = out.json();
    assert_eq!(envelope["code"], "VM_LIMIT_REACHED");
    assert_eq!(
        envelope["details"]["holders"],
        json!(["holder-a", "holder-b"])
    );
    assert!(
        envelope["remediation"]
            .as_str()
            .unwrap()
            .contains("vm stop"),
        "{envelope}"
    );
    assert!(
        !fixture.fake_state().join("limit-1.cloned").exists(),
        "the refusal leaves no clone"
    );
}

#[test]
fn a_wait_for_a_slot_reports_on_stderr_while_it_runs_and_then_fails_like_a_start_that_never_waited()
{
    let _serial = serial();
    let fixture = Fixture::healthy();
    std::fs::write(fixture.fake_state().join("full"), "").unwrap();
    let began = Instant::now();
    let out = fixture.start("wait-1", &["--wait=2", "--json"]);
    assert_eq!(out.code(), 6, "stdout {} stderr {}", out.out(), out.err());
    assert!(
        began.elapsed() >= Duration::from_secs(2),
        "it waited its two seconds"
    );
    assert_eq!(out.json()["code"], "VM_LIMIT_REACHED");
    // The progress line comes through the app, on stderr, and `--json` keeps stdout to the envelope.
    assert!(out.err().contains("waiting for a"), "{}", out.err());
    assert!(out.err().contains("holder-a, holder-b"), "{}", out.err());
    assert!(!fixture.fake_state().join("wait-1.cloned").exists());
}

#[test]
fn an_id_that_could_name_another_file_is_refused_before_anything_runs() {
    let _serial = serial();
    let fixture = Fixture::healthy();
    let out = fixture.start("../escape", &["--json"]);
    assert_eq!(out.code(), 2, "stdout {} stderr {}", out.out(), out.err());
    assert_eq!(out.json()["code"], "USAGE_ERROR");
    assert!(
        std::fs::read_dir(fixture.fake_state())
            .unwrap()
            .flatten()
            .all(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                !name.ends_with(".cloned")
            }),
        "no clone was made"
    );
}

#[test]
fn without_the_app_a_start_fails_and_names_the_remedy() {
    let _serial = serial();
    let app = TestApp::not_running();
    let out = app.cli(&[
        "vm",
        "start",
        "--platform",
        "linux",
        "--id",
        "x-1",
        "--json",
    ]);
    assert_eq!(out.code(), 1, "{}", out.out());
    let envelope = out.json();
    assert_eq!(envelope["code"], "APP_UNREACHABLE");
    assert_eq!(envelope["details"]["cause"], "not_running");
}
