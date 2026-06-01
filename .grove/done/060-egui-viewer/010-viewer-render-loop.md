# 010-viewer-render-loop

**Kind:** work

## Goal

Build the viewer skeleton and a working **read-only** render: a `testanyware
viewer` command that opens an `eframe`/`wgpu` window and displays the guest's
live framebuffer. This leaf establishes the architecture every later viewer leaf
builds on (ADR-0005); input forwarding (020) and reconnect/sugar (030) come
later.

## Done when

- `testanyware viewer` resolves its endpoint via `resolve_vnc(&opts)` (standard
  `--vm`/`--connect`/`--agent` options) and registers in
  `surface.rs::CANONICAL_COMMANDS` (neither `mutating` nor `data_producing`, no
  `schema_id`).
- `eframe` + `wgpu` are added to `testanyware-cli`'s `Cargo.toml`.
- A **dedicated `std::thread` running its own current-thread tokio runtime**
  owns the `RfbConnection` and runs the `tokio::select!` loop: `next_message()`
  → copy RGBA into `Arc<Mutex<FrameSlot>>` + `ctx.request_repaint()`; interval
  tick → `request_framebuffer_update(incremental=true, full rect)`; shutdown
  signal → break. eframe runs on the main thread (see surface/main-thread note
  below).
- The eframe `update()` uploads the latest framebuffer to an egui
  `TextureHandle` (partial update for changed rects where practical) and draws
  it; the window resizes to the framebuffer dimensions (DesktopSize honoured).
- On connection drop/error, a "disconnected" status overlay is shown; closing
  the window tears down the RFB thread cleanly (shutdown signal, join).
- Verified on the macOS primary host against a golden VM: the window shows the
  live desktop and tracks updates.

## Notes

- **Main-thread escape:** `eframe::run_native` must run on the process main
  thread, but the CLI is `#[tokio::main]` so the `viewer` command body isn't
  guaranteed to be on it. Resolve this (e.g. route `viewer` so eframe owns the
  main thread); ADR-0005 chose the dedicated-RFB-thread model specifically so
  the *only* thing that must be main-thread is eframe itself.
- `FrameSlot` = latest RGBA `Vec<u8>` + width/height + dirty flag. `Mutex` is
  the default; triple-buffer/`watch` is a noted escape hatch, not needed now.
- Input forwarding is **out of scope here** — but design the thread/channel
  shape input-aware (the `mpsc<ViewerInput>` UI→RFB channel can be stubbed/empty
  now so 020 only adds the producer + mapping, not a re-architecture).
- No host-OS `#[cfg]` gates (cross-platform per ADR-0005 / Q6).
