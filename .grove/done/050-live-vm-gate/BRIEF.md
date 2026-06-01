# 050-live-vm-gate — brief

## Goal

Stand up a **live-VM verification gate**: an end-to-end check that drives a real
golden VM and asserts the headless stack — RFB client, input layer, encodings,
and OCR — behaves correctly against a real VNC server + in-VM agent. This is the
capstone that proves the stack built up to here, and it **closes the live
Vision-OCR check deferred by the `040-macos-vision-ocr` leaf**.

## Why this node has children (decomposition decision, 2026-05-31)

The leaf was written assuming the gate could just `vm start` a cheap kept-built
golden and run four checks. Bootstrapping it exposed two unbuilt prerequisites,
so the work was decomposed (direction chosen by the user: *port tart runner
first, then gate*):

1. **The Rust CLI can only drive a QEMU Windows guest today.**
   `vm start --platform macos` returns `BackendUnsupported`
   (`testanyware-vm/src/lifecycle.rs:126`) — the **tart runner is unported**.
   `--platform linux` would use QEMU but there is **no Linux qcow2 golden**
   present; only `testanyware-golden-windows-11.qcow2` exists. The cheap,
   kept-built goldens the gate is meant to use (`testanyware-golden-linux-24.04`,
   `testanyware-golden-macos-tahoe`) are **tart** VMs the Rust CLI cannot drive.
   → **`010-tart-runner`** ports the tart backend so the gate can clone+start
   those goldens. (This pulls forward the root-brief "tart runner" checklist
   item; that item is now owned here — do not also do it standalone.)

2. **The ZRLE/Tight check needs encoding-override plumbing that doesn't exist.**
   The `SetEncodings` preference list is hard-coded (`testanyware-rfb/src/
   connection.rs:164`, ZRLE > Tight > CopyRect > Raw). There is no flag/env to
   force the server to use ZRLE-only or Tight-only, so the gate cannot make the
   server exercise each decoder and diff against Raw without new code.
   → **`020-rfb-encoding-override`** adds that mechanism.

3. **The macOS Vision OCR check is host-side, not blocked on a macOS guest.**
   `OcrEngine::detect()` returns the in-process Vision engine on any macOS host
   (`testanyware-ocr-client/src/engine.rs:79`); `screen find-text` captures a
   guest framebuffer over VNC then OCRs it locally. So the OCR check needs *a
   running guest showing known text* on this arm64 Mac — it does not require the
   guest to be macOS. (Targeting the macOS tart golden is still natural once
   `010` lands, since show-menu/menu-bar checks benefit from a macOS guest.)

The gate itself — env-gated integration test + lifecycle helper + the four
checks — is **`030-gate-harness-and-checks`**, which depends on both `010` and
`020`. If those four checks prove too big for one session, `030` decomposes
further by check.

## Children

- `010-tart-runner` — port `vm start/stop/list/delete` via the `tart` CLI on
  macOS so the Rust CLI can drive the kept-built Linux/macOS tart goldens.
- `020-rfb-encoding-override` — a flag/env to force the RFB client to advertise
  a single encoding (ZRLE-only / Tight-only / Raw-only) for `screen capture`.
- `030-gate-harness-and-checks` — the `#[ignore]`d, env-gated live-VM gate and
  its four checks.

## Done when

- A live-VM gate exists that, against a freshly-cloned golden VM, verifies:
  input landing, `show-menu` opening a menu, ZRLE+Tight capture correctness, and
  live Vision OCR — each a checkable assertion, not a manual eyeball.
- It is runnable locally on an arm64 Mac on demand, clearly documented (how to
  invoke, which platform/golden it needs), and skipped by default in `cargo
  test` so the normal suite stays VM-free.
- The deferred live Vision-OCR check is recorded as closed.

## Context pointers

- VMs are cheap per task: golden images are kept built, so the gate just
  clones + starts one (`vm-costs` memory — clone+start only).
- VM interaction gotchas: `vm-ssh-from-harness`, `tart-ip-lies` memories —
  backgrounded SSH breaks askpass auth; use `tart list` state column not
  `tart ip`; `COPYFILE_DISABLE` for extraction.
- Cmd-key mapping for macOS VNC input: `cmd-key-tahoe` memory
  (Command = `XK_Alt_L`, Option = `XK_Meta_L`) — relevant to menu/keyboard
  shortcut assertions on a macOS guest.
- Harness shape: an `#[ignore]`d integration test gated on `TESTANYWARE_LIVE_VM=1`,
  living beside `cli-contract.rs`, runnable locally on demand (no CI — see
  `local-release-no-ci`).

## Notes

This node does **not** cover the egui viewer (060) — verified separately. Scope
is the *headless* automatable stack.
