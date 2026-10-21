# 5. The embedded viewer: eframe/wgpu UI on the main thread, RFB on a dedicated thread

Date: 2026-06-01

## Status

Accepted

## Context

The Swift CLI's `vm start --viewer` opened an *external* interactive VNC
application through AppleScript. The Rust target is an **embedded viewer**. It
is a real RFB client that renders a live `FramebufferUpdate` stream into the
CLI's own window. It forwards mouse and keyboard input back to the guest.

The viewer is the **first long-lived RFB consumer** in the Rust CLI. So it is
the first place that drives the `testanyware-rfb` client as a continuous loop
rather than a one-shot. `screen record` is the second long-lived consumer, a
bounded one (ADR-0006). Every other command opens a short-lived connection for
each invocation (ADR-0004).

Three structural forces shaped the architecture:

1. **`RfbConnection` is async (tokio), single-owner `&mut self`.**
   `next_message().await` blocks on the socket. `key_event` and `pointer_event`
   write to it. Only one owner can hold it.
2. **eframe must own the main OS thread.** `eframe::run_native` blocks. winit
   and NSApp require the platform event loop on the main thread. On macOS, the
   primary host, this is a hard requirement.
3. **The CLI is `#[tokio::main]`.** Command dispatch is `.await`ed, and any
   worker thread may poll it. So the viewer command body is *not* guaranteed to
   run on the main thread. Nesting `eframe::run_native` inside the global
   runtime invites main-thread and nested-runtime hazards.

## Decision

**Surface.** The viewer is a dedicated top-level `testanyware viewer` command.
It resolves its VNC endpoint from the standard `--vm`, `--connect` and `--agent`
options through `resolve_vnc(&opts)`, as `screen` and `input` do. It is
long-lived and interactive. So it is neither `mutating` nor `data_producing` in
`surface.rs::CANONICAL_COMMANDS`. It emits no data envelope, so it is exempt
from `--json` and `--dry-run`.

`vm start --viewer` stays as **sugar**. After the VM is up and `vm start` emits
the `vm-start` envelope, it opens the viewer inline. It blocks until the window
closes, which is acceptable for an explicit window request. This keeps the
contract-bound `vm start` from becoming a blocking interactive command in its
canonical behaviour.

**Stack.** The viewer uses `eframe` with the `wgpu` backend. eframe includes the
machinery for the window, the winit event loop, the main thread and the run
loop. The decision takes wgpu over glow for the best HiDPI and Retina behaviour
on macOS. Its backend set, Metal, DX12 and Vulkan, also covers the Windows and
Linux roadmap. The viewer shows the framebuffer as an egui `TextureHandle`, with
partial updates for changed rectangles.

**Threading.** A **dedicated `std::thread` that runs its own current-thread
tokio runtime** owns the `RfbConnection`. eframe runs on the process main
thread. The RFB thread runs a `tokio::select!` loop:

- `readable()` → read the message with `next_message()`, copy the framebuffer
  RGBA into a shared slot, then call `egui::Context::request_repaint()`. The
  loop races only the wait for a first byte. A `select!` that dropped
  `next_message()` halfway would lose the bytes that it read.
- `input_rx.recv()` → `key_event` / `pointer_event`.
- An interval tick (or a post-update) → `request_framebuffer_update(incremental
  = true, full rect)`, which keeps the stream flowing.
- A shutdown signal (the window closed) → leave the loop and close the
  connection.

The handoff from RFB to UI is `Arc<Mutex<FrameSlot>>`, which holds the latest
RGBA, the dimensions and a dirty flag. The handoff from UI to RFB is an `mpsc`
of input events. The RFB thread holds a `Context` clone, and its
`request_repaint()` wakes eframe. So eframe does not busy-poll at 60 fps while
it waits for frames.

## Considered Options

- **Reuse the global runtime, and special-case the viewer in `main.rs`.**
  eframe would run on the main thread, and a `Handle` would spawn the RFB task
  on the existing runtime. Rejected. It still requires a `main.rs` carve-out to
  escape async dispatch. It also ties the viewer's lifetime to the CLI's global
  runtime.
- **A synchronous pump inside `update()`.** Each frame would drive the
  connection with non-blocking reads, with no background task. Rejected. It
  couples the paint rate to socket I/O. A slow read or a large rectangle can
  stall the UI thread.
- **glow instead of wgpu.** It has a lighter dependency and cross-compile
  surface. Rejected in favour of HiDPI quality and a backend set for the later
  platforms. The binary-size cost moves to the distribution leaves. The decision
  does not avoid it.

## Consequences

- One extra OS thread and a second, current-thread tokio runtime. Both are
  fully isolated from the CLI's global runtime. So there is no nested-runtime
  hazard and no main-thread carve-out in `main.rs`.
- `eframe` and `wgpu` are a large new dependency tree. Its **binary-size and
  cross-compile cost must be revisited in the distribution leaves**: Homebrew
  and the Windows zip, with releases built locally from `scripts/` and no CI.
  This ADR does not resolve that cost. It flags it.
  - **Binary size resolved (grove `120`, 2026-06-03):** the release-profile
    `testanyware` is **~15 MB** uncompressed. That build is thin-LTO with
    `strip = "symbols"`, for aarch64-apple-darwin, and wgpu uses Metal. The
    size is well within the normal Homebrew range, and the xz tarball is far
    smaller. wgpu **stays**, and no `glow` fallback is necessary. The
    cross-compile half stays open for Tier 2 (`zig cc`, grove `140`).
- `Mutex<FrameSlot>` is the default handoff. A triple buffer or
  `tokio::sync::watch` is a known escape hatch if lock contention ever appears.
  That is unlikely at VNC update rates.
- The viewer is **cross-platform**: it has no host-OS `#[cfg]` gates. The stack
  is portable, and `keymap::Platform` already maps guest keysyms. It is
  verified on macOS now. The later platform-host leaves carry the Windows and
  Linux verification.
- Three leaves deliver it in stages: a read-only render, then input forwarding,
  then auto-reconnect and the start sugar. This ADR governs all three.
