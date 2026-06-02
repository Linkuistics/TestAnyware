# 070-next-wave

**Kind:** planning

## Goal

Materialize the **next (and likely final) wave** of the port grove. The first
waves (agent parity, doctor, screen find-text, server-retire, show-menu, macOS
Vision OCR, RFB encodings, live-VM gate, egui viewer) are all built & retired.
This planning task grills the remaining root-brief arc into ordered work
leaves/nodes and decides their sequencing. The grove's "Done when" (full
contract parity + distribution + `cli/` deleted) is reached when this wave's
leaves are all retired.

## Context

Verified against `cli-rs` HEAD (2026-06-02), the genuinely-pending arc items
(code-confirmed, not just stale checkboxes) are:

1. **`screen record`** — still `unimplemented!("screen record")` in `main.rs`
   (canonical + `record` alias). Root brief mandates **embedded libav
   (`ffmpeg-next`), not a subprocess.** Largest single unknown: codec/container
   choice, the `screen-record` schema already declared in `surface.rs`, and how
   it drives the long-lived RFB stream (cf. the viewer — another continuous
   consumer; reconcile against ADR-0004 short-lived-vs-long-lived).
2. **Windows-host support** — `backlog task 14`, stubbed in
   `qemu_profile.rs` / `process.rs` (usable defaults, not real support). The
   cross-platform pass: process spawning, paths, and the `#[cfg]` facility
   seams already established for OCR.
3. **`vm create-golden --platform <p>` subcommand** — not in `surface.rs`.
   Retires the external `vm-create-golden-*.sh` scripts. NOTE: `scripts/` shows
   **no golden scripts present now** — reconcile where golden creation currently
   lives (were they already removed? is it manual?) before porting. See memory
   [[project_golden_creation_in_cli]].
4. **Distribution** — Homebrew (macOS + Linux) + Windows zip. Releases run
   **locally from `scripts/` on an arm64 Mac, no CI** (memory
   [[local_release_no_ci]]). Must revisit the `eframe`/`wgpu` binary-size &
   cross-compile cost flagged by ADR-0005. Linux cross-check via `zig cc`
   (memory [[reference_linux_crosscheck_zig]]).
5. **Final parity verification → delete `cli/`** (100 Swift files still present)
   + de-transition `CONTEXT.md` (drop the "in transition" framing). The grove's
   terminal step; gated on the `cli-contract.rs` full surface passing and a
   parity sweep.

Carried-over follow-up (not a wave item, but must not be lost): **leaf 030
viewer live GUI verification** on the macOS host (window opens; bounce VM
reconnects; `vm start --viewer`). Promoted into the root brief.

## Done when

- Each pending item above is either a materialized work leaf/node with a clear
  brief, or an explicit decision to defer/drop it (recorded in the root brief).
- Sequencing is decided. Recommended ordering to grill against:
  **(a)** `screen record` (last contract-parity stub; closes "no
  `unimplemented()`"), **(b)** `vm create-golden` (depends on golden-layout
  reconciliation), **(c)** Windows-host pass, **(d)** distribution (depends on
  everything compiling cross-platform), **(e)** final parity + delete `cli/`
  (strictly last). Grill whether Windows-host and distribution interleave.
- Open scope questions resolved with the user: is `screen record`'s embedded-
  libav still required, or is that decision revisitable? Is Windows-host in
  scope for *this* grove or a sibling? Does `vm create-golden` cover all three
  platforms or just the ones with kept-built goldens?
- The tree is grown (leaves/nodes added via `grove-llm leaf-add`/`leaf-insert`);
  ADRs raised only where a decision is hard-to-reverse/surprising (e.g.
  `screen record` codec/container; distribution packaging).

## Notes

- This is a **planning** task: open with grilling (`grilling.md`), one question
  at a time, recommended answer per question; update `CONTEXT.md` inline as
  terms resolve; write a PRD only at a genuine agreement point.
- The acceptance gate for every resulting work leaf is the **CLI design
  contract** (`docs/architecture/cli-design-contract.md`).
- Best done in a **fresh session** (`grove do port-swift-cli-to-rust`) for a
  clean grilling context — this leaf is the durable agenda either way.
