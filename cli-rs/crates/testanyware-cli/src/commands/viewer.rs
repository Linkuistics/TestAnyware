//! `testanyware viewer` — the embedded `eframe`/`wgpu` viewer (ADR-0005).
//!
//! Leaf 060/010 built the **read-only render skeleton**; leaf 060/020 made it
//! **interactive** (egui pointer/keyboard forwarded to the guest over RFB);
//! leaf 060/030 (this work) adds **lifecycle polish**: the RFB thread
//! **auto-reconnects** with bounded backoff so the window survives guest
//! reboots / transient drops, and `vm start --viewer` opens the viewer inline
//! (the sugar lives in `commands/vm.rs`, reusing [`run_viewer`]).
//!
//! ## Architecture (ADR-0005)
//!
//! `eframe::run_native` must own the process **main thread** (winit/NSApp
//! requirement). The CLI is `#[tokio::main]`, but its `block_on` drives the
//! root future on the main thread, so as long as the `viewer` dispatch arm
//! is a *synchronous* call (no `.await`, no `tokio::spawn`) this function
//! runs on the main thread — eframe gets it with no `main.rs` carve-out.
//!
//! A **dedicated `std::thread` running its own current-thread tokio
//! runtime** owns the [`RfbConnection`] and runs a `tokio::select!` loop,
//! fully isolated from the CLI's global runtime (no nested-runtime hazard).
//! Handoff:
//!   - UI ← RFB: [`Arc<Mutex<FrameSlot>>`] holding the latest RGBA + dims.
//!   - UI → RFB: an `mpsc` of [`ViewerInput`] produced by the eframe app's
//!     `ui()` from egui input events and drained by the `select!` loop.
//!   - shutdown: a `watch` channel set when the window closes.
//!   - wake: an [`egui::Context`] clone in the RFB thread calls
//!     `request_repaint()` on each new frame, so eframe does not busy-poll.
//!
//! ## Input model (leaf 020)
//!
//! Input reuses the existing `testanyware-rfb` layer that powers every
//! `input *` command — `RfbConnection::{key_event, pointer_event}` plus the
//! `keymap` name→keysym/button tables; this leaf only *wires* egui events
//! into them. Keysym mapping is keyed on the **guest** [`Platform`] (resolved
//! the same way `input` resolves it), not the host, so the macOS
//! Cmd→`XK_Alt_L` swap in `keymap` applies automatically.
//!
//! - **Pointer**: egui pointer position maps to a framebuffer pixel through
//!   the displayed image rect (`fb_pixel`); held buttons become an RFB
//!   button mask; wheel events decompose into transient wheel-bit pulses.
//! - **Typing**: `egui::Event::Text` characters are sent via base keysyms
//!   with shift bracketing (reusing `shifted_char_to_base`), composing with
//!   the globally-tracked physical shift so capitals/symbols are correct.
//! - **Special keys** (arrows, Enter, Tab, Esc, F-keys, …) come from
//!   `egui::Event::Key` via `key_for_name`; letters/digits also come from
//!   `Key` events **only while a command modifier is held** (Cmd/Ctrl/Alt
//!   chords, where egui suppresses `Text`).
//! - **Modifiers** are tracked frame-to-frame from `egui::Modifiers` and
//!   forwarded as held keysyms via `modifier_for_name(.., guest_platform)`;
//!   they are released if the window loses focus to avoid stuck modifiers.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use eframe::egui;
use testanyware_rfb::keymap::{self, Platform};
use testanyware_rfb::{RfbConnection, ServerEvent};

use crate::output::{print_error, OutputMode};
use crate::resolve::{
    resolve_platform_with_env, resolve_vnc, ConnectionOptions, EnvProvider, ResolveError,
};

/// Trackpad/wheel `Point`-unit delta treated as one wheel line. egui emits
/// `Point` deltas for precise trackpads; VNC wheel events are discrete
/// notches, so we coalesce points into lines before decomposing into pulses.
const POINTS_PER_WHEEL_LINE: f32 = 40.0;
/// A `Page`-unit wheel notch is treated as this many lines.
const LINES_PER_WHEEL_PAGE: f32 = 3.0;
/// Cap on wheel pulses emitted per axis per frame, so a fast flick or a
/// large trackpad delta cannot flood the guest with hundreds of clicks.
const MAX_WHEEL_PULSES: u32 = 16;

/// How often the RFB thread asks the server for an incremental update,
/// keeping the framebuffer stream flowing (~30 polls/sec). The server
/// answers only when something changed, so an idle desktop is cheap.
const INCREMENTAL_POLL_INTERVAL: Duration = Duration::from_millis(33);

/// First reconnect delay after a drop, and the floor the backoff resets to
/// once a session has genuinely streamed (a guest reboot reconnects quickly).
const INITIAL_BACKOFF: Duration = Duration::from_millis(500);
/// Ceiling on the exponential backoff — the viewer is a dev tool, not a
/// service, so keep retries snappy and modest (ADR-0005 / leaf 030 notes).
const MAX_BACKOFF: Duration = Duration::from_secs(5);
/// Consecutive *unproductive* connect attempts (never delivered a frame)
/// before the RFB thread gives up and paints a terminal overlay. A drop
/// after a working session resets this, so bouncing a VM never exhausts it.
const MAX_CONNECT_ATTEMPTS: u32 = 12;

/// Latest framebuffer state shared from the RFB thread to the UI.
///
/// The RFB thread overwrites `rgba`/`width`/`height` and sets `dirty` on
/// each applied update; the UI consumes it (clearing `dirty`) and uploads
/// to a texture. On a connection drop the RFB thread sets `disconnected` +
/// `status` so the UI paints an overlay over the last frame; while the
/// thread retries it updates `status` to "Reconnecting…", and on success it
/// clears `disconnected`. After the give-up ceiling it sets `gave_up` so
/// [`run_viewer`] can exit non-zero once the window is closed (leaf 030).
#[derive(Default)]
struct FrameSlot {
    rgba: Vec<u8>,
    width: u32,
    height: u32,
    /// A new frame is waiting for the UI to upload.
    dirty: bool,
    /// The RFB connection dropped or is reconnecting; show the overlay.
    disconnected: bool,
    /// Human-readable reason for the overlay text.
    status: Option<String>,
    /// Reconnection exhausted its budget; the overlay is terminal and the
    /// process should exit non-zero when the window closes.
    gave_up: bool,
}

/// UI → RFB input event. Produced by [`ViewerApp`] from egui input and
/// drained by the RFB thread's `select!` loop, which turns each into a
/// `key_event` / `pointer_event` on the connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ViewerInput {
    /// A key press (`down = true`) or release, as a resolved X keysym.
    Key { keysym: u32, down: bool },
    /// A pointer event: bit-packed button mask + framebuffer-pixel coords.
    Pointer { button_mask: u8, x: u16, y: u16 },
}

/// `testanyware viewer` entry point. Synchronous on purpose — see the
/// module docs: it must run on the main thread so eframe can own it.
pub fn run_viewer(opts: ConnectionOptions) {
    // Validate the endpoint up front (synchronous). A resolution failure is
    // a real usage/config error, so it exits non-zero *before* any window
    // opens — unlike a live connection drop, which surfaces as the overlay.
    // The resolved value is discarded: the RFB thread re-resolves on every
    // (re)connect from `opts`, so a spec rewritten between attempts (e.g. a
    // new VNC port after `vm start`) is picked up automatically (leaf 030).
    if let Err(err) = resolve_vnc(&opts) {
        exit_resolve_error(err);
    }
    // Keysym mapping is keyed on the *guest* platform (matching the `input`
    // command), not the host. A bad/absent value falls back to macOS rather
    // than aborting the window — the viewer is interactive, not a one-shot.
    let platform = resolve_guest_platform(&opts);

    let frame = Arc::new(Mutex::new(FrameSlot::default()));
    let (input_tx, input_rx) = tokio::sync::mpsc::channel::<ViewerInput>(256);
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    // The RFB thread needs the egui Context to wake the UI, but the Context
    // only exists once eframe builds the window. Spawn the thread now and
    // hand it the Context through a one-shot std channel from the app
    // creator closure; the thread blocks on `recv()` until the window is up.
    let (ctx_tx, ctx_rx) = std::sync::mpsc::channel::<egui::Context>();
    let rfb_frame = Arc::clone(&frame);
    let rfb_opts = opts; // moved into the thread for re-resolution on reconnect
    let rfb_thread = std::thread::Builder::new()
        .name("rfb-viewer".to_string())
        .spawn(move || {
            // If the window failed to start, the creator closure never ran,
            // so ctx_tx was dropped: nothing to do.
            let Ok(ctx) = ctx_rx.recv() else { return };
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("build viewer RFB runtime");
            rt.block_on(rfb_loop(rfb_opts, rfb_frame, input_rx, shutdown_rx, ctx));
        })
        .expect("spawn viewer RFB thread");

    let app_frame = Arc::clone(&frame);
    let native_options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: egui::ViewportBuilder::default()
            // Sized provisionally; the app resizes to the framebuffer
            // dimensions once the first frame arrives (DesktopSize honoured).
            .with_inner_size([1024.0, 768.0])
            .with_title("TestAnyware viewer"),
        ..Default::default()
    };

    let result = eframe::run_native(
        "testanyware-viewer",
        native_options,
        Box::new(move |cc| {
            // Hand the live Context to the waiting RFB thread, then build
            // the app. If the receiver is gone the send is a harmless no-op.
            let _ = ctx_tx.send(cc.egui_ctx.clone());
            Ok(Box::new(ViewerApp::new(app_frame, input_tx, platform)))
        }),
    );

    // Window closed (or failed to open): signal shutdown and join so the
    // RFB connection is torn down cleanly before we exit.
    let _ = shutdown_tx.send(true);
    let _ = rfb_thread.join();

    if let Err(err) = result {
        eprintln!("testanyware viewer: failed to open window: {err}");
        std::process::exit(1);
    }

    // If reconnection exhausted its budget, the window stayed up showing a
    // terminal overlay; report the give-up as a non-zero exit now that the
    // user has closed it (a poisoned lock is treated as "did not give up").
    if frame.lock().map(|s| s.gave_up).unwrap_or(false) {
        eprintln!("testanyware viewer: connection lost and reconnection gave up");
        std::process::exit(1);
    }
}

/// Why a `connect_and_stream` attempt ended.
enum StreamOutcome {
    /// The window closed; the thread should exit without reconnecting.
    Shutdown,
    /// The connection failed or dropped. `productive` is true if the session
    /// delivered at least one framebuffer — the signal that distinguishes a
    /// real session ending (guest reboot → reconnect indefinitely) from a
    /// dead endpoint (never streamed → count toward the give-up ceiling).
    Lost { reason: String, productive: bool },
}

/// Exponential-backoff + give-up policy for the reconnect loop. Pure data so
/// the escalation/reset/ceiling behaviour is unit-tested without a socket.
struct Backoff {
    /// Consecutive *unproductive* failures since the last productive drop.
    failures: u32,
    /// Delay before the next reconnect attempt.
    delay: Duration,
}

impl Backoff {
    fn new() -> Self {
        Self { failures: 0, delay: INITIAL_BACKOFF }
    }

    /// A session that streamed ended (e.g. guest reboot): clear the failure
    /// budget and reset the delay so the reconnect is prompt.
    fn note_productive_drop(&mut self) {
        self.failures = 0;
        self.delay = INITIAL_BACKOFF;
    }

    /// An unproductive attempt (never streamed) ended. Escalates the delay
    /// and returns `true` once the give-up ceiling is reached.
    fn note_failure(&mut self) -> bool {
        self.failures += 1;
        if self.failures >= MAX_CONNECT_ATTEMPTS {
            return true;
        }
        self.delay = (self.delay * 2).min(MAX_BACKOFF);
        false
    }

    fn delay(&self) -> Duration {
        self.delay
    }

    fn failures(&self) -> u32 {
        self.failures
    }
}

/// The dedicated-thread RFB driver: an **auto-reconnect loop** (leaf 030)
/// around [`connect_and_stream`]. It re-resolves the endpoint from `opts` on
/// every attempt, paints a "Reconnecting…" overlay between attempts, and
/// gives up (terminal overlay + `gave_up`) after [`MAX_CONNECT_ATTEMPTS`]
/// consecutive unproductive failures. A productive session ending resets the
/// budget, so bouncing a VM reconnects forever.
async fn rfb_loop(
    opts: ConnectionOptions,
    frame: Arc<Mutex<FrameSlot>>,
    mut input_rx: tokio::sync::mpsc::Receiver<ViewerInput>,
    mut shutdown_rx: tokio::sync::watch::Receiver<bool>,
    ctx: egui::Context,
) {
    let mut backoff = Backoff::new();
    loop {
        // Cheap pre-attempt check: the window may have closed during backoff.
        // `borrow()` does not mark the change seen, so a pending shutdown is
        // still caught by the `changed()` selects below.
        if *shutdown_rx.borrow() {
            return;
        }
        match connect_and_stream(&opts, &frame, &mut input_rx, &mut shutdown_rx, &ctx).await {
            StreamOutcome::Shutdown => return,
            StreamOutcome::Lost { reason, productive } => {
                if productive {
                    backoff.note_productive_drop();
                } else if backoff.note_failure() {
                    mark_gave_up(&frame, &ctx, &reason, backoff.failures());
                    return;
                }
                mark_reconnecting(&frame, &ctx, &reason, backoff.failures());
                // Wait out the backoff, but wake immediately if the window
                // closes so closing the viewer never hangs on the delay.
                tokio::select! {
                    _ = tokio::time::sleep(backoff.delay()) => {}
                    _ = shutdown_rx.changed() => return,
                }
            }
        }
    }
}

/// One connect → stream attempt. Owns the connection for the attempt's life,
/// copying each applied frame into the shared slot and waking the UI. Returns
/// the [`StreamOutcome`] describing how it ended; the caller decides whether
/// to reconnect. Re-resolves the endpoint each call so a rewritten spec is
/// honoured on reconnect.
async fn connect_and_stream(
    opts: &ConnectionOptions,
    frame: &Arc<Mutex<FrameSlot>>,
    input_rx: &mut tokio::sync::mpsc::Receiver<ViewerInput>,
    shutdown_rx: &mut tokio::sync::watch::Receiver<bool>,
    ctx: &egui::Context,
) -> StreamOutcome {
    let endpoint = match resolve_vnc(opts) {
        Ok(e) => e,
        Err(err) => {
            return StreamOutcome::Lost {
                reason: format!("Cannot resolve endpoint: {err}"),
                productive: false,
            }
        }
    };

    let connect = RfbConnection::connect(
        &endpoint.host,
        endpoint.port,
        endpoint.password.as_deref().map(str::as_bytes),
    );
    let mut conn = tokio::select! {
        result = connect => match result {
            Ok(c) => c,
            Err(err) => {
                return StreamOutcome::Lost {
                    reason: format!("Failed to connect: {err}"),
                    productive: false,
                }
            }
        },
        // The connect may block on a dead host; let a window close abort it.
        _ = shutdown_rx.changed() => return StreamOutcome::Shutdown,
    };

    // Apply the @2x logical target (ADR-0016 D2): the viewer then renders the
    // downsampled logical frame and forwards clicks in logical coords (k5 scales
    // ×2 on the wire), so the window shows 1920×1080 and clicks land correctly.
    crate::commands::apply_logical_target(&mut conn, &endpoint);

    // Connected: clear any overlay so the live frame shows again.
    mark_connected(frame, ctx);
    // True once a framebuffer arrives — see `StreamOutcome::Lost::productive`.
    let mut productive = false;

    // Kick off the stream with one full (non-incremental) update; the
    // interval below then requests incremental updates to keep it flowing.
    let (w, h) = conn.framebuffer_size();
    if let Err(err) = conn.request_framebuffer_update(false, 0, 0, w as u16, h as u16).await {
        return StreamOutcome::Lost { reason: format!("Connection lost: {err}"), productive };
    }

    let mut poll = tokio::time::interval(INCREMENTAL_POLL_INTERVAL);
    poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            msg = conn.next_message() => match msg {
                Ok(ServerEvent::FramebufferUpdated { rectangles }) if rectangles > 0 => {
                    let fb = conn.framebuffer();
                    update_slot(frame, fb.width(), fb.height(), fb.rgba());
                    productive = true;
                    ctx.request_repaint();
                }
                Ok(_) => {} // no-op update / bell / cut-text: ignore
                Err(err) => {
                    return StreamOutcome::Lost {
                        reason: format!("Connection lost: {err}"),
                        productive,
                    };
                }
            },
            Some(input) = input_rx.recv() => {
                let result = match input {
                    ViewerInput::Key { keysym, down } => conn.key_event(keysym, down).await,
                    ViewerInput::Pointer { button_mask, x, y } => {
                        conn.pointer_event(button_mask, x, y).await
                    }
                };
                if let Err(err) = result {
                    return StreamOutcome::Lost {
                        reason: format!("Connection lost: {err}"),
                        productive,
                    };
                }
            }
            _ = shutdown_rx.changed() => return StreamOutcome::Shutdown, // window closed
            _ = poll.tick() => {
                let (w, h) = conn.framebuffer_size();
                if let Err(err) =
                    conn.request_framebuffer_update(true, 0, 0, w as u16, h as u16).await
                {
                    return StreamOutcome::Lost {
                        reason: format!("Connection lost: {err}"),
                        productive,
                    };
                }
            }
        }
    }
}

/// Copy an applied framebuffer into the shared slot and flag it dirty.
fn update_slot(frame: &Arc<Mutex<FrameSlot>>, width: u32, height: u32, rgba: &[u8]) {
    let mut slot = frame.lock().expect("frame slot poisoned");
    slot.rgba.clear();
    slot.rgba.extend_from_slice(rgba);
    slot.width = width;
    slot.height = height;
    slot.dirty = true;
}

/// Clear the overlay on a (re)connect so the UI resumes showing live frames.
fn mark_connected(frame: &Arc<Mutex<FrameSlot>>, ctx: &egui::Context) {
    {
        let mut slot = frame.lock().expect("frame slot poisoned");
        slot.disconnected = false;
        slot.status = None;
    }
    ctx.request_repaint();
}

/// Paint a "Reconnecting…" overlay (over the frozen last frame) while the
/// thread waits to retry. `failures == 0` means a session that streamed just
/// dropped (a reboot); otherwise it is the consecutive-failure count.
fn mark_reconnecting(
    frame: &Arc<Mutex<FrameSlot>>,
    ctx: &egui::Context,
    reason: &str,
    failures: u32,
) {
    let status = if failures == 0 {
        format!("Reconnecting… ({reason})")
    } else {
        format!("Reconnecting… (attempt {failures}): {reason}")
    };
    {
        let mut slot = frame.lock().expect("frame slot poisoned");
        slot.disconnected = true;
        slot.status = Some(status);
    }
    ctx.request_repaint();
}

/// Paint the terminal overlay after the reconnect budget is exhausted and
/// flag `gave_up` so the process exits non-zero once the window is closed.
fn mark_gave_up(
    frame: &Arc<Mutex<FrameSlot>>,
    ctx: &egui::Context,
    reason: &str,
    attempts: u32,
) {
    {
        let mut slot = frame.lock().expect("frame slot poisoned");
        slot.disconnected = true;
        slot.gave_up = true;
        slot.status = Some(format!(
            "Disconnected: {reason} — gave up after {attempts} attempts. \
             Close the window to exit."
        ));
    }
    ctx.request_repaint();
}

/// Which physical modifiers we have currently forwarded to the guest as
/// held keysyms. Diffed against egui's `Modifiers` each frame so we emit
/// exactly one down on press and one up on release. `cmd` tracks the macOS
/// Command key (`mac_cmd`); on other hosts it stays false (cross-platform
/// modifier reach is refined in the later host leaves — ADR-0005 Q6).
#[derive(Default, Clone, Copy)]
struct ModsHeld {
    shift: bool,
    ctrl: bool,
    alt: bool,
    cmd: bool,
}

/// A snapshot of the egui input this frame, extracted under the input lock
/// so the mapping logic ([`ViewerApp::process_input`]) is a pure function of
/// plain data — unit-testable without an `egui::Context`.
struct InputSnapshot {
    pointer_pos: Option<egui::Pos2>,
    primary: bool,
    secondary: bool,
    middle: bool,
    modifiers: egui::Modifiers,
    /// `(key, pressed, repeat)` for each key event, in order.
    keys: Vec<(egui::Key, bool, bool)>,
    /// Committed text (post-IME/shift), in order.
    texts: Vec<String>,
    /// `(unit, delta)` for each wheel event, in order.
    wheels: Vec<(egui::MouseWheelUnit, egui::Vec2)>,
}

/// The eframe application: uploads the shared framebuffer to a texture and
/// draws it, scaled to fit, with a "disconnected" overlay on connection
/// loss, and forwards egui pointer/keyboard input to the guest.
struct ViewerApp {
    frame: Arc<Mutex<FrameSlot>>,
    /// UI → RFB sender. `try_send` is used so the UI never blocks; at VNC
    /// rates the 256-deep channel does not fill, and a dropped event under
    /// pathological backpressure is preferable to stalling the paint thread.
    input_tx: tokio::sync::mpsc::Sender<ViewerInput>,
    /// Guest platform — selects the keysym/modifier tables.
    platform: Platform,
    texture: Option<egui::TextureHandle>,
    /// Dimensions of the current texture; a change means the guest desktop
    /// resized, so we recreate the texture and resize the window.
    texture_dims: Option<(u32, u32)>,
    /// Modifiers currently forwarded as held keysyms.
    mods_held: ModsHeld,
    /// Last `(mask, x, y)` pointer event sent, to suppress duplicates.
    last_pointer: Option<(u8, u16, u16)>,
    /// Sub-line wheel remainder carried between frames so precise trackpad
    /// scrolling accumulates into whole wheel pulses instead of being lost.
    scroll_accum: egui::Vec2,
}

impl ViewerApp {
    fn new(
        frame: Arc<Mutex<FrameSlot>>,
        input_tx: tokio::sync::mpsc::Sender<ViewerInput>,
        platform: Platform,
    ) -> Self {
        Self {
            frame,
            input_tx,
            platform,
            texture: None,
            texture_dims: None,
            mods_held: ModsHeld::default(),
            last_pointer: None,
            scroll_accum: egui::Vec2::ZERO,
        }
    }

    /// The X keysym for the guest's Shift, used for typing-path bracketing.
    fn shift_keysym(&self) -> u32 {
        keymap::modifier_for_name("shift", self.platform).unwrap_or(keymap::xk::SHIFT_L)
    }

    /// Map egui input to a batch of [`ViewerInput`] events. Pure over its
    /// inputs except for the per-frame state it carries (`mods_held`,
    /// `last_pointer`, `scroll_accum`); kept free of `egui::Context` so it
    /// is exercised directly in tests. `img_rect` is the on-screen rect the
    /// framebuffer is painted into (for coordinate mapping); `fb` is the
    /// guest desktop size in pixels.
    fn process_input(
        &mut self,
        snap: &InputSnapshot,
        img_rect: egui::Rect,
        fb: (u32, u32),
    ) -> Vec<ViewerInput> {
        let mut out = Vec::new();
        let shift_ks = self.shift_keysym();

        // 1. Modifier edges first, so a chord's modifier is down before the
        //    key it modifies arrives later this frame.
        for (name, now, slot) in [
            ("shift", snap.modifiers.shift, &mut self.mods_held.shift),
            ("ctrl", snap.modifiers.ctrl, &mut self.mods_held.ctrl),
            ("alt", snap.modifiers.alt, &mut self.mods_held.alt),
            ("cmd", snap.modifiers.mac_cmd, &mut self.mods_held.cmd),
        ] {
            if now != *slot {
                if let Some(ks) = keymap::modifier_for_name(name, self.platform) {
                    out.push(ViewerInput::Key { keysym: ks, down: now });
                }
                *slot = now;
            }
        }
        // A non-shift ("command") modifier changes typing semantics: egui
        // suppresses Text under it, so chord keys come via Key events.
        let command_active = self.mods_held.ctrl || self.mods_held.alt || self.mods_held.cmd;

        // 2. Typed characters (only when no command modifier is held — under
        //    one, egui emits no Text and the Key path handles the chord).
        if !command_active {
            for text in &snap.texts {
                for ch in text.chars() {
                    if let Some((base, needs_shift)) = char_to_send(ch) {
                        emit_char(&mut out, base, needs_shift, self.mods_held.shift, shift_ks);
                    }
                }
            }
        }

        // 3. Key events: special keys always; letters/digits only as chords.
        for &(key, pressed, _repeat) in &snap.keys {
            if let Some(ks) = special_keysym(key) {
                out.push(ViewerInput::Key { keysym: ks, down: pressed });
            } else if command_active {
                if let Some(ks) = letter_or_digit_keysym(key) {
                    out.push(ViewerInput::Key { keysym: ks, down: pressed });
                }
            }
        }

        // 4. Pointer position/buttons + wheel.
        if let Some(pos) = snap.pointer_pos {
            let (x, y) = fb_pixel(pos, img_rect, fb.0, fb.1);
            let mask = button_mask(snap.primary, snap.secondary, snap.middle);
            if self.last_pointer != Some((mask, x, y)) {
                out.push(ViewerInput::Pointer { button_mask: mask, x, y });
                self.last_pointer = Some((mask, x, y));
            }
            let (dx, dy) = self.accumulate_wheel(&snap.wheels);
            for comp in keymap::decompose_scroll(dx, dy) {
                let bit = comp.direction.button_bit();
                for _ in 0..comp.steps.min(MAX_WHEEL_PULSES) {
                    // A wheel "click" is a transient down+up edge of the
                    // direction bit, preserving any held buttons (matching
                    // the `scroll` command's pattern in `input.rs`).
                    out.push(ViewerInput::Pointer { button_mask: mask | (1 << bit), x, y });
                    out.push(ViewerInput::Pointer { button_mask: mask, x, y });
                }
            }
        }

        out
    }

    /// Fold this frame's wheel events into whole wheel lines, carrying the
    /// sub-line remainder. Returns `(dx, dy)` in the `keymap` scroll
    /// convention: `dy > 0` scrolls down, `dx > 0` scrolls right — matching
    /// egui's "positive = content moves down/right". (Sign is the one thing
    /// here that wants live confirmation against a guest.)
    fn accumulate_wheel(&mut self, wheels: &[(egui::MouseWheelUnit, egui::Vec2)]) -> (i32, i32) {
        for &(unit, delta) in wheels {
            let lines = match unit {
                egui::MouseWheelUnit::Line => delta,
                egui::MouseWheelUnit::Point => delta / POINTS_PER_WHEEL_LINE,
                egui::MouseWheelUnit::Page => delta * LINES_PER_WHEEL_PAGE,
            };
            self.scroll_accum += lines;
        }
        let dx = self.scroll_accum.x.trunc();
        let dy = self.scroll_accum.y.trunc();
        self.scroll_accum.x -= dx;
        self.scroll_accum.y -= dy;
        (dx as i32, dy as i32)
    }

    /// Release every held modifier (used on focus loss) so the guest never
    /// sees a stuck Cmd/Shift/etc. after the viewer is backgrounded.
    fn release_all_modifiers(&mut self) {
        for (name, slot) in [
            ("shift", &mut self.mods_held.shift),
            ("ctrl", &mut self.mods_held.ctrl),
            ("alt", &mut self.mods_held.alt),
            ("cmd", &mut self.mods_held.cmd),
        ] {
            if *slot {
                if let Some(ks) = keymap::modifier_for_name(name, self.platform) {
                    let _ = self.input_tx.try_send(ViewerInput::Key { keysym: ks, down: false });
                }
                *slot = false;
            }
        }
    }
}

impl eframe::App for ViewerApp {
    // eframe 0.34: `ui` is the required paint method; it hands us the root
    // `Ui` (no margin/background) directly. The old `update(ctx, …)` is
    // deprecated. We get the `Context` for texture/viewport work via
    // `ui.ctx()`.
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        // Drain the latest frame under the lock, then release it before
        // touching egui (keep the critical section tiny).
        let (disconnected, status) = {
            let mut slot = self.frame.lock().expect("frame slot poisoned");
            if slot.dirty {
                slot.dirty = false;
                let dims = (slot.width, slot.height);
                let image = egui::ColorImage::from_rgba_unmultiplied(
                    [slot.width as usize, slot.height as usize],
                    &slot.rgba,
                );
                let resized = self.texture_dims != Some(dims);
                match self.texture.as_mut() {
                    Some(tex) if !resized => tex.set(image, egui::TextureOptions::default()),
                    _ => {
                        self.texture = Some(ctx.load_texture(
                            "vnc-framebuffer",
                            image,
                            egui::TextureOptions::default(),
                        ));
                    }
                }
                if resized {
                    self.texture_dims = Some(dims);
                    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                        dims.0 as f32,
                        dims.1 as f32,
                    )));
                }
            }
            (slot.disconnected, slot.status.clone())
        };

        // The root `Ui` has no background; paint it black behind the frame.
        ui.painter().rect_filled(ui.max_rect(), 0.0, egui::Color32::BLACK);

        if let (Some(tex), Some((fw, fh))) = (&self.texture, self.texture_dims) {
            // Compute the displayed image rect explicitly (letterboxed, aspect
            // preserved) so pointer→framebuffer mapping is exact, rather than
            // letting a layout helper choose the rect for us.
            let img_rect = fit_rect(ui.max_rect(), fw, fh);
            ui.painter().image(
                tex.id(),
                img_rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );

            // Forward input only when the window has focus; otherwise release
            // any held modifiers so they don't stick in the guest.
            if ctx.input(|i| i.focused) {
                let snap = ui.input(read_input_snapshot);
                for ev in self.process_input(&snap, img_rect, (fw, fh)) {
                    let _ = self.input_tx.try_send(ev);
                }
            } else {
                self.release_all_modifiers();
                self.last_pointer = None;
            }
        } else if !disconnected {
            ui.centered_and_justified(|ui| {
                ui.label(
                    egui::RichText::new("Connecting…")
                        .size(24.0)
                        .color(egui::Color32::GRAY),
                );
            });
        }

        if disconnected {
            let rect = ui.max_rect();
            let painter = ui.painter();
            painter.rect_filled(rect, 0.0, egui::Color32::from_black_alpha(160));
            let text = status.as_deref().unwrap_or("Disconnected");
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                text,
                egui::FontId::proportional(24.0),
                egui::Color32::WHITE,
            );
        }
    }
}

/// Extract the per-frame [`InputSnapshot`] from egui's `InputState`.
fn read_input_snapshot(i: &egui::InputState) -> InputSnapshot {
    let mut keys = Vec::new();
    let mut texts = Vec::new();
    let mut wheels = Vec::new();
    for event in &i.events {
        match event {
            egui::Event::Key { key, pressed, repeat, .. } => keys.push((*key, *pressed, *repeat)),
            egui::Event::Text(t) => texts.push(t.clone()),
            egui::Event::MouseWheel { unit, delta, .. } => wheels.push((*unit, *delta)),
            _ => {}
        }
    }
    InputSnapshot {
        pointer_pos: i.pointer.latest_pos(),
        primary: i.pointer.button_down(egui::PointerButton::Primary),
        secondary: i.pointer.button_down(egui::PointerButton::Secondary),
        middle: i.pointer.button_down(egui::PointerButton::Middle),
        modifiers: i.modifiers,
        keys,
        texts,
        wheels,
    }
}

/// The letterboxed rect that a `fw × fh` framebuffer occupies inside
/// `avail`, scaled to fit while preserving aspect ratio and centered.
fn fit_rect(avail: egui::Rect, fw: u32, fh: u32) -> egui::Rect {
    let scale = (avail.width() / fw as f32).min(avail.height() / fh as f32);
    let size = egui::vec2(fw as f32 * scale, fh as f32 * scale);
    egui::Rect::from_center_size(avail.center(), size)
}

/// Map an egui pointer position (logical points) to a framebuffer pixel.
/// Both the position and `img_rect` are in points and the framebuffer is
/// displayed at `img_rect / fb`, so the mapping is `(pos - min) / scale` and
/// needs no `pixels_per_point` correction. Out-of-image positions clamp to
/// the framebuffer bounds.
fn fb_pixel(pos: egui::Pos2, img_rect: egui::Rect, fb_w: u32, fb_h: u32) -> (u16, u16) {
    let sx = img_rect.width() / fb_w as f32;
    let sy = img_rect.height() / fb_h as f32;
    let fx = ((pos.x - img_rect.min.x) / sx).clamp(0.0, fb_w as f32 - 1.0);
    let fy = ((pos.y - img_rect.min.y) / sy).clamp(0.0, fb_h as f32 - 1.0);
    (fx as u16, fy as u16)
}

/// Build an RFB pointer button mask from egui's held-button booleans.
/// RFB §7.5.5: bit 0 = left, 1 = middle, 2 = right.
fn button_mask(primary: bool, secondary: bool, middle: bool) -> u8 {
    let mut mask = 0u8;
    if primary {
        mask |= 1 << keymap::button_bit::LEFT;
    }
    if middle {
        mask |= 1 << keymap::button_bit::MIDDLE;
    }
    if secondary {
        mask |= 1 << keymap::button_bit::RIGHT;
    }
    mask
}

/// Resolve a typed character to a base keysym plus whether Shift is needed
/// to produce it on the guest. Uppercase letters and US-shifted symbols
/// (via [`keymap::shifted_char_to_base`]) need Shift; other printables send
/// their Unicode scalar directly (X11 keysyms cover the Latin-1 range).
/// Control characters are dropped (Enter/Tab arrive as Key events).
fn char_to_send(ch: char) -> Option<(u32, bool)> {
    if ch.is_control() {
        return None;
    }
    if ch.is_ascii_uppercase() {
        Some((ch.to_ascii_lowercase() as u32, true))
    } else if let Some(base) = keymap::shifted_char_to_base(ch) {
        Some((base as u32, true))
    } else {
        Some((ch as u32, false))
    }
}

/// Emit a single-character key tap, momentarily correcting the physical
/// Shift state so the character is produced correctly. With Shift tracked
/// globally (for Shift+Click / Shift+Arrow), a tap whose required Shift
/// state differs from the held state toggles Shift around the keystroke and
/// restores it — so a capital typed while Shift is *not* physically held
/// still works (and vice-versa, e.g. under Caps Lock).
fn emit_char(out: &mut Vec<ViewerInput>, base: u32, needs_shift: bool, shift_held: bool, shift_ks: u32) {
    let key = |keysym, down| ViewerInput::Key { keysym, down };
    match (needs_shift, shift_held) {
        (true, false) => {
            out.push(key(shift_ks, true));
            out.push(key(base, true));
            out.push(key(base, false));
            out.push(key(shift_ks, false));
        }
        (false, true) => {
            out.push(key(shift_ks, false));
            out.push(key(base, true));
            out.push(key(base, false));
            out.push(key(shift_ks, true));
        }
        _ => {
            out.push(key(base, true));
            out.push(key(base, false));
        }
    }
}

/// X keysym for a non-text "special" key (navigation, editing, function),
/// resolved through [`keymap::key_for_name`] where possible. Returns `None`
/// for keys that produce text (letters/digits/symbols) — those flow through
/// the Text path, or [`letter_or_digit_keysym`] for chords.
fn special_keysym(key: egui::Key) -> Option<u32> {
    use egui::Key;
    let name = match key {
        Key::ArrowUp => "up",
        Key::ArrowDown => "down",
        Key::ArrowLeft => "left",
        Key::ArrowRight => "right",
        Key::Escape => "escape",
        Key::Tab => "tab",
        Key::Backspace => "backspace",
        Key::Enter => "return",
        // egui's `Delete` is forward-delete; `key_for_name("delete")` maps to
        // Backspace, so route it explicitly to forward-delete instead.
        Key::Delete => "forwarddelete",
        Key::Insert => return Some(keymap::xk::INSERT),
        Key::Home => "home",
        Key::End => "end",
        Key::PageUp => "pageup",
        Key::PageDown => "pagedown",
        Key::F1 | Key::F2 | Key::F3 | Key::F4 | Key::F5 | Key::F6 | Key::F7 | Key::F8 | Key::F9
        | Key::F10 | Key::F11 | Key::F12 | Key::F13 | Key::F14 | Key::F15 | Key::F16 | Key::F17
        | Key::F18 | Key::F19 => key.name(), // "F1".."F19" — key_for_name parses these
        _ => return None,
    };
    keymap::key_for_name(name).ok()
}

/// X keysym for a letter/digit/space key, used only for command-modifier
/// chords (Cmd/Ctrl/Alt + key). egui names letters "A".."Z", digits
/// "0".."9", and space "Space" — all handled by [`keymap::key_for_name`].
/// Returns `None` for everything else (symbols flow through the Text path).
fn letter_or_digit_keysym(key: egui::Key) -> Option<u32> {
    let name = key.name();
    let is_typing = name == "Space"
        || (name.len() == 1 && name.as_bytes()[0].is_ascii_alphanumeric());
    if is_typing {
        keymap::key_for_name(if name == "Space" { "space" } else { name }).ok()
    } else {
        None
    }
}

/// Resolve the *guest* platform for keysym mapping, the same chain the
/// `input` command uses, defaulting to macOS on any error/absent/unknown
/// value (the viewer is interactive — a bad value must not abort the window).
fn resolve_guest_platform(opts: &ConnectionOptions) -> Platform {
    resolve_platform_with_env(opts, &EnvProvider::process())
        .ok()
        .flatten()
        .and_then(|name| Platform::from_name(&name))
        .unwrap_or(Platform::Macos)
}

fn exit_resolve_error(err: ResolveError) -> ! {
    // The viewer is interactive (no --json), so report in text mode.
    print_error(
        OutputMode::Text,
        err.code(),
        &err.to_string(),
        None,
        err.details(),
        err.exit_code(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot() -> Arc<Mutex<FrameSlot>> {
        Arc::new(Mutex::new(FrameSlot::default()))
    }

    #[test]
    fn update_slot_copies_pixels_and_dims_and_flags_dirty() {
        let frame = slot();
        let rgba: [u8; 8] = [1, 2, 3, 255, 4, 5, 6, 255];
        update_slot(&frame, 2, 1, &rgba);

        let s = frame.lock().unwrap();
        assert_eq!(s.width, 2);
        assert_eq!(s.height, 1);
        assert_eq!(s.rgba, rgba);
        assert!(s.dirty, "a fresh frame must be flagged dirty for the UI");
        assert!(!s.disconnected);
    }

    #[test]
    fn update_slot_overwrites_a_smaller_previous_frame() {
        // A desktop resize hands us a frame of different length; the slot
        // must replace, not append (else the buffer/dims would disagree and
        // ColorImage::from_rgba_unmultiplied would panic in the UI).
        let frame = slot();
        update_slot(&frame, 2, 2, &[0u8; 2 * 2 * 4]);
        update_slot(&frame, 1, 1, &[9u8; 4]);

        let s = frame.lock().unwrap();
        assert_eq!((s.width, s.height), (1, 1));
        assert_eq!(s.rgba.len(), 4, "buffer length must track the new dims");
        assert_eq!(s.rgba, [9, 9, 9, 9]);
    }

    #[test]
    fn mark_disconnected_sets_status_without_a_context_dependency() {
        // We can't construct an egui::Context in a unit test, so exercise
        // the slot mutation directly (the request_repaint wake is a UI
        // concern verified live). This guards the overlay's data path.
        let frame = slot();
        {
            let mut s = frame.lock().unwrap();
            s.disconnected = true;
            s.status = Some("Connection lost: boom".to_string());
        }
        let s = frame.lock().unwrap();
        assert!(s.disconnected);
        assert_eq!(s.status.as_deref(), Some("Connection lost: boom"));
    }

    // ---- input mapping --------------------------------------------------

    use keymap::{button_bit, xk};

    /// A 200×100-point square (origin at 10,10) displaying a 100×50 guest
    /// framebuffer — scale 2.0 points/pixel, no letterboxing for simplicity.
    fn rect_2x() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(200.0, 100.0))
    }

    fn test_app() -> ViewerApp {
        let (tx, _rx) = tokio::sync::mpsc::channel::<ViewerInput>(256);
        ViewerApp::new(slot(), tx, Platform::Macos)
    }

    fn empty_snapshot() -> InputSnapshot {
        InputSnapshot {
            pointer_pos: None,
            primary: false,
            secondary: false,
            middle: false,
            modifiers: egui::Modifiers::default(),
            keys: Vec::new(),
            texts: Vec::new(),
            wheels: Vec::new(),
        }
    }

    #[test]
    fn fb_pixel_maps_corner_and_center_with_scale() {
        let r = rect_2x();
        // Top-left of the image → (0,0).
        assert_eq!(fb_pixel(egui::pos2(10.0, 10.0), r, 100, 50), (0, 0));
        // 100 points right / 2.0 = pixel 50; 50 points down / 2.0 = pixel 25.
        assert_eq!(fb_pixel(egui::pos2(110.0, 60.0), r, 100, 50), (50, 25));
    }

    #[test]
    fn fb_pixel_clamps_outside_the_image() {
        let r = rect_2x();
        // Far past the bottom-right clamps to the last pixel, never overflows.
        assert_eq!(fb_pixel(egui::pos2(9999.0, 9999.0), r, 100, 50), (99, 49));
        // Above/left of the origin clamps to (0,0).
        assert_eq!(fb_pixel(egui::pos2(-50.0, -50.0), r, 100, 50), (0, 0));
    }

    #[test]
    fn fit_rect_letterboxes_a_wide_image_in_a_square() {
        // 200×100 fb in a 100×100 area → scale 0.5, 100×50 centered.
        let r = fit_rect(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(100.0, 100.0)), 200, 100);
        assert_eq!(r.width(), 100.0);
        assert_eq!(r.height(), 50.0);
        assert_eq!(r.center(), egui::pos2(50.0, 50.0));
    }

    #[test]
    fn button_mask_packs_rfb_bits() {
        assert_eq!(button_mask(false, false, false), 0);
        assert_eq!(button_mask(true, false, false), 1 << button_bit::LEFT);
        assert_eq!(button_mask(false, true, false), 1 << button_bit::RIGHT);
        assert_eq!(button_mask(false, false, true), 1 << button_bit::MIDDLE);
        assert_eq!(
            button_mask(true, true, true),
            (1 << button_bit::LEFT) | (1 << button_bit::RIGHT) | (1 << button_bit::MIDDLE)
        );
    }

    #[test]
    fn char_to_send_classifies_shift_need() {
        assert_eq!(char_to_send('a'), Some(('a' as u32, false)));
        assert_eq!(char_to_send('A'), Some(('a' as u32, true)));
        assert_eq!(char_to_send('5'), Some(('5' as u32, false)));
        assert_eq!(char_to_send('@'), Some(('2' as u32, true))); // shifted symbol
        assert_eq!(char_to_send('?'), Some(('/' as u32, true)));
        assert_eq!(char_to_send('\n'), None); // control chars drop
    }

    #[test]
    fn emit_char_brackets_when_shift_state_differs() {
        // Capital needed but Shift not physically held → toggle Shift around.
        let mut out = Vec::new();
        emit_char(&mut out, 'a' as u32, true, false, xk::SHIFT_L);
        assert_eq!(
            out,
            vec![
                ViewerInput::Key { keysym: xk::SHIFT_L, down: true },
                ViewerInput::Key { keysym: 'a' as u32, down: true },
                ViewerInput::Key { keysym: 'a' as u32, down: false },
                ViewerInput::Key { keysym: xk::SHIFT_L, down: false },
            ]
        );
        // Lowercase wanted while Shift IS held → drop Shift around the tap.
        let mut out = Vec::new();
        emit_char(&mut out, 'a' as u32, false, true, xk::SHIFT_L);
        assert_eq!(out[0], ViewerInput::Key { keysym: xk::SHIFT_L, down: false });
        assert_eq!(out[3], ViewerInput::Key { keysym: xk::SHIFT_L, down: true });
        // States already agree → plain down/up, no Shift churn.
        let mut out = Vec::new();
        emit_char(&mut out, 'a' as u32, false, false, xk::SHIFT_L);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn special_keysym_maps_navigation_and_avoids_delete_trap() {
        assert_eq!(special_keysym(egui::Key::ArrowUp), Some(xk::UP));
        assert_eq!(special_keysym(egui::Key::Enter), Some(xk::RETURN));
        assert_eq!(special_keysym(egui::Key::Escape), Some(xk::ESCAPE));
        assert_eq!(special_keysym(egui::Key::F5), Some(xk::F1 + 4));
        // egui Delete is forward-delete (0xffff), NOT Backspace.
        assert_eq!(special_keysym(egui::Key::Delete), Some(xk::DELETE));
        assert_eq!(special_keysym(egui::Key::Backspace), Some(xk::BACKSPACE));
        // Letters/digits are not "special" — they take the Text/chord path.
        assert_eq!(special_keysym(egui::Key::A), None);
        assert_eq!(special_keysym(egui::Key::Num1), None);
    }

    #[test]
    fn letter_or_digit_keysym_covers_alnum_and_space_only() {
        assert_eq!(letter_or_digit_keysym(egui::Key::A), Some('a' as u32));
        assert_eq!(letter_or_digit_keysym(egui::Key::Num7), Some('7' as u32));
        assert_eq!(letter_or_digit_keysym(egui::Key::Space), Some(xk::SPACE));
        assert_eq!(letter_or_digit_keysym(egui::Key::Comma), None);
        assert_eq!(letter_or_digit_keysym(egui::Key::ArrowUp), None);
    }

    #[test]
    fn process_input_forwards_modifier_edges_once() {
        let mut app = test_app();
        let mut snap = empty_snapshot();
        snap.modifiers = egui::Modifiers { shift: true, ..Default::default() };
        let out = app.process_input(&snap, rect_2x(), (100, 50));
        assert_eq!(out, vec![ViewerInput::Key { keysym: xk::SHIFT_L, down: true }]);
        // Held across the next frame → no repeat down.
        let out = app.process_input(&snap, rect_2x(), (100, 50));
        assert!(out.is_empty());
        // Released → exactly one up.
        snap.modifiers = egui::Modifiers::default();
        let out = app.process_input(&snap, rect_2x(), (100, 50));
        assert_eq!(out, vec![ViewerInput::Key { keysym: xk::SHIFT_L, down: false }]);
    }

    #[test]
    fn process_input_macos_cmd_maps_to_alt_l_swap() {
        // The Cmd→XK_Alt_L Tahoe swap must come through the guest keymap.
        let mut app = test_app();
        let mut snap = empty_snapshot();
        snap.modifiers = egui::Modifiers { mac_cmd: true, ..Default::default() };
        let out = app.process_input(&snap, rect_2x(), (100, 50));
        assert_eq!(out, vec![ViewerInput::Key { keysym: xk::ALT_L, down: true }]);
    }

    #[test]
    fn process_input_types_text_but_suppresses_under_command_modifier() {
        let mut app = test_app();
        let mut snap = empty_snapshot();
        snap.texts = vec!["x".to_string()];
        let out = app.process_input(&snap, rect_2x(), (100, 50));
        assert_eq!(
            out,
            vec![
                ViewerInput::Key { keysym: 'x' as u32, down: true },
                ViewerInput::Key { keysym: 'x' as u32, down: false },
            ]
        );
        // With Ctrl held, egui would emit no Text; even if some arrived we
        // must not type it (the chord goes through the Key path instead).
        let mut app = test_app();
        let mut snap = empty_snapshot();
        snap.modifiers = egui::Modifiers { ctrl: true, ..Default::default() };
        snap.texts = vec!["x".to_string()];
        snap.keys = vec![(egui::Key::C, true, false)];
        let out = app.process_input(&snap, rect_2x(), (100, 50));
        // ctrl down edge, then the chord letter — no typed 'x'.
        assert_eq!(
            out,
            vec![
                ViewerInput::Key { keysym: xk::CONTROL_L, down: true },
                ViewerInput::Key { keysym: 'c' as u32, down: true },
            ]
        );
    }

    #[test]
    fn process_input_dedups_pointer_and_pulses_wheel() {
        let mut app = test_app();
        let mut snap = empty_snapshot();
        snap.pointer_pos = Some(egui::pos2(110.0, 60.0)); // → (50, 25)
        let out = app.process_input(&snap, rect_2x(), (100, 50));
        assert_eq!(out, vec![ViewerInput::Pointer { button_mask: 0, x: 50, y: 25 }]);
        // Same position/mask next frame → suppressed.
        let out = app.process_input(&snap, rect_2x(), (100, 50));
        assert!(out.is_empty());
        // One line of downward scroll → a transient WHEEL_DOWN down+up pulse.
        snap.wheels = vec![(egui::MouseWheelUnit::Line, egui::vec2(0.0, 1.0))];
        let out = app.process_input(&snap, rect_2x(), (100, 50));
        let down = 1 << button_bit::WHEEL_DOWN;
        assert_eq!(
            out,
            vec![
                ViewerInput::Pointer { button_mask: down, x: 50, y: 25 },
                ViewerInput::Pointer { button_mask: 0, x: 50, y: 25 },
            ]
        );
    }

    #[test]
    fn accumulate_wheel_carries_sub_line_point_remainder() {
        let mut app = test_app();
        // 30 points < one 40-point line → no whole step yet, remainder kept.
        let (_, dy) = app.accumulate_wheel(&[(egui::MouseWheelUnit::Point, egui::vec2(0.0, 30.0))]);
        assert_eq!(dy, 0);
        // Another 30 points → 60 total ≥ 40 → exactly one line, 20 carried.
        let (_, dy) = app.accumulate_wheel(&[(egui::MouseWheelUnit::Point, egui::vec2(0.0, 30.0))]);
        assert_eq!(dy, 1);
    }

    // ---- reconnect backoff (leaf 030) -----------------------------------

    #[test]
    fn backoff_escalates_then_gives_up_after_the_ceiling() {
        let mut b = Backoff::new();
        assert_eq!(b.delay(), INITIAL_BACKOFF);
        // The first MAX-1 failures escalate without giving up.
        for n in 1..MAX_CONNECT_ATTEMPTS {
            assert!(!b.note_failure(), "attempt {n} must not give up yet");
            assert_eq!(b.failures(), n);
        }
        // The MAX-th consecutive failure gives up.
        assert!(b.note_failure(), "the ceiling attempt must give up");
        assert_eq!(b.failures(), MAX_CONNECT_ATTEMPTS);
    }

    #[test]
    fn backoff_delay_doubles_and_caps_at_max() {
        let mut b = Backoff::new();
        b.note_failure(); // 500ms → 1s
        assert_eq!(b.delay(), Duration::from_secs(1));
        b.note_failure(); // → 2s
        assert_eq!(b.delay(), Duration::from_secs(2));
        b.note_failure(); // → 4s
        assert_eq!(b.delay(), Duration::from_secs(4));
        b.note_failure(); // 8s clamped to the 5s ceiling
        assert_eq!(b.delay(), MAX_BACKOFF);
        b.note_failure(); // stays clamped
        assert_eq!(b.delay(), MAX_BACKOFF);
    }

    #[test]
    fn backoff_productive_drop_resets_failures_and_delay() {
        let mut b = Backoff::new();
        b.note_failure();
        b.note_failure();
        assert!(b.failures() > 0);
        assert!(b.delay() > INITIAL_BACKOFF);
        // A session that actually streamed ended (guest reboot): full reset,
        // so a VM bounced repeatedly never exhausts the give-up budget.
        b.note_productive_drop();
        assert_eq!(b.failures(), 0);
        assert_eq!(b.delay(), INITIAL_BACKOFF);
        // And the budget is whole again afterwards.
        assert!(!b.note_failure());
    }
}
