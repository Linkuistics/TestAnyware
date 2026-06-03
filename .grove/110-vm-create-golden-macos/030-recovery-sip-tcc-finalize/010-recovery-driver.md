# 010-recovery-driver

**Kind:** work

## Goal

Build the **in-process recovery automation** (ADR-0008): the `RecoverySession`
observe/act/verify primitives and the `recovery_boot_csrutil(cmd)` function that
boots the setup VM into macOS Recovery over VNC and runs a `csrutil` command by
driving the framebuffer with RFB + OCR. This is the high-risk, novel part of the
node; it is built and hardened standalone before any finalize code exists.

## Context

Re-engineers `provisioner/scripts/vm-create-golden-macos.sh::_recovery_boot_csrutil`
(lines 398–594) per node decision Q3 / ADR-0008. The macro sequence is fixed
(parity); the micro mechanism replaces blind sleeps with signal-driven waits.

Builds on what leaves `010`/`020` of node `110` already landed: the `russh`
`SshSession` (`testanyware-vm/src/ssh.rs`), `golden.rs` (`provision_boot1`,
`SetupVm { id, pid, ip, key_path }`, `cleanup_setup_vm`), and `tart.rs`
(`parse_vnc_url`, `run_detached`, `poll_ip`, `vm_exists`).

Reusable pieces to compose (do NOT reinvent):
- `RfbConnection::connect(host, port, password)` + `request_framebuffer_update`
  + `next_message` (drive the framebuffer), `framebuffer().rgba()`,
  `key_event`/`pointer_event`, and the `input.rs` extension trait
  (`type_text`, `press_key`, `key_down_named`/`key_up_named`, `mouse_down`/
  `mouse_up`/`mouse_move`, `drag`).
- `testanyware-ocr-client`: `OcrEngine::detect()`, `recognize(png) -> Vec<OcrDetection>`,
  `find_text(query, &detections) -> FindOutcome`, `OcrDetection::centre()`.
- The capture pipeline currently inlined in `commands/screen.rs::run_screen_find_text`
  (`encode_png` + the poll-frame→OCR→match loop): **extract a shared helper**
  (e.g. `testanyware-vm` or a small shared module) so the recovery driver and
  `screen find-text` use one implementation. Decide the home during the work;
  keep `screen find-text` behaviour unchanged.

VNC keysym quirk [[cmd-key-tahoe]]: Command = `XK_Alt_L` (0xffe9) on the
Virtualization.framework VNC path — applies here as for `input *`.

## Design (from ADR-0008 / node Q1, Q2)

`RecoverySession` wraps an `RfbConnection` + `OcrEngine`, primitives:
- `wait_for_text(query, deadline) -> Located` — pump a fresh frame → OCR →
  `find_text`; retry with backoff until match or deadline. Returns the matched
  detection's centre for click targeting.
- `act(input)` — RFB key/type/pointer (thin wrapper over the input trait).
- `settle(quiet, deadline)` — wait until the framebuffer stops changing for
  `quiet` (compare successive `rgba()` snapshots / a cheap hash); bounded by
  `deadline`. The proxy for signal-less gaps; **independent** from `wait_for_text`
  so primacy can flip during live tuning.
- `verify_transition(predicate, deadline)` — confirm the screen changed as
  expected after an act.

`recovery_boot_csrutil(setup: &SetupVm, cmd: &str)` — straight-line script:
1. Graceful stop of the running setup VM (System Events shut down over SSH, with
   force-stop fallback — script `_stop_vm_graceful`, lines 331–347).
2. `tart run <setup> --recovery --no-graphics --vnc-experimental` detached;
   parse the `vnc://[:pass@]host:port` line from the run log (`parse_vnc_url`).
3. `RfbConnection::connect`; `wait_for_text` the framebuffer is live.
4. Startup picker: `wait_for_text("Options")` → Right, Right, Return →
   `verify_transition` to the recovery desktop (`wait_for_text("Utilities")`).
5. Open Terminal: `wait_for_text("Utilities")` → `mouse_down` (hold to open the
   menu, bypassing the modal) → `wait_for_text("Terminal")` → `mouse_move` →
   `mouse_up` → `verify_transition` Terminal is frontmost.
6. Run csrutil: `act(type cmd)`, Return → OCR-primary prompt sync per Q2
   (proceed → y, name → user, password → pass [settle for the masked entry] →
   Return) → `wait_for_text("System Integrity Protection is")`, retry the whole
   interaction on a miss.
7. `act(type "halt")`, Return; wait for the tart process to exit (force-stop
   fallback).
8. Reboot normally (`tart run --no-graphics --vnc-experimental` detached);
   wait for SSH (reuse `poll_ip` + `SshSession::connect_key`).

## Done when

- `recovery_boot_csrutil` exists with the `RecoverySession` primitives, behind
  `#[cfg(target_os = "macos")]`, in `testanyware-vm`.
- The shared capture helper is extracted; `screen find-text` still passes its
  existing tests (behaviour unchanged).
- **Live-VM fail-fast verification:** against a freshly `provision_boot1`-ed
  setup VM, `recovery_boot_csrutil(setup, "csrutil disable")` drives recovery
  end-to-end and, after the normal reboot, `csrutil status` over SSH reports
  **disabled**. Record the run (cf. node `110` leaf verify logs). [[vm-costs]]
- Pure logic (VNC-URL parse reuse, prompt-step sequencing where extractable,
  settle hashing) has unit tests; `cargo test -p testanyware-vm` green; `zig cc`
  cross-check unaffected (russh path unchanged; this is macOS-cfg'd anyway).

## Notes

- A failed run leaves the setup VM SSH-reachable post-reboot, so iteration is
  debuggable. Keep the setup VM around across attempts where possible.
- Do not wire `run_vm_create_golden` to the full pipeline here — that, plus TCC
  and finalize, is `020-tcc-and-finalize`. This leaf may expose
  `recovery_boot_csrutil` and leave it called only from a test/harness.
