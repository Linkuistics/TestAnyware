# 030-recovery-sip-tcc-finalize — brief

## Goal

Build the **re-engineered recovery automation** (node decision Q3), the SIP
disable/enable cycle, the TCC sqlite grants, the final agent-health gate, and
the clean shutdown + `tart clone` to the golden image. Finishes node `110` and
deletes the source script `provisioner/scripts/vm-create-golden-macos.sh`.

This sub-node was a planning leaf; its design grilling (2026-06-03) is recorded
below and in **ADR-0008** (`docs/adr/0008-golden-recovery-automation-rfb-ocr.md`).
The design is settled — the two child leaves are work tasks.

## Done when (sub-node)

- `vm create-golden --platform macos` end-to-end produces a golden functionally
  equivalent to the script: agent LaunchAgent healthy on 8648, both TCC grants
  present with `auth_value=2`, SIP re-enabled, SSH off — **verified by actually
  creating a golden on the Mac** ([[vm-costs]]).
- `--dry-run` describes the full 5-boot plan without mutating; contract codes +
  help template satisfied; `cli-contract.rs` passes.
- `provisioner/scripts/vm-create-golden-macos.sh` **deleted**; any release
  bundling reference updated.

## Decomposition

- `010-recovery-driver` (work, high-risk) — the `RecoverySession` primitives +
  `recovery_boot_csrutil(cmd)`. Live-VM verified standalone by running
  `csrutil disable` and confirming `csrutil status` over SSH, before any finalize
  code exists. The riskiest part of the whole node; budget live-VM iteration.
- `020-tcc-and-finalize` (work, mechanical SSH) — the top-level SIP/TCC
  orchestration, TCC sqlite grants, health gate, clean shutdown, clone to golden,
  end-to-end wiring of `run_vm_create_golden`, script deletion, `cli-contract.rs`.

## Decisions (running log)

Settled in the 030 design grilling (2026-06-03). Durable record is ADR-0008;
this log is the conversation trail.

- **Q1 — driver shape: imperative observe/act/verify helpers, not a generic
  FSM.** The recovery sequence is a fixed linear path (no branching, no cycles,
  no revisited states), so a data-driven state-machine engine would be
  indirection the path never exercises (the "runaway tree"/over-engineering
  anti-pattern). "State machine" stays a description of the *per-step pattern*,
  not a literal engine. Build a `RecoverySession` wrapping the `RfbConnection` +
  `OcrEngine` exposing `wait_for_text(query, deadline) -> Located`, `act(input)`,
  `settle(quiet, deadline)`, and `verify_transition(predicate, deadline)`; the
  recovery flow reads as a straight-line script of these calls, parity with the
  bash structure but each blind `sleep` replaced by a signal-driven wait. The
  frame-refresh + OCR step recomposes the proven `screen find-text` pipeline
  (`screen.rs:142`: `request_framebuffer_update` → drain `next_message` →
  `encode_png` → `OcrEngine::recognize` → `find_text`); **extract that shared
  helper** rather than duplicate it.

- **Q2 — csrutil prompt sync: OCR-prompt primary + screen-settle fallback +
  authoritative result/SSH backstop.** The script's blind `sleep 15/10/10/15`
  (lines 545–563) is replaced by `wait_for_text` on each prompt that prints text
  (the "proceed? [y/n]" prompt, the admin name/password labels, and the final
  `System Integrity Protection is …` result line). The single genuinely
  signal-less micro-gap — password entry echoes nothing in the terminal — uses
  **screen-settle** (framebuffer quiesces for a short window) with a bounded
  floor, not a long fixed sleep. The final step is `wait_for_text` on the
  csrutil result line, and on a miss the whole csrutil interaction is **retried**
  (a robustness gain the blind-sleep script lacks). The authoritative correctness
  gate stays the existing post-reboot `csrutil status` check over SSH (script
  683/717) — so the in-recovery waits need not be perfect. Design constraint:
  keep OCR-vs-settle swappable (which is *primary* may flip during live
  iteration if OCR proves flaky on small recovery-Terminal monospace), so
  `wait_for_text` and `settle` are independent primitives, not one fused call.

- **Q3 — decompose into two work leaves** (`010-recovery-driver`,
  `020-tcc-and-finalize`); the two halves are separately verifiable (the
  strongest split signal). See Decomposition above.

- **Q4 — write ADR-0008** (`docs/adr/0008-golden-recovery-automation-rfb-ocr.md`).
  All three ADR tests hold: hard to reverse (template for Tier-2 recovery
  flows; script deleted), surprising without context ("why not port the
  sleeps?"), genuine trade-off (re-engineer-vs-transliterate was a user override
  — node Q3). Written this session.

## Pointers

- Source script (parity reference): `provisioner/scripts/vm-create-golden-macos.sh`
  — `_recovery_boot_csrutil` (398–594), `grant_tcc_permissions` (604–671),
  top-level SIP/TCC cycle (677–723), finalize (725–818).
- Existing infra: `testanyware-vm` (`tart.rs::parse_vnc_url`/`run_detached`/
  `poll_ip`, `ssh.rs::SshSession`, `golden.rs::{provision_boot1, SetupVm,
  cleanup_setup_vm}`, `health.rs::wait_for_agent`, `process.rs`),
  `testanyware-rfb` (`RfbConnection`, `input.rs`), `testanyware-ocr-client`
  (`OcrEngine`, `find_text`). The reusable capture pipeline lives in
  `commands/screen.rs` (`encode_png`, the find-text poll loop) — extract for reuse.
- VNC keysym quirks [[cmd-key-tahoe]] (Command = `XK_Alt_L`) apply on the
  recovery RFB path, as for `input *`.
- Memory: [[vm-costs]] (live verification is cheap), [[tart-ip-lies]] (use
  `tart list` state, not `tart ip`).

## Notes

- A failed recovery automation leaves the setup VM in a known state
  (SSH-reachable post-reboot), so re-runs are debuggable.
- `vm create-golden` is macOS-host-only (`#[cfg(target_os = "macos")]`).
