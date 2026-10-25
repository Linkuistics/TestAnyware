//! The boot of a VM in a macOS golden creation, as the framework's refusal meets it (spec
//! "admission-queue": "Golden creation").
//!
//! Golden creation boots a VM several times: the base VM, the setup VM, two Recovery boots and the
//! normal boots after them. Each boot starts a detached `tart run`. Virtualization.framework can
//! refuse that boot when both macOS guest slots are in use, and `tart run` then exits within a
//! second. This module notices the exit, and it tries the boot again while the refusal lasts.
//! The framework can free the slot of the previous boot a moment after tart exits, so a refusal
//! straight after a boot is often short.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::error::{AdmissionFacts, LimitReached, VmError};
use crate::process::{child_alive, terminate};
use crate::start_outcome::{
    exit_detail, next_slot_step, parse_limit_refusal, read_log_tail, SlotWaitStep,
    MACOS_GUEST_LIMIT, SLOT_RETRY_INTERVAL,
};
use crate::tart::{await_vnc_url, run_detached_in, run_detached_recovery_in, Polled, VncUrl};
use crate::tools::ToolSearch;

/// How long a boot waits for its VNC line, and how it tries again after a refusal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BootBudget {
    /// The reads of the log of `tart run` that look for the VNC line.
    pub vnc_polls: u32,
    /// The pause between two reads. It is also the longest time a dead `tart run` goes unseen.
    pub vnc_interval: Duration,
    /// The pause between a refusal and the next try.
    pub retry_every: Duration,
    /// How long after the first refusal a try still happens.
    pub retry_within: Duration,
}

impl BootBudget {
    /// 60 s for the VNC line, and a try every 3 s for 30 s after a refusal.
    pub const DEFAULT: BootBudget = BootBudget {
        vnc_polls: 300,
        vnc_interval: Duration::from_millis(200),
        retry_every: SLOT_RETRY_INTERVAL,
        retry_within: Duration::from_secs(30),
    };
}

/// Which `tart run` a boot starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootMode {
    Normal,
    /// `tart run --recovery`: macOS Recovery, which has no SSH.
    Recovery,
}

/// A boot that printed its VNC line, so the framework accepted it.
#[derive(Debug, Clone)]
pub struct Booted {
    /// The detached `tart run`.
    pub pid: i32,
    pub log_path: PathBuf,
    pub vnc: VncUrl,
}

/// Start `tart run` for `id` and wait for its VNC line. Return when the line comes.
///
/// - **`tart run` exits with the refusal of the framework.** Try again after `retry_every`, until
///   `retry_within` after the first refusal. Then fail with `VM_LIMIT_REACHED` and `refused_by`
///   `framework`.
/// - **`tart run` exits for another reason.** Fail with `TART_FAILED` and the log, at once.
/// - **The line does not come.** Stop `tart run`, and fail with `VM_BOOT_TIMEOUT` at the VNC stage.
///
/// When it fails, no `tart run` of this call is left running. The caller removes the VM.
pub async fn boot_to_vnc(
    tools: &ToolSearch,
    id: &str,
    log_dir: &Path,
    mode: BootMode,
    budget: BootBudget,
) -> Result<Booted, VmError> {
    let mut first_refusal: Option<Instant> = None;
    let mut told = false;
    loop {
        let (pid, log_path) = match mode {
            BootMode::Normal => run_detached_in(tools, id, log_dir)?,
            BootMode::Recovery => run_detached_recovery_in(tools, id, log_dir)?,
        };
        match await_vnc_url(&log_path, budget.vnc_polls, budget.vnc_interval, || child_alive(pid))
            .await
        {
            Polled::Found(vnc) => return Ok(Booted { pid, log_path, vnc }),
            Polled::TimedOut { .. } => {
                terminate(pid, Duration::from_millis(200), 10);
                return Err(VmError::boot_timeout(id, crate::error::BootStage::Vnc));
            }
            Polled::ProcessExited => {}
        }

        let tail = read_log_tail(&log_path);
        let Some(refusal) = parse_limit_refusal(&tail) else {
            return Err(VmError::TartFailed { detail: exit_detail(&tail) });
        };
        let since = *first_refusal.get_or_insert_with(Instant::now);
        match next_slot_step(since.elapsed(), budget.retry_within, budget.retry_every) {
            SlotWaitStep::RetryAfter(pause) => {
                if !told {
                    told = true;
                    eprintln!(
                        "The framework refused the boot of '{id}': the macOS guest limit is full. \
                         Trying again every {}s for {}s.",
                        budget.retry_every.as_secs(),
                        budget.retry_within.as_secs(),
                    );
                }
                tokio::time::sleep(pause).await;
            }
            SlotWaitStep::GiveUp => {
                return Err(VmError::VmLimitReached(Box::new(LimitReached {
                    platform: "macos".to_string(),
                    limit: MACOS_GUEST_LIMIT,
                    holders: refusal.holders,
                    raw: refusal.raw,
                    // The default says that the framework refused, and that no app took part.
                    admission: AdmissionFacts::default(),
                })));
            }
        }
    }
}
