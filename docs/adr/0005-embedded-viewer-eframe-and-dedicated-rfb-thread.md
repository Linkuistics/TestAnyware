# 5. The embedded viewer: eframe/wgpu UI on the main thread, RFB on a dedicated thread

Date: 2026-06-01

## Status

Accepted

## Context

The Swift CLI's `vm start --viewer` launched an *external* interactive VNC
application via AppleScript. The Rust target is an **embedded viewer**: a real
RFB client rendering a live `FramebufferUpdate` stream into the CLI's own
window and forwarding mouse/keyboard back to the guest. It is the **only
long-lived RFB consumer** in the Rust CLI — every other command opens a
short-lived per-invocation connection (ADR-0004) — so it is the first place the
`testanyware-rfb` client is driven as a continuous loop rather than a one-shot.

Three structural forces shaped the architecture:

1. **`RfbConnection` is async (tokio), single-owner `&mut self`.**
   `next_message().await` blocks on the socket; `key_event`/`pointer_event`
   write to it. Only one owner can hold it.
2. **eframe must own the main OS thread.** `eframe::run_native` blocks, and
   winit/NSApp require the platform event loop on the main thread (hard
   requirement on macOS, the primary host).
3. **The CLI is `#[tokio::main]`.** Command dispatch is `.await`ed and may be
   polled on any worker thread, so the viewer command body is *not* guaranteed
   to run on the main thread, and nesting `eframe::run_native` inside the global
   runtime invites main-thread and nested-runtime hazards.

## Decision

**Surface.** The viewer is a dedicated top-level `testanyware viewer` command
that resolves its VNC endpoint via the standard `--vm`/`--connect`/`--agent`
options through `resolve_vnc(&opts)` — the same family as `screen` and `input`.
It is long-lived and interactive, hence neither `mutating` nor `data_producing`
in `surface.rs::CANONICAL_COMMANDS` and exempt from `--json`/`--dry-run` (it
emits no data envelope). `vm start --viewer` is retained as **sugar**: after the
VM is up and the `vm-start` envelope is emitted, it opens the viewer inline
(blocking-until-close is acceptable for an explicit window request). This keeps
the contract-bound `vm start` from being turned into a blocking interactive
command as its canonical behaviour.

**Stack.** `eframe` with the `wgpu` backend. Batteries-included eframe solves
the window / winit event loop / main-thread / run-loop machinery; wgpu is chosen
over glow for best HiDPI/Retina behaviour on macOS and a future-proof backend
set (Metal/DX12/Vulkan) for the Windows/Linux roadmap. The framebuffer is shown
as an egui `TextureHandle` with partial updates for changed rectangles.

**Threading.** A **dedicated `std::thread` running its own current-thread tokio
runtime** owns the `RfbConnection`. eframe runs on the process main thread. The
RFB thread runs a `tokio::select!` loop:

- `next_message()` → copy the framebuffer RGBA into a shared slot, then call
  `egui::Context::request_repaint()`;
- `input_rx.recv()` → `key_event` / `pointer_event`;
- an interval tick (or post-update) → `request_framebuffer_update(incremental =
  true, full rect)` to keep the stream flowing;
- a shutdown signal (window closed) → break and tear down.

Handoff is `Arc<Mutex<FrameSlot>>` (latest RGBA + dimensions + dirty flag) for
UI ← RFB, and an `mpsc` of input events for UI → RFB. `request_repaint()` from a
`Context` clone held by the RFB thread is the wake mechanism, so eframe does not
busy-poll at 60 fps waiting for frames.

## Considered Options

- **Reuse the global runtime, special-case the viewer in `main.rs`** to run
  eframe on the main thread and spawn the RFB task on the existing runtime via a
  `Handle`. Rejected: it still requires a `main.rs` carve-out to escape async
  dispatch, and entangles the viewer's lifetime with the CLI's global runtime.
- **Synchronous pump inside `update()`** — drive the connection with
  non-blocking reads each frame, no background task. Rejected: couples paint
  rate to socket I/O and risks stalling the UI thread on a slow read or a large
  rectangle.
- **glow instead of wgpu** — lighter dependency and cross-compile surface.
  Rejected in favour of HiDPI quality and backend future-proofing; the
  binary-size cost is deferred to the distribution leaves, not avoided.

## Consequences

- One extra OS thread and a second (current-thread) tokio runtime, fully
  isolated from the CLI's global runtime — no nested-runtime hazard, no
  main-thread carve-out in `main.rs`.
- `eframe`/`wgpu` are a substantial new dependency tree. Its **binary-size and
  cross-compile cost must be revisited in the distribution leaves** (Homebrew +
  Windows zip; releases built locally from `scripts/`, no CI). This ADR does not
  resolve that cost — it flags it.
- `Mutex<FrameSlot>` is the default handoff; a triple-buffer / `tokio::sync::
  watch` is a known escape hatch if lock contention ever surfaces (unlikely at
  VNC update rates).
- The viewer is written **cross-platform** (no host-OS `#[cfg]` gates — the
  stack is portable and guest-keysym mapping is already handled by
  `keymap::Platform`), verified on macOS now; Windows/Linux verification rides
  the later platform-host leaves.
- Delivery is staged across three leaves (read-only render → input forwarding →
  auto-reconnect + start-sugar); this ADR governs all three.
