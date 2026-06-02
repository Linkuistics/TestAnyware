# 140-tier2-plan

**Kind:** planning

## Goal

Re-grill and decompose **Tier 2** — the Linux/Windows additive (beyond-parity)
work — shaped by Tier-1 outcomes. Kept lazy (grove constraint 4) until Tier 1
lands, because Tier 2's shape genuinely depends on Tier-1 results.

## Context

Tier 2 is **net-new capability** the Swift CLI never had, and is **unverifiable
in this environment** (no Windows host; no kept-built Linux/Windows goldens — the
live-VM gate is macOS/tart only). "Done" for these = compiles cross-platform +
best-effort smoke; live verification is a recorded known gap.

Inputs to grill against (all from Tier 1):
- `080` **cross-compile spike** outcome — feasible via `zig cc`, or fall back to
  build-on-target via VMs? This shapes the whole distribution leaf.
- `100` **`VideoEncoder` seam** — does the `ffmpeg-next` encoder drop in cleanly?
- `110` **golden port** shape — how much of the macOS golden orchestration
  generalizes to the linux/windows scripts (`vm-create-golden-{linux,windows}.sh`,
  587 + 514 lines).

Tier-2 items to decompose:
- **Windows-host support** (cross-platform pass): process spawning, paths, the
  `#[cfg]` facility seams (`qemu_profile.rs`, `process.rs` stubs — "backlog task
  14"). Memory [[rust-port-conditional-facilities]].
- **`ffmpeg-next` encoders** for linux/windows `screen record` (ADR-0006 seam).
- **linux/windows `vm create-golden`** (full Rust port, per Q3).
- **linux/windows distribution** (Homebrew Linux + Windows zip), shaped by `080`.

## Done when

- Tier-2 leaves/nodes materialized with clear briefs (via
  `grove-llm leaf-add`/`leaf-insert`).
- Sequencing decided — in particular **whether Windows-host and distribution
  interleave** (the open question the root brief flagged).
- ADRs raised only where hard-to-reverse/surprising.
- The unverifiable-in-env known gap explicitly recorded per item.

## Notes

- Grill one question at a time, recommended answer per question (`grilling.md`,
  `driving.md`). Best in a **fresh session**.
- Acceptance gate for resulting work leaves stays the **CLI design contract**.
