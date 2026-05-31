# 050-live-vm-gate

**Kind:** work

## Goal

Stand up a **live-VM verification gate**: an end-to-end check that drives a real
golden VM and asserts the RFB client + input layer behave correctly against a
real VNC server. This is the capstone that proves the headless stack built up
to here — and it **closes the live Vision-OCR check deferred by the prior wave's
`040-macos-vision-ocr` leaf**.

## Context

What this gate must exercise against a running guest:
- **Input layer**: `input click`/`key`/`type`/`drag`/`scroll` land where
  expected (via the existing short-lived RFB connections).
- **`agent show-menu`** (leaf 030): opening a menu path via VNC click actually
  opens the menu in the guest.
- **Encodings** (leaf 040): force the server to use ZRLE and Tight (via
  `SetEncodings` preference) and assert `screen capture` output matches the Raw
  path — i.e. the new decoders are correct against a *real* server, not just
  synthetic fixtures.
- **Vision OCR live check**: run `screen find-text` against a real macOS guest
  framebuffer and confirm the in-process Vision engine (ADR-0003) returns
  expected text — the check ADR-0002/040 left deferred.

## Context pointers

- VMs are cheap per task: golden images are kept built, so the gate just
  clones + starts one (see the `vm-costs` memory — clone+start only). Use the
  existing `vm start`/`vm stop` lifecycle.
- VM interaction gotchas: see the `vm-ssh-from-harness` and `tart-ip-lies`
  memories — backgrounded SSH breaks askpass auth; use `tart list` state not
  `tart ip`; `COPYFILE_DISABLE` for extraction.
- Cmd-key mapping for macOS VNC input: `cmd-key-tahoe` memory
  (Command = `XK_Alt_L`, Option = `XK_Meta_L`) — relevant if asserting
  menu/keyboard shortcuts.
- Decide the **harness shape** at bootstrap: a `#[ignore]`d integration test
  gated on an env flag (e.g. `TESTANYWARE_LIVE_VM=1`) vs. a `scripts/` runner.
  Lean toward an ignored integration test so it lives with `cli-contract.rs`
  and runs locally on demand (no CI — see `local-release-no-ci`).

## Done when

- A live-VM gate exists that, against a freshly-cloned golden VM, verifies:
  input landing, `show-menu` opening a menu, ZRLE+Tight capture correctness, and
  live Vision OCR — each as a checkable assertion, not a manual eyeball.
- It is runnable locally on an arm64 Mac on demand, clearly documented (how to
  invoke, which platform/golden it needs), and skipped by default in `cargo
  test` so the normal suite stays VM-free.
- The prior wave's deferred live Vision-OCR check is recorded as closed here.

## Notes

This gate does **not** cover the egui viewer (060) — the viewer is interactive
and verified separately. The gate's scope is the *headless* automatable stack.
If the four checks prove too big for one session, decompose by check.
