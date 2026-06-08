//! In-process macOS Recovery automation (ADR-0008) — **macOS-host only**.
//!
//! `csrutil` can only toggle SIP from macOS Recovery, and recovery has no
//! SSH, so this is the one part of golden creation that cannot ride the
//! `russh` provisioning layer (ADR-0007): it must drive the recovery GUI's
//! VNC framebuffer directly. [`recovery_boot_csrutil`] boots the setup VM into
//! Recovery over VNC, navigates the startup picker, opens Terminal via the
//! Utilities menu, runs a `csrutil` command (answering its prompts), halts,
//! and reboots normally back to an SSH-reachable state.
//!
//! Re-engineers `provisioner/scripts/vm-create-golden-macos.sh`'s
//! `_recovery_boot_csrutil` (lines 398–594): the **macro sequence is at
//! parity** (boot recovery → picker → Terminal → csrutil → halt → reboot), but
//! every blind `sleep N` is replaced by a **signal-driven wait** built from two
//! independent primitives on [`RecoverySession`]:
//!
//!   - [`RecoverySession::wait_for_text`] — OCR the framebuffer until a query
//!     appears (large readable UI text: the picker's "Options", the menu bar's
//!     "Utilities"/"Terminal", the csrutil result line). Returns the matched
//!     detection's centre for click targeting.
//!   - [`RecoverySession::settle`] — wait until the framebuffer stops changing
//!     for a quiet window. The proxy for the signal-less micro-gaps (the
//!     csrutil prompts on small recovery-Terminal monospace, where OCR is
//!     least reliable, and the masked password entry that echoes nothing).
//!
//! The two are kept **independent** (ADR-0008 Q2) so which one is *primary* at
//! a given prompt can flip during live tuning. The authoritative correctness
//! gate is **not** these in-recovery waits but the post-reboot `csrutil status`
//! check over SSH (the `020` finalize leaf / the live-VM verification below),
//! so the waits need only be good enough to reach the result, not perfect.
//!
//! VNC keysym quirk (memory `cmd-key-tahoe`): Command = `XK_Alt_L` on the
//! Virtualization.framework VNC path. It does not bite here — recovery opens
//! Terminal via a menu *drag* precisely to avoid Cmd-T (a modal swallows it) —
//! but [`Platform::Macos`] is threaded through input so any future modifier use
//! stays correct.

use std::time::{Duration, Instant};

use testanyware_ocr_client::{find_text, FindOutcome, OcrDetection, OcrEngine};
use testanyware_rfb::{InputError, Platform, RfbConnection, RfbError};
use tokio::io::BufReader;
use tokio::net::TcpStream;

use crate::capture::{capture_frame, capture_frame_png, CaptureError};
use crate::error::VmError;
use crate::golden::{SetupVm, VANILLA_PASS, VANILLA_USER};
use crate::paths::VmPaths;
use crate::ssh::SshSession;

// ---- tunables (live-iteration knobs) ------------------------------------
//
// These deadlines mirror the script's blind sleeps' *intent* (how long a
// transition may reasonably take) but bound a polling wait rather than a fixed
// pause: a fast transition returns early, a slow one is still caught. They are
// the first thing to adjust during live verification.

/// How long to wait for the recovery VNC framebuffer to start updating.
const FB_LIVE_WAIT: Duration = Duration::from_secs(90);
/// Startup-disk picker ("Options") — a cold recovery boot can be slow.
const PICKER_WAIT: Duration = Duration::from_secs(120);
/// Recovery desktop ("Utilities" in the menu bar).
const DESKTOP_WAIT: Duration = Duration::from_secs(120);
/// "Terminal" item, polled while the Utilities dropdown is open.
const MENU_ITEM_WAIT: Duration = Duration::from_secs(15);
/// Per-csrutil-prompt OCR attempt before falling back to [`settle`].
const PROMPT_OCR_WAIT: Duration = Duration::from_secs(20);
/// csrutil result line ("System Integrity Protection is …").
const RESULT_WAIT: Duration = Duration::from_secs(30);

/// How long the framebuffer must hold still for [`settle`] to return.
const SETTLE_QUIET: Duration = Duration::from_secs(2);
/// Upper bound on a single [`settle`] call (it returns Ok at the deadline —
/// settle is best-effort, never the authoritative gate).
const SETTLE_DEADLINE: Duration = Duration::from_secs(20);
/// Poll spacing while watching for the framebuffer to quiesce.
const SETTLE_POLL: Duration = Duration::from_millis(400);

/// `wait_for_text` poll backoff, capped so a long wait still re-OCRs often.
const BACKOFF_START: Duration = Duration::from_millis(500);
const BACKOFF_MAX: Duration = Duration::from_secs(2);

/// Pause between discrete navigation keystrokes (picker arrows / Return). Some
/// guests drop a keystroke sent immediately after the prior one; mirrors the
/// script's `sleep 0.3` between `input key` calls.
const NAV_PAUSE: Duration = Duration::from_millis(300);

/// How many times to run the whole csrutil prompt interaction before giving
/// up on seeing the result line and deferring to the SSH backstop. The retry
/// is the robustness gain the blind-sleep script lacked (ADR-0008 Q2).
const CSRUTIL_ATTEMPTS: u32 = 2;

/// How long to wait for the recovery `tart` process to exit after `halt`
/// before forcing a `tart stop` (script lines 573–586: 60 × 2s). The recovery
/// Terminal's `halt` is a real root shutdown, so this wait usually completes.
const HALT_WAIT_ATTEMPTS: u32 = 60;
const HALT_WAIT_INTERVAL: Duration = Duration::from_secs(2);

/// How long [`stop_vm_graceful`] waits for the normal-boot VM's `tart` process
/// to exit before forcing a `tart stop`. The script waits 60 × 2s; we bound it
/// to 20 × 2s = 40s because the `010` live runs showed the System-Events
/// shutdown never takes effect headless — it always falls through to the
/// force-stop, so a longer wait is pure dead time. Force-stopping a recovery
/// *intermediate* VM is harmless (the next step reboots it anyway).
const GRACEFUL_STOP_ATTEMPTS: u32 = 20;
const GRACEFUL_STOP_INTERVAL: Duration = Duration::from_secs(2);

// ---- OCR queries --------------------------------------------------------
//
// Case-insensitive substrings (matched by `find_text`). The navigation
// queries are large, reliably-OCR'd UI text. The csrutil-prompt queries are
// small monospace and the likeliest to need tuning against a live run — hence
// the OCR-then-settle fallback wherever they are used.

const Q_PICKER_OPTIONS: &str = "Options";
const Q_DESKTOP_UTILITIES: &str = "Utilities";
const Q_MENU_TERMINAL: &str = "Terminal";
/// `csrutil disable`/`enable` print a confirmation prompt, then prompt for an
/// admin username and password. These anchors were read off the live recovery
/// Terminal (`010` verification, Tahoe): the confirm prompt ends `… ? [y/n]:`,
/// the username prompt is `Authorized user:`, the password prompt `Password:`.
/// They remain best-effort (each has a settle fallback) — wording can drift
/// across macOS versions — but matching the observed text avoids leaning on the
/// slow fallback on the happy path.
const Q_PROMPT_PROCEED: &str = "y/n";
const Q_PROMPT_USERNAME: &str = "Authorized";
const Q_PROMPT_PASSWORD: &str = "Password";
/// Confirms csrutil actually ran and produced its SIP output. Both the confirm
/// prompt ("Turning off System Integrity Protection …") and the result carry
/// this phrase, so a match means "csrutil rendered SIP output on this screen"
/// — present iff Terminal opened and the command ran. It is **not** a
/// success/failure discriminator: the authoritative gate is the post-reboot
/// `csrutil status` check over SSH (ADR-0008). Absence (wrong screen, Terminal
/// never opened, command mistyped) is what triggers the interaction retry.
const Q_RESULT_SIP: &str = "System Integrity Protection";

// ---- errors -------------------------------------------------------------

/// A failure inside the recovery driver. Converts into
/// [`VmError::GoldenCreateFailed`] so the command layer surfaces one stable
/// §4 code; the variant detail is preserved in the message.
#[derive(Debug, thiserror::Error)]
pub enum RecoveryError {
    #[error("recovery: expected on-screen text {query:?} did not appear within the deadline")]
    TextNotFound { query: String },
    #[error("recovery: RFB error: {0}")]
    Rfb(#[from] RfbError),
    #[error("recovery: input error: {0}")]
    Input(#[from] InputError),
    #[error("recovery: OCR error: {0}")]
    Ocr(String),
    #[error("recovery: PNG encode failed: {0}")]
    Encode(String),
}

impl From<CaptureError> for RecoveryError {
    fn from(e: CaptureError) -> Self {
        match e {
            CaptureError::Rfb(e) => RecoveryError::Rfb(e),
            CaptureError::Encode(e) => RecoveryError::Encode(e.to_string()),
        }
    }
}

impl From<RecoveryError> for VmError {
    fn from(e: RecoveryError) -> Self {
        VmError::GoldenCreateFailed { detail: e.to_string() }
    }
}

// ---- the screen-settle detector (pure, unit-tested) ---------------------

/// Tracks successive framebuffer hashes to decide when the screen has stopped
/// changing — the state behind [`RecoverySession::settle`], factored out so
/// the quiesce logic is unit-testable without a live framebuffer.
#[derive(Debug, Default)]
struct SettleTracker {
    last: Option<u64>,
    stable_since: Option<Instant>,
}

impl SettleTracker {
    /// Observe a frame `hash` seen at `now`. Returns `true` once the hash has
    /// been unchanged for at least `quiet`. Any change resets the clock.
    fn observe(&mut self, hash: u64, now: Instant, quiet: Duration) -> bool {
        if self.last == Some(hash) {
            let since = *self.stable_since.get_or_insert(now);
            now.duration_since(since) >= quiet
        } else {
            self.last = Some(hash);
            self.stable_since = Some(now);
            false
        }
    }
}

/// A cheap order-sensitive hash of a frame's pixels, for change detection.
fn frame_hash(rgba: &[u8]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    rgba.hash(&mut h);
    h.finish()
}

// ---- csrutil prompt sequence (pure, unit-tested) ------------------------

/// One step of the csrutil terminal interaction. Modelling the sequence as
/// data (rather than inline calls) lets the ordering be unit-tested without a
/// live Terminal, and keeps [`RecoverySession::run_csrutil_interaction`] a
/// plain interpreter.
#[derive(Debug, Clone, PartialEq, Eq)]
enum RecoveryStep {
    /// Type a literal string (no trailing newline).
    Type(String),
    /// Press a named key (e.g. `return`).
    Key(&'static str),
    /// Wait for an OCR prompt anchor, falling back to [`settle`] on a miss.
    AwaitPrompt(&'static str),
}

/// The ordered keystroke/wait plan for one `csrutil` invocation, mirroring the
/// script's structure (lines 541–563): run the command, confirm, then enter
/// the admin username and password, waiting for each prompt in between.
fn csrutil_interaction_steps(cmd: &str, user: &str, pass: &str) -> Vec<RecoveryStep> {
    vec![
        RecoveryStep::Type(cmd.to_string()),
        RecoveryStep::Key("return"),
        RecoveryStep::AwaitPrompt(Q_PROMPT_PROCEED),
        RecoveryStep::Type("y".to_string()),
        RecoveryStep::Key("return"),
        RecoveryStep::AwaitPrompt(Q_PROMPT_USERNAME),
        RecoveryStep::Type(user.to_string()),
        RecoveryStep::Key("return"),
        RecoveryStep::AwaitPrompt(Q_PROMPT_PASSWORD),
        RecoveryStep::Type(pass.to_string()),
        RecoveryStep::Key("return"),
    ]
}

// ---- the session --------------------------------------------------------

/// The matched location returned by [`RecoverySession::wait_for_text`]: the
/// detection plus its centre point, ready to use as a click/drag target.
#[derive(Debug, Clone)]
pub struct Located {
    pub detection: OcrDetection,
    pub centre: (f64, f64),
}

/// An observe/act/verify session bound to one recovery VNC connection +
/// OCR engine. Short-lived: one is created per recovery boot and dropped
/// (closing the TCP connection) when the boot's work is done.
pub struct RecoverySession {
    conn: RfbConnection<BufReader<TcpStream>>,
    engine: OcrEngine,
}

impl RecoverySession {
    /// Connect to the recovery VNC endpoint and select the host OCR engine.
    pub async fn connect(
        host: &str,
        port: u16,
        password: Option<&str>,
    ) -> Result<Self, RecoveryError> {
        let conn = RfbConnection::connect(host, port, password.map(str::as_bytes)).await?;
        Ok(Self { conn, engine: OcrEngine::detect() })
    }

    /// Release the OCR engine's resources (a no-op for in-process Vision; the
    /// EasyOCR fallback daemon is terminated). Call once when the session's
    /// recovery work is finished.
    pub async fn shutdown(&self) {
        self.engine.shutdown().await;
    }

    // -- observe ----------------------------------------------------------

    /// Poll the framebuffer (capture → OCR → [`find_text`]) until `query`
    /// appears or `deadline` passes. The matched detection's centre is
    /// returned for click targeting. Backoff between polls grows to
    /// [`BACKOFF_MAX`] so a long wait still re-OCRs frequently early on.
    pub async fn wait_for_text(
        &mut self,
        query: &str,
        deadline: Instant,
    ) -> Result<Located, RecoveryError> {
        let mut backoff = BACKOFF_START;
        loop {
            let png = capture_frame_png(&mut self.conn).await?;
            let detections = self
                .engine
                .recognize(&png)
                .await
                .map_err(|e| RecoveryError::Ocr(e.to_string()))?;
            if let FindOutcome::Found { matches, .. } = find_text(query, &detections) {
                if let Some(detection) = matches.into_iter().next() {
                    let centre = detection.centre();
                    return Ok(Located { detection, centre });
                }
            }
            if Instant::now() >= deadline {
                return Err(RecoveryError::TextNotFound { query: query.to_string() });
            }
            tokio::time::sleep(backoff).await;
            backoff = (backoff * 2).min(BACKOFF_MAX);
        }
    }

    /// Wait until the framebuffer stops changing for [`SETTLE_QUIET`], bounded
    /// by `deadline`. Returns `Ok` either on quiesce or at the deadline —
    /// settle is a best-effort proxy for a signal-less gap, never an
    /// authoritative gate, so a timeout is "proceed", not "fail".
    pub async fn settle(&mut self, deadline: Instant) -> Result<(), RecoveryError> {
        let mut tracker = SettleTracker::default();
        loop {
            let fb = capture_frame(&mut self.conn).await?;
            let now = Instant::now();
            if tracker.observe(frame_hash(fb.rgba()), now, SETTLE_QUIET) {
                return Ok(());
            }
            if now >= deadline {
                return Ok(());
            }
            tokio::time::sleep(SETTLE_POLL).await;
        }
    }

    /// OCR-primary prompt sync with a settle fallback (ADR-0008 Q2): try to OCR
    /// `query` within [`PROMPT_OCR_WAIT`]; if it does not appear (small
    /// monospace OCR is the flaky case), fall back to waiting for the screen to
    /// quiesce. Either way the next keystroke then fires.
    async fn await_prompt(&mut self, query: &str) -> Result<(), RecoveryError> {
        match self.wait_for_text(query, deadline_in(PROMPT_OCR_WAIT)).await {
            Ok(_) => Ok(()),
            Err(RecoveryError::TextNotFound { .. }) => self.settle(deadline_in(SETTLE_DEADLINE)).await,
            Err(e) => Err(e),
        }
    }

    // -- act --------------------------------------------------------------

    /// Press and release a named key with no modifiers.
    async fn key(&mut self, name: &str) -> Result<(), RecoveryError> {
        self.conn.press_key(name, &[], Platform::Macos).await?;
        Ok(())
    }

    /// Type a literal string (per-character key events; newlines are skipped
    /// by the input layer — issue [`RecoverySession::key`]`("return")`).
    async fn type_text(&mut self, text: &str) -> Result<(), RecoveryError> {
        self.conn.type_text(text).await?;
        Ok(())
    }

    async fn mouse_down(&mut self, x: u16, y: u16) -> Result<(), RecoveryError> {
        self.conn.mouse_down(x, y, "left").await?;
        Ok(())
    }

    async fn mouse_up(&mut self, x: u16, y: u16) -> Result<(), RecoveryError> {
        self.conn.mouse_up(x, y, "left").await?;
        Ok(())
    }

    async fn mouse_move(&mut self, x: u16, y: u16) -> Result<(), RecoveryError> {
        self.conn.mouse_move(x, y).await?;
        Ok(())
    }

    // -- composite flows --------------------------------------------------

    /// Open Terminal from the recovery desktop via the Utilities menu. macOS
    /// menu-bar menus open on a **click** and stay open (sticky) — a held
    /// mouse-down does *not* pop the dropdown on the recovery desktop (observed
    /// in the `010` live run). So: OCR-locate "Utilities", click it to open the
    /// dropdown, OCR-locate "Terminal" in the open menu, and click it to launch
    /// Terminal. Replaces the script's fragile press-hold-drag (lines 493–536),
    /// whose separate-connection mouse-down released on disconnect — i.e. was a
    /// click in disguise.
    async fn open_terminal_via_menu(&mut self) -> Result<(), RecoveryError> {
        let utilities = self.wait_for_text(Q_DESKTOP_UTILITIES, deadline_in(DESKTOP_WAIT)).await?;
        let (ux, uy) = round_point(utilities.centre);
        self.click_at(ux, uy).await?;

        // The sticky dropdown renders; OCR finds the "Terminal" item.
        let terminal = self.wait_for_text(Q_MENU_TERMINAL, deadline_in(MENU_ITEM_WAIT)).await?;
        let (tx, ty) = round_point(terminal.centre);
        self.click_at(tx, ty).await?;

        // Let the Terminal window come frontmost before typing into it.
        self.settle(deadline_in(SETTLE_DEADLINE)).await?;
        Ok(())
    }

    /// Move onto a point and click it (move → down → up), with short settle
    /// pauses so the guest registers the hover before the press and the press
    /// before the release.
    async fn click_at(&mut self, x: u16, y: u16) -> Result<(), RecoveryError> {
        self.mouse_move(x, y).await?;
        tokio::time::sleep(NAV_PAUSE).await;
        self.mouse_down(x, y).await?;
        tokio::time::sleep(NAV_PAUSE).await;
        self.mouse_up(x, y).await?;
        Ok(())
    }

    /// Run one pass of the csrutil prompt interaction (type command → confirm
    /// → username → password), interpreting [`csrutil_interaction_steps`].
    async fn run_csrutil_interaction(&mut self, cmd: &str) -> Result<(), RecoveryError> {
        for step in csrutil_interaction_steps(cmd, VANILLA_USER, VANILLA_PASS) {
            match step {
                RecoveryStep::Type(text) => self.type_text(&text).await?,
                RecoveryStep::Key(name) => {
                    tokio::time::sleep(NAV_PAUSE).await;
                    self.key(name).await?;
                }
                RecoveryStep::AwaitPrompt(query) => self.await_prompt(query).await?,
            }
        }
        Ok(())
    }
}

// ---- top-level recovery boot --------------------------------------------

/// Boot the setup VM into Recovery, run `cmd` (a `csrutil disable`/`enable`)
/// against its Terminal, halt, and reboot normally back to an SSH-reachable
/// state. Returns the post-reboot [`SetupVm`] (new `tart` pid + IP) for the
/// caller to continue provisioning.
///
/// Macro sequence at parity with the script's `_recovery_boot_csrutil`; every
/// blind sleep replaced by a signal-driven wait. The in-recovery waits need
/// not be perfect: the authoritative correctness gate is the post-reboot
/// `csrutil status` check over SSH, which the caller (or the live-VM test)
/// performs against the returned VM.
pub async fn recovery_boot_csrutil(
    setup: &SetupVm,
    cmd: &str,
    paths: &VmPaths,
) -> Result<SetupVm, VmError> {
    eprintln!("=== Recovery: {cmd} ===");

    // 1. Graceful stop of the running setup VM (System Events shutdown over
    //    SSH, then wait for the tart process to exit, force-stop fallback).
    stop_vm_graceful(setup).await;
    // Let tart fully release the VM before re-launching it (as TartRunner::start
    // does after a reclaim) so the recovery `tart run` does not race the stop.
    tokio::time::sleep(Duration::from_secs(2)).await;

    // 2. Boot into Recovery with VNC; parse the endpoint from the run log.
    eprintln!("Booting into Recovery with VNC...");
    let (recovery_pid, log_path) = crate::tart::run_detached_recovery(&setup.id, &paths.vms_dir())?;
    let Some(vnc) = crate::tart::poll_vnc_url(&log_path, 60, Duration::from_secs(1)) else {
        crate::process::terminate(recovery_pid, Duration::from_millis(200), 10);
        return Err(VmError::VmBootTimeout { id: setup.id.clone() });
    };
    eprintln!("Recovery VNC at {}:{}", vnc.host, vnc.port);

    // Drive the recovery GUI, tearing the recovery boot down on any failure so
    // a botched run never leaves an orphaned recovery VM.
    let driver_result =
        drive_recovery(&vnc.host, vnc.port, vnc.password.as_deref(), cmd).await;
    if let Err(e) = driver_result {
        // Stop (do NOT delete) — the clone is left for inspection / retry, and
        // deleting it would also destroy the still-needed setup VM.
        crate::tart::stop(&setup.id);
        crate::process::terminate(recovery_pid, Duration::from_millis(200), 10);
        return Err(e.into());
    }

    // 7. Wait for the recovery VM to halt (force-stop fallback).
    eprint!("Waiting for recovery VM to halt...");
    if !wait_for_pid_exit(recovery_pid, HALT_WAIT_ATTEMPTS, HALT_WAIT_INTERVAL) {
        eprintln!(" forcing stop.");
        // Stop only — the very next step reboots this same VM normally.
        crate::tart::stop(&setup.id);
        crate::process::terminate(recovery_pid, Duration::from_millis(200), 10);
    } else {
        eprintln!(" done.");
    }

    // 8. Reboot normally and wait for SSH (pubkey auth persists across the
    //    recovery cycle, so key auth works on the way back up).
    reboot_and_wait_ssh(setup, paths).await
}

/// The interactive recovery GUI sequence (steps 3–6), isolated so the caller
/// can wrap it in teardown-on-error. Connects, navigates the picker, opens
/// Terminal, runs csrutil (with result-line retry), and halts the guest from
/// the recovery Terminal.
async fn drive_recovery(
    host: &str,
    port: u16,
    password: Option<&str>,
    cmd: &str,
) -> Result<(), RecoveryError> {
    let mut session = RecoverySession::connect(host, port, password).await?;

    // 3. Wait for the framebuffer to come alive (any OCR-able text).
    eprintln!("Waiting for recovery framebuffer...");
    session.wait_for_text(Q_PICKER_OPTIONS, deadline_in(FB_LIVE_WAIT.max(PICKER_WAIT))).await?;

    // 4. Startup picker: Right, Right, Return → recovery desktop.
    eprintln!("Navigating startup picker...");
    for key in ["right", "right", "return"] {
        session.key(key).await?;
        tokio::time::sleep(NAV_PAUSE).await;
    }

    // 5. Open Terminal via the Utilities menu (press-hold-drag).
    eprintln!("Opening Terminal via Utilities menu...");
    session.open_terminal_via_menu().await?;

    // 6. Run csrutil, retrying the whole prompt interaction until the result
    //    line appears (or attempts run out — the SSH backstop is authoritative).
    eprintln!("Running '{cmd}'...");
    let mut saw_result = false;
    for attempt in 1..=CSRUTIL_ATTEMPTS {
        session.run_csrutil_interaction(cmd).await?;
        match session.wait_for_text(Q_RESULT_SIP, deadline_in(RESULT_WAIT)).await {
            Ok(located) => {
                eprintln!("  csrutil result: {}", located.detection.text);
                saw_result = true;
                break;
            }
            Err(RecoveryError::TextNotFound { .. }) => {
                eprintln!(
                    "  csrutil result line not seen (attempt {attempt}/{CSRUTIL_ATTEMPTS}); \
                     SSH status check is authoritative."
                );
                // Dismiss any half-entered prompt before re-running.
                session.key("return").await?;
            }
            Err(e) => return Err(e),
        }
    }
    let _ = saw_result; // informational only; SSH backstop is the gate.

    // Halt the guest from the recovery Terminal.
    eprintln!("Halting recovery VM...");
    session.type_text("halt").await?;
    session.key("return").await?;
    session.shutdown().await;
    Ok(())
}

/// Graceful stop: ask macOS to shut down cleanly over SSH (System Events, not
/// `shutdown -h`, so session state is saved), then wait for the `tart` process
/// to exit, forcing a `tart stop` on timeout. Ports `_stop_vm_graceful`
/// (script lines 331–347). Best-effort throughout — a stop failure must not
/// abort the recovery cycle.
async fn stop_vm_graceful(setup: &SetupVm) {
    eprint!("Shutting down setup VM...");
    if let Ok(session) =
        SshSession::connect_key(&setup.ip, 22, VANILLA_USER, &setup.key_path).await
    {
        let _ = session
            .exec("osascript -e 'tell application \"System Events\" to shut down'")
            .await;
    }
    if wait_for_pid_exit(setup.pid, GRACEFUL_STOP_ATTEMPTS, GRACEFUL_STOP_INTERVAL) {
        eprintln!(" done.");
    } else {
        eprintln!(" forcing stop.");
        // Stop only — recovery boots this same VM next; deleting it would
        // strand the recovery cycle (the bug fixed in the `010` live run).
        crate::tart::stop(&setup.id);
        crate::process::terminate(setup.pid, Duration::from_millis(200), 10);
    }
}

/// Poll until `pid` is no longer alive, up to `attempts` spaced by `interval`.
/// Returns `true` if it exited within the window. A non-positive pid is
/// treated as already gone. Shared with [`crate::finalize`] for the final
/// clean-shutdown wait.
pub(crate) fn wait_for_pid_exit(pid: i32, attempts: u32, interval: Duration) -> bool {
    for attempt in 0..attempts {
        if !crate::process::process_alive(pid) {
            return true;
        }
        if attempt + 1 < attempts {
            std::thread::sleep(interval);
        }
    }
    !crate::process::process_alive(pid)
}

/// Boot the setup VM normally and wait for SSH key auth to answer, returning
/// the refreshed [`SetupVm`]. `--vnc-experimental` is kept so WindowServer
/// starts (needed for the later TCC/accessibility work — script line 591); on
/// Linux the same flag gives GDM a virtual display so it does not crash-loop
/// (Linux script's `--vnc-experimental` note). `pub(crate)` so the Tier-2 Linux
/// golden ([`crate::golden_linux`]) reuses it for its apply-settings reboot —
/// the boot+poll-IP+wait-SSH mechanism is platform-neutral.
pub(crate) async fn reboot_and_wait_ssh(setup: &SetupVm, paths: &VmPaths) -> Result<SetupVm, VmError> {
    eprintln!("Rebooting normally...");
    let (pid, _log) = crate::tart::run_detached(&setup.id, &paths.vms_dir())?;

    eprint!("Waiting for guest IP...");
    let Some(ip) = crate::tart::poll_ip(&setup.id, 60, Duration::from_secs(3)) else {
        eprintln!(" timed out.");
        crate::process::terminate(pid, Duration::from_millis(200), 10);
        return Err(VmError::VmBootTimeout { id: setup.id.clone() });
    };
    eprintln!(" {ip}");

    eprint!("Waiting for SSH...");
    let mut last: Option<VmError> = None;
    for attempt in 0..60u32 {
        match SshSession::connect_key(&ip, 22, VANILLA_USER, &setup.key_path).await {
            Ok(_) => {
                eprintln!(" ready.");
                return Ok(SetupVm {
                    id: setup.id.clone(),
                    pid,
                    ip,
                    key_path: setup.key_path.clone(),
                });
            }
            Err(e) => last = Some(e),
        }
        if attempt + 1 < 60 {
            tokio::time::sleep(Duration::from_secs(3)).await;
        }
    }
    eprintln!(" not reachable.");
    crate::process::terminate(pid, Duration::from_millis(200), 10);
    Err(last.unwrap_or(VmError::VmBootTimeout { id: setup.id.clone() }))
}

/// A deadline `d` from now — the polling-wait equivalent of the script's
/// fixed-budget `sleep N`.
fn deadline_in(d: Duration) -> Instant {
    Instant::now() + d
}

/// Round an OCR centre point to integer framebuffer coordinates for a pointer
/// event. Negative or overflowing values are clamped into `u16`.
fn round_point((x, y): (f64, f64)) -> (u16, u16) {
    let clamp = |v: f64| v.round().clamp(0.0, u16::MAX as f64) as u16;
    (clamp(x), clamp(y))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settle_tracker_reports_quiesced_after_unchanged_for_quiet() {
        let mut t = SettleTracker::default();
        let base = Instant::now();
        let quiet = Duration::from_secs(2);
        // First sighting starts the clock; not settled yet.
        assert!(!t.observe(7, base, quiet));
        // Same hash, but only 1s elapsed — still not settled.
        assert!(!t.observe(7, base + Duration::from_secs(1), quiet));
        // Same hash, 2s elapsed — settled.
        assert!(t.observe(7, base + Duration::from_secs(2), quiet));
    }

    #[test]
    fn settle_tracker_resets_clock_on_any_change() {
        let mut t = SettleTracker::default();
        let base = Instant::now();
        let quiet = Duration::from_secs(2);
        assert!(!t.observe(1, base, quiet));
        assert!(t.observe(1, base + Duration::from_secs(2), quiet)); // settled at 2s
        // A change resets: even though time keeps advancing, the new hash's
        // clock starts fresh.
        assert!(!t.observe(2, base + Duration::from_secs(3), quiet));
        assert!(!t.observe(2, base + Duration::from_secs(4), quiet)); // only 1s on hash=2
        assert!(t.observe(2, base + Duration::from_secs(5), quiet)); // 2s on hash=2
    }

    #[test]
    fn frame_hash_is_stable_and_change_sensitive() {
        let a = [0u8, 1, 2, 3];
        let b = [0u8, 1, 2, 3];
        let c = [0u8, 1, 2, 4];
        assert_eq!(frame_hash(&a), frame_hash(&b));
        assert_ne!(frame_hash(&a), frame_hash(&c));
    }

    #[test]
    fn csrutil_steps_follow_command_confirm_username_password_order() {
        let steps = csrutil_interaction_steps("csrutil disable", "admin", "admin");
        assert_eq!(
            steps,
            vec![
                RecoveryStep::Type("csrutil disable".into()),
                RecoveryStep::Key("return"),
                RecoveryStep::AwaitPrompt(Q_PROMPT_PROCEED),
                RecoveryStep::Type("y".into()),
                RecoveryStep::Key("return"),
                RecoveryStep::AwaitPrompt(Q_PROMPT_USERNAME),
                RecoveryStep::Type("admin".into()),
                RecoveryStep::Key("return"),
                RecoveryStep::AwaitPrompt(Q_PROMPT_PASSWORD),
                RecoveryStep::Type("admin".into()),
                RecoveryStep::Key("return"),
            ]
        );
    }

    #[test]
    fn csrutil_steps_carry_the_command_verbatim() {
        // `enable` and `disable` share one plan; only the typed command differs.
        let enable = csrutil_interaction_steps("csrutil enable", "u", "p");
        assert_eq!(enable[0], RecoveryStep::Type("csrutil enable".into()));
        // Three prompts, three Types of payload (cmd, y, user, pass = 4 Types).
        let types = enable.iter().filter(|s| matches!(s, RecoveryStep::Type(_))).count();
        let prompts = enable.iter().filter(|s| matches!(s, RecoveryStep::AwaitPrompt(_))).count();
        assert_eq!(types, 4);
        assert_eq!(prompts, 3);
    }

    #[test]
    fn round_point_clamps_and_rounds() {
        assert_eq!(round_point((10.4, 20.6)), (10, 21));
        assert_eq!(round_point((-5.0, 0.0)), (0, 0));
        assert_eq!(round_point((1e9, 1e9)), (u16::MAX, u16::MAX));
    }
}
