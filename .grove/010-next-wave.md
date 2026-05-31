# 010-next-wave

**Kind:** planning

## Goal

The first wave (010-agent-action-parity, 020-port-doctor,
030-screen-find-text, 040-macos-vision-ocr) is complete and retired into
`done/`. Decide and sequence the **next wave** of leaves from the root
BRIEF's "Remaining-work checklist", then grow the tree (`leaf-add` /
`leaf-decompose`) so the following sessions have concrete work tasks.

This is a planning task: open with a grilling session (one question at a
time, recommend an answer for each) to agree the next wave's scope and
order, update `CONTEXT.md` inline as terms resolve, and raise ADRs only for
hard-to-reverse choices.

## Context

Remaining backlog (from the root `BRIEF.md`, verified against `cli-rs/`
HEAD as of 040's completion). Items cluster by **shared capability
dependency**, which is how the BRIEF says to order them:

**Command-parity gaps still open (`unimplemented()` stubs):**
- `server` — the hidden `_server` internal command (OCR bridge). Closest
  to the just-finished OCR work; the `OcrChildBridge` daemon pattern is the
  scaffold it builds on (see the `ocr-bridge-is-scaffold-not-residue`
  memory). Likely the lowest-friction next leaf.
- `agent show-menu` — blocked on the VNC-input layer (opens menu items via
  VNC click); pairs with the VNC work, not standalone.
- `screen record` — needs embedded libav (`ffmpeg-next`), not a subprocess.
  ADR-0003's pure-Rust-objc2 precedent bears on the macOS path
  (AVAssetWriter).

**Platform / facilities (not yet materialized):**
- VNC viewer with `egui` to replace the AppleScript launcher (`--viewer`).
- ZRLE + Tight encodings for the RFB client crate.
- tart runner for the macOS-host-macOS-guest path
  (`#[cfg(target_os = "macos")]`).
- Windows-host support (cross-platform pass).
- Live-VM verification gate for the RFB client + input layer (also closes
  040's deferred live Vision-OCR check).

**Distribution / finish:**
- `vm create-golden --platform <p>` subcommand, retiring the external
  `vm-create-golden-*.sh` scripts.
- Distribute via Homebrew (macOS + Linux) and Windows zip. Releases run
  locally from `scripts/` on an arm64 Mac (no CI; see the
  `local-release-no-ci` memory).
- Final parity verification -> **delete `cli/`** and de-transition
  `CONTEXT.md`.

## Done when

- The next wave's leaves exist in `.grove/` (via `leaf-add`, or
  `leaf-decompose` if a leaf is too big for one session), in an agreed
  order grounded in shared-capability clustering.
- Any cross-cutting decisions made during grilling are captured (CONTEXT.md
  inline; an ADR only if hard-to-reverse).
- This planning leaf is committed and retired.

## Notes

Open sequencing question to grill: does the next wave tackle the
**vision/OCR-adjacent** thread (`server` _server bridge -> live-VM OCR
gate), the **VNC-capability** thread (input layer -> `show-menu` -> ZRLE/
Tight -> egui viewer), or the **distribution** thread (golden subcommand ->
Homebrew/zip -> delete `cli/`) first? Recommend starting with `server`
(smallest, builds directly on the OCR bridge just touched) unless the user
prioritises a user-facing capability.
