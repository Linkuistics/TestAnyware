//! The boot of a macOS golden creation against a fake `tart`: it notices that `tart run` exited,
//! and it tries again while the framework refuses (spec "admission-queue": "Golden creation").
//! The tart backend is macOS-only, so the file is too.
//!
//! The fake `tart` (`fixtures/fake-tart`) refuses a `run` as the framework does: with `capacity`,
//! while that many guests run, and with `full`, always. Each refusal appends a line to
//! `refused.log`. The tests shorten the pauses of the budget, because a pause of 3 s for 30 s is
//! too long for a test. One test pins the real values.

#![cfg(target_os = "macos")]

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use testanyware_vm::error::RefusedBy;
use testanyware_vm::golden_boot::{boot_to_vnc, BootBudget, BootMode};
use testanyware_vm::tools::ToolSearch;
use testanyware_vm::VmError;

const FAKE_TART: &str = include_str!("fixtures/fake-tart");

/// A directory with the fake `tart`, and the log directory of the boots.
struct Fixture {
    dir: tempfile::TempDir,
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
        Self { dir }
    }

    fn tools(&self) -> ToolSearch {
        ToolSearch::new(vec![self.dir.path().join("tools")])
    }

    fn state(&self) -> PathBuf {
        self.dir.path().join("tools").join("state")
    }

    fn logs(&self) -> PathBuf {
        self.dir.path().join("logs")
    }

    fn refusals(&self) -> usize {
        std::fs::read_to_string(self.state().join("refused.log"))
            .map(|log| log.lines().count())
            .unwrap_or(0)
    }

    async fn boot(&self, mode: BootMode, budget: BootBudget) -> Result<(), VmError> {
        let booted = boot_to_vnc(&self.tools(), "setup-vm", &self.logs(), mode, budget).await?;
        // The test does not need the guest. Stop its `sleep`.
        testanyware_vm::process::terminate(booted.pid, Duration::from_millis(50), 20);
        Ok(())
    }
}

/// A budget with the real VNC wait and the pauses of a test.
fn quick(retry_within: Duration) -> BootBudget {
    BootBudget {
        retry_every: Duration::from_millis(100),
        retry_within,
        ..BootBudget::DEFAULT
    }
}

/// A guest that holds a slot for the fake: a `sleep` that the fake counts.
fn start_holder(state: &Path, name: &str) -> std::process::Child {
    std::fs::create_dir_all(state).unwrap();
    let child = std::process::Command::new("sleep").arg("60").spawn().unwrap();
    std::fs::write(state.join(format!("{name}.running")), child.id().to_string()).unwrap();
    child
}

#[test]
fn the_budget_is_3_s_for_30_s_and_60_s_for_the_vnc_line() {
    let budget = BootBudget::DEFAULT;
    assert_eq!(budget.retry_every, Duration::from_secs(3));
    assert_eq!(budget.retry_within, Duration::from_secs(30));
    assert_eq!(budget.vnc_interval * budget.vnc_polls, Duration::from_secs(60));
}

#[tokio::test]
async fn a_boot_with_a_free_slot_returns_at_the_vnc_line() {
    let fixture = Fixture::new();
    fixture.boot(BootMode::Normal, quick(Duration::from_secs(5))).await.unwrap();
    assert_eq!(fixture.refusals(), 0);
}

#[tokio::test]
async fn a_refusal_that_clears_within_the_window_ends_in_a_boot() {
    let fixture = Fixture::new();
    let state = fixture.state();
    std::fs::create_dir_all(&state).unwrap();
    std::fs::write(state.join("capacity"), "1").unwrap();
    let mut holder = start_holder(&state, "holder");
    // The holder stops a moment after the first refusal, as the framework frees a slot.
    let freeing = tokio::spawn({
        let state = state.clone();
        async move {
            tokio::time::sleep(Duration::from_millis(700)).await;
            let _ = holder.kill();
            let _ = holder.wait();
            std::fs::remove_file(state.join("holder.running")).unwrap();
        }
    });

    let result = fixture.boot(BootMode::Normal, quick(Duration::from_secs(10))).await;
    freeing.await.unwrap();

    result.expect("the boot must succeed once the slot is free");
    assert!(fixture.refusals() >= 2, "the boot must have been refused and tried again");
}

#[tokio::test]
async fn a_refusal_that_lasts_fails_with_the_limit_after_the_window_and_leaves_nothing_running() {
    let fixture = Fixture::new();
    std::fs::create_dir_all(fixture.state()).unwrap();
    std::fs::write(fixture.state().join("full"), "").unwrap();

    let started = Instant::now();
    let error = fixture
        .boot(BootMode::Normal, quick(Duration::from_millis(600)))
        .await
        .expect_err("a refusal that lasts must fail");

    let VmError::VmLimitReached(limit) = &error else {
        panic!("expected VM_LIMIT_REACHED, got {error:?}");
    };
    assert_eq!(error.code(), "VM_LIMIT_REACHED");
    assert_eq!(limit.admission.refused_by, RefusedBy::Framework);
    assert_eq!(limit.holders, vec!["holder-a", "holder-b"]);
    assert!(limit.raw.contains("exceeds the system limit"), "{}", limit.raw);
    assert!(
        started.elapsed() >= Duration::from_millis(600),
        "the boot must keep trying for the whole window"
    );
    assert!(started.elapsed() < Duration::from_secs(10));
    assert!(fixture.refusals() >= 3, "the boot must try again while the window lasts");
    assert!(!fixture.state().join("setup-vm.running").exists());
}

#[tokio::test]
async fn an_exit_is_noticed_and_not_waited_out_for_the_vnc_line() {
    let fixture = Fixture::new();
    std::fs::create_dir_all(fixture.state()).unwrap();
    std::fs::write(fixture.state().join("full"), "").unwrap();

    // The default VNC wait is 60 s. A window of 0 s gives the verdict at the first refusal.
    let budget = BootBudget { retry_within: Duration::ZERO, ..BootBudget::DEFAULT };
    let started = Instant::now();
    let error = fixture.boot(BootMode::Normal, budget).await.unwrap_err();

    assert_eq!(error.code(), "VM_LIMIT_REACHED");
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "took {:?}, which is a poll of the log and not a notice of the exit",
        started.elapsed()
    );
    assert_eq!(fixture.refusals(), 1);
}

#[tokio::test]
async fn a_failure_of_tart_that_is_no_refusal_ends_at_once_with_its_words() {
    let fixture = Fixture::new();
    std::fs::create_dir_all(fixture.state()).unwrap();
    std::fs::write(fixture.state().join("run-fails"), "").unwrap();

    let started = Instant::now();
    let error = fixture.boot(BootMode::Normal, BootBudget::DEFAULT).await.unwrap_err();

    assert_eq!(error.code(), "TART_FAILED");
    assert!(error.to_string().contains("tart: boom"), "{error}");
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[tokio::test]
async fn a_recovery_boot_meets_the_refusal_in_the_same_way() {
    let fixture = Fixture::new();
    std::fs::create_dir_all(fixture.state()).unwrap();
    std::fs::write(fixture.state().join("full"), "").unwrap();

    let error = fixture
        .boot(BootMode::Recovery, quick(Duration::from_millis(300)))
        .await
        .unwrap_err();

    assert_eq!(error.code(), "VM_LIMIT_REACHED");
    assert!(fixture.refusals() >= 2);
}
