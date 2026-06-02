# 060-egui-viewer — brief

## Goal

Build the embedded **`egui` viewer** (glossary: *Embedded viewer*) — the Rust
CLI's `testanyware viewer` command and its `vm start --viewer` sugar — replacing
the Swift CLI's external AppleScript launcher. An in-process `eframe`/`wgpu`
window renders a live RFB `FramebufferUpdate` stream and forwards input to the
guest. It is the **only long-lived RFB consumer** (every other command opens a
short-lived per-invocation connection — ADR-0004) and the first continuous
driver of the `testanyware-rfb` client. Beyond Swift parity — an enhancement.

The architecture is fully decided (see *Decisions (running log)* below and
ADR-0005); the child leaves are implementation, not further design.

## Done when

- `testanyware viewer` is a working interactive viewer: renders the live
  framebuffer and forwards mouse + keyboard to the guest, verified on the macOS
  primary host against a golden VM.
- `viewer` is registered in `surface.rs::CANONICAL_COMMANDS` and satisfies the
  CLI design contract for an interactive command (no `--json`/`--dry-run`
  expected; help-text template, stable error codes).
- `vm start --viewer` opens the viewer (sugar) instead of warning that it is
  unported; the `cli-contract.rs` test passes.
- All three child leaves are complete and retired.

## Decomposition

Staged delivery (ADR-0005): each leaf is one focused session, ordered by
dependency.

- `010-viewer-render-loop` — the skeleton: `viewer` subcommand + `resolve_vnc`
  wiring, `eframe`/`wgpu` deps, the dedicated-RFB-thread + isolated-runtime
  architecture, read-only framebuffer→texture render, "disconnected" overlay +
  clean exit. Establishes everything the later leaves build on.
- `020-viewer-input-forwarding` — interactivity: egui input events → existing
  `keymap` → `key_event`/`pointer_event`, framebuffer-pixel coordinate mapping,
  focus, mouse buttons + wheel + keyboard + modifiers.
- `030-viewer-reconnect-and-start-sugar` — lifecycle polish: auto-reconnect with
  backoff (RFB thread re-`connect`s via `resolve_vnc`, "reconnecting…" overlay),
  and the `vm start --viewer` sugar wiring.

## Pointers

- ADR a session here must read: `docs/adr/0005-embedded-viewer-eframe-and-
  dedicated-rfb-thread.md` (governs all three leaves); `docs/adr/0004-*` for the
  short-lived-vs-long-lived RFB framing.
- Glossary terms in play: *Embedded viewer*, *Shared-VNC server* (the retired
  thing it is NOT), *CLI design contract* (see `CONTEXT.md`).
- Key code seams the leaves touch:
  - `testanyware-rfb`: `RfbConnection::{connect, next_message,
    request_framebuffer_update, framebuffer, key_event, pointer_event}`,
    `Framebuffer::rgba()`, `keymap::{key_for_name, resolve_modifiers,
    mouse_button_bit_for_name, ScrollComponent, Platform}`.
  - `testanyware-cli`: `resolve::{resolve_vnc, ConnectionOptions, ResolvedVnc}`,
    `surface.rs::CANONICAL_COMMANDS`, `commands/vm.rs` (the `--viewer` no-op stub
    at the warning, to be replaced by the sugar in leaf 030).

## Decisions (running log)

**Q1 — Scope & staging: interactive, staged in two work leaves.** Target is a
fully interactive viewer (mouse + keyboard), matching what the Swift
AppleScript launcher delivered via an external VNC app — a read-only-only
viewer would be a capability *regression* dressed as an enhancement. Delivery
is staged: **leaf A** builds the read-only render loop (window, framebuffer →
texture, repaint, connection lifecycle) and makes the connection-ownership /
threading decision *input-aware*; **leaf B** adds input forwarding (egui event
→ keymap → RFB write, framebuffer-pixel coordinate mapping, focus). The input
layer is not new work — `RfbConnection::{key_event,pointer_event}` plus
`keymap::{key_for_name,resolve_modifiers,mouse_button_bit_for_name,
ScrollComponent}` already power every `input *` command; leaf B *wires* them.

**Q2 — Invocation surface: dedicated `testanyware viewer` subcommand, canonical;
`vm start --viewer` demoted to sugar.** The viewer is a new top-level command
that resolves its VNC endpoint via the standard `--vm`/`--connect`/`--agent`
options through `resolve_vnc(&opts)` — the same family as `screen` and `input`,
not a flag on a lifecycle command. Rationale is the CLI design contract: `vm
start` is `mutating + data_producing` (emits a `vm-start` envelope and exits); a
window that blocks until close contradicts that. The dedicated `viewer` command
is long-lived + interactive, so it is naturally exempt from `--json`/`--dry-run`
(no data envelope). `vm start --viewer` survives as **sugar**: after the VM is up
and the start envelope is emitted, it opens the viewer inline (blocking-until-
close is acceptable for an explicit window request). New entry needed in
`surface.rs::CANONICAL_COMMANDS` for `viewer` (neither mutating nor
data-producing; no `schema_id`).

**Q3 — Rendering stack: `eframe` with the `wgpu` backend.** Batteries-included
eframe solves the window / winit event loop / main-thread / app run-loop
machinery; wgpu (eframe's default) is chosen over glow for best HiDPI/Retina
behaviour on the macOS primary host and a more future-proof backend set
(Metal/DX12/Vulkan) for the Windows/Linux roadmap, accepting the larger
dependency and wider cross-compile surface. The framebuffer is displayed as an
egui `TextureHandle` (partial updates via `set_partial` for changed rects).
**Forward-pointer:** the wgpu binary-size / cross-compile cost must be revisited
in the distribution leaves (Homebrew + Windows zip, local release from
`scripts/`, no CI) — note it there, don't re-litigate here.

**Q4 — Threading model: dedicated RFB thread + isolated current-thread runtime;
eframe on the main thread (ADR-worthy).** eframe `run_native` owns the main OS
thread (winit/NSApp requirement); the viewer command spawns a **dedicated
`std::thread`** running its **own current-thread `tokio` runtime** that owns the
`RfbConnection` and a `tokio::select!` loop:
  (a) `next_message()` → copy framebuffer RGBA into the shared slot + call
      `ctx.request_repaint()`;
  (b) `input_rx.recv()` → `key_event` / `pointer_event`;
  (c) interval tick (or post-update) → `request_framebuffer_update(incremental
      = true, full rect)` to keep the stream flowing;
  (d) shutdown signal (window closed) → break.
This fully isolates the viewer's async from the CLI's global `#[tokio::main]`
runtime — no nested-runtime hazard, no `main.rs` carve-out to escape async
dispatch — at the cost of one extra thread + a second runtime. Handoff: UI ←
RFB via `Arc<Mutex<FrameSlot>>` (latest RGBA + dims + dirty flag); UI → RFB via
an `mpsc` of input events; `egui::Context::request_repaint()` (a `Context`
clone held by the RFB thread) is the wake mechanism so eframe does not
busy-poll. **This decision earns an ADR** (hard to reverse, surprising, real
trade-off). Default handoff is `Mutex<FrameSlot>`; a triple-buffer / `watch`
is a noted perf escape hatch if lock contention ever shows (unlikely at VNC
update rates).

**Q5 — Connection-drop behavior: overlay + clean exit in leaf A; auto-reconnect
deferred.** On a `next_message()`/`connect` error, leaf A shows a "disconnected"
status overlay and clean-exits when the window closes — no auto-reconnect — so
the first read-only leaf stays one focused session. Auto-reconnect (RFB thread
loops back through `resolve_vnc` + `RfbConnection::connect` with backoff, UI
shows "reconnecting…") is additive and lands in a dedicated later leaf (leaf C).

**Q6 — Platform reach: cross-platform code, macOS-verified now.** The viewer
needs no host-OS `#[cfg]` gates — eframe/wgpu/winit are portable and the only
platform-sensitive concern (keysym mapping for input) is already handled by
`keymap::Platform`, keyed on the *guest* OS not the host. Write it portable from
the outset; verify on the macOS primary host now; Windows/Linux verification
rides the later platform-host leaves. (Contrast the OCR seam, which genuinely
needs per-platform native facilities — the viewer does not.)

**Q7 — Encoding/perf dependency: none blocking.** Node 040 already landed ZRLE +
Tight in the RFB client, and the connection negotiates them automatically
(ZRLE-preferred, Tight, Raw fallback — `connection.rs` SetEncodings). The viewer
inherits whatever is negotiated with no viewer-side work; Raw/CopyRect is a fine
correctness baseline and the compressed encodings are a free bandwidth win. Not
a prerequisite, not a leaf.

## Verification log

- **Leaf 020 (input forwarding) live-verified 2026-06-02** on the macOS
  primary host against a fresh `testanyware-golden-macos-tahoe` clone via
  `testanyware viewer --vm <id>`. Click (Spotlight opened from a menu-bar
  click), typing (`agent snapshot` read the Spotlight field back as the exact
  typed string, incl. digits + `-`), pointer-move (hover tooltip), and scroll
  (correct direction, not inverted) all land in the guest. The read-only
  render half (leaf 010) is confirmed live by the same run. So the node's
  macOS done-when is met for 010+020; only leaf 030 (reconnect + start sugar)
  remains before the node can retire.

## Notes

`egui`/`eframe` is a new top-level dependency — weigh it against the
local-release-from-`scripts/` model (no CI; binary size; cross-compile matrix in
`Cargo.toml`). Relates to backlog "task 8" (viewer) in old code comments —
descriptions only, not status.
