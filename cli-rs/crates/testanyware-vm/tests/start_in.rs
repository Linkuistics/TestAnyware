//! `VmLifecycle::start_in` against a fake `tart`: what a start does when its caller goes away,
//! and the lines it reports. The tart backend is macOS-only, so the file is too.
//!
//! The fake `tart` (`fixtures/fake-tart`) keeps a clone as a file and a running VM as a
//! `sleep` process. Its agent port is a port that nothing listens on, so a start stays in its
//! agent wait until the test cancels it.

#![cfg(target_os = "macos")]

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use testanyware_vm::display_request::parse_display_request;
use testanyware_vm::lifecycle::{Platform, VmLifecycle, VmStartOptions};
use testanyware_vm::tools::ToolSearch;
use testanyware_vm::{Progress, StartContext, VmError, VmPaths};
use tokio_util::sync::CancellationToken;

const FAKE_TART: &str = include_str!("fixtures/fake-tart");
const BASE: &str = "testanyware-golden-linux-24.04";

/// A directory with the fake `tart`, and the paths of one app.
struct Fixture {
    dir: tempfile::TempDir,
    lines: Arc<Mutex<Vec<String>>>,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let tools = dir.path().join("tools");
        std::fs::create_dir_all(&tools).unwrap();
        let tart = tools.join("tart");
        std::fs::write(&tart, FAKE_TART).unwrap();
        std::fs::set_permissions(&tart, std::os::unix::fs::PermissionsExt::from_mode(0o755))
            .unwrap();
        Self {
            dir,
            lines: Arc::default(),
        }
    }

    fn state(&self) -> PathBuf {
        self.dir.path().join("tools").join("state")
    }

    fn paths(&self) -> VmPaths {
        VmPaths::new(
            self.dir.path().join("state"),
            self.dir.path().join("data"),
            self.dir.path().join("tmp"),
        )
    }

    fn context(&self, cancel: CancellationToken, agent_port: u16) -> StartContext {
        let lines = Arc::clone(&self.lines);
        StartContext::for_app(
            self.paths(),
            ToolSearch::new(vec![self.dir.path().join("tools")]),
            Progress::to(move |line| lines.lock().unwrap().push(line.to_string())),
            cancel,
        )
        .with_agent_port(agent_port)
    }

    fn lines(&self) -> Vec<String> {
        self.lines.lock().unwrap().clone()
    }
}

fn options(id: &str) -> VmStartOptions {
    let display = parse_display_request(None).unwrap();
    VmStartOptions::new(
        Platform::Linux,
        Some(BASE.into()),
        Some(id.into()),
        display,
        false,
    )
}

/// A port with nothing behind it, so each agent probe is refused.
fn closed_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

async fn until(what: &str, mut ready: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while !ready() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

fn pid_alive(pid: i32) -> bool {
    testanyware_vm::process::process_alive(pid)
}

fn running_pid(state: &Path, id: &str) -> Option<i32> {
    std::fs::read_to_string(state.join(format!("{id}.running")))
        .ok()?
        .trim()
        .parse()
        .ok()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_client_that_leaves_during_the_agent_wait_cancels_the_start_and_cleans_up() {
    let fixture = Fixture::new();
    let cancel = CancellationToken::new();
    let ctx = fixture.context(cancel.clone(), closed_port());
    let start = tokio::spawn({
        let ctx = ctx.clone();
        async move { VmLifecycle::start_in(&options("cancel1"), &ctx).await }
    });

    // The VM runs, and the start waits for an agent that never answers.
    let state = fixture.state();
    until("tart run", || {
        running_pid(&state, "cancel1").is_some_and(pid_alive)
    })
    .await;
    let tart_pid = running_pid(&state, "cancel1").unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(
        !start.is_finished(),
        "the start should still wait for its agent"
    );

    cancel.cancel();
    let result = tokio::time::timeout(Duration::from_secs(20), start)
        .await
        .expect("the cancel ends the start")
        .unwrap();

    let err = result.expect_err("a cancelled start fails");
    assert!(matches!(err, VmError::StartCancelled { .. }), "{err}");
    assert!(
        !state.join("cancel1.cloned").exists(),
        "the clone is deleted"
    );
    // The reaper has waited for the process, so `kill -0` finds nothing: no zombie.
    until("tart run to be gone", || !pid_alive(tart_pid)).await;
    let paths = fixture.paths();
    assert!(!paths.spec_path("cancel1").exists());
    assert!(!paths.meta_path("cancel1").exists());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_client_that_leaves_during_a_slot_wait_ends_the_wait_and_removes_the_clone() {
    let fixture = Fixture::new();
    std::fs::create_dir_all(fixture.state()).unwrap();
    std::fs::write(fixture.state().join("full"), "").unwrap();
    let cancel = CancellationToken::new();
    let ctx = fixture.context(cancel.clone(), closed_port());
    let mut opts = options("slot1");
    opts.wait = Some(Duration::from_secs(600));
    let start = tokio::spawn({
        let ctx = ctx.clone();
        async move { VmLifecycle::start_in(&opts, &ctx).await }
    });

    // The wait names its holders, so it is in the retry loop with a clone.
    until("the wait to name its holders", || {
        fixture
            .lines()
            .iter()
            .any(|l| l.contains("holder-a, holder-b"))
    })
    .await;
    assert!(
        fixture.state().join("slot1.cloned").exists(),
        "the wait holds one clone"
    );

    cancel.cancel();
    let err = tokio::time::timeout(Duration::from_secs(20), start)
        .await
        .expect("the cancel ends the wait")
        .unwrap()
        .expect_err("a cancelled wait fails");
    assert!(matches!(err, VmError::StartCancelled { .. }), "{err}");
    assert!(
        !fixture.state().join("slot1.cloned").exists(),
        "the clone is deleted"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_cancel_that_arrives_before_the_boot_removes_the_clone_without_a_signal_handler() {
    // The client left during the clone, which runs to its end. The start reads the cancel
    // after it and never runs `tart run`. The app's context also has no signal policy: a
    // SIGTERM handler in the app would turn a signal into the end of the app.
    let fixture = Fixture::new();
    let cancel = CancellationToken::new();
    cancel.cancel();
    let ctx = fixture.context(cancel, closed_port());
    assert!(!ctx.process_signals);
    let mut opts = options("early1");
    opts.wait = Some(Duration::from_secs(5));
    let err = VmLifecycle::start_in(&opts, &ctx)
        .await
        .expect_err("cancelled");
    assert!(matches!(err, VmError::StartCancelled { .. }), "{err}");
    assert!(
        !fixture.state().join("early1.cloned").exists(),
        "the clone is deleted"
    );
    assert!(
        !fixture.state().join("early1.running").exists(),
        "tart run never ran"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_cancel_whose_teardown_fails_keeps_the_handle_that_vm_stop_needs() {
    let fixture = Fixture::new();
    std::fs::create_dir_all(fixture.state()).unwrap();
    std::fs::write(fixture.state().join("stuck"), "").unwrap();
    let cancel = CancellationToken::new();
    let ctx = fixture.context(cancel.clone(), closed_port());
    let start = tokio::spawn({
        let ctx = ctx.clone();
        async move { VmLifecycle::start_in(&options("stuck1"), &ctx).await }
    });
    let state = fixture.state();
    until("tart run", || {
        running_pid(&state, "stuck1").is_some_and(pid_alive)
    })
    .await;
    let tart_pid = running_pid(&state, "stuck1").unwrap();

    cancel.cancel();
    let err = tokio::time::timeout(Duration::from_secs(30), start)
        .await
        .expect("the cancel ends the start")
        .unwrap()
        .expect_err("a cancelled start fails");
    assert!(matches!(err, VmError::StartCancelled { .. }), "{err}");

    // The clone survived the failed teardown, so the meta stays for `vm stop` to retry with.
    assert!(state.join("stuck1.cloned").exists());
    let meta = fixture.paths().meta_path("stuck1");
    assert!(meta.exists(), "the VM survived, and its handle was dropped");

    // Clean up what the fake left running.
    let _ = std::fs::remove_file(state.join("stuck"));
    let tools = ToolSearch::new(vec![fixture.dir.path().join("tools")]);
    VmLifecycle::stop_with("stuck1", &fixture.paths(), &tools).unwrap();
    until("tart run to be gone", || !pid_alive(tart_pid)).await;
    assert!(!state.join("stuck1.cloned").exists());
}
