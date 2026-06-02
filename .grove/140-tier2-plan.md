# 140-tier2-plan

**Kind:** planning

## Goal

Re-grill and decompose **Tier 2** — the Linux/Windows additive (beyond-parity)
work — shaped by Tier-1 outcomes. Kept lazy (grove constraint 4) until Tier 1
lands, because Tier 2's shape genuinely depends on Tier-1 results.

## Context

Tier 2 is **net-new capability** the Swift CLI never had (Swift = macOS-only),
but it is **self-verifiable**: TestAnyware runs up **Linux and Windows host-VMs**,
installs the **locally cross-compiled** (`zig cc`) host binary, and tests it
there. A host binary inside a guest drives a VM's agent/RFB endpoint over the
network, so the non-`vm-start` surface needs no nested virt. (This supersedes the
`070` "unverifiable in this env" framing.)

Inputs to grill against (all from Tier 1):
- `080` **cross-compile spike** outcome — feasible via `zig cc`, or fall back to
  build-on-target via VMs? This shapes the whole distribution leaf.
- `100` **`VideoEncoder` seam** — does the `ffmpeg-next` encoder drop in cleanly?
- `110` **golden port** shape — how much of the macOS golden orchestration
  generalizes to the linux/windows scripts (`vm-create-golden-{linux,windows}.sh`,
  587 + 514 lines).

Tier-2 items to decompose:
- **Linux-host support** (cross-platform pass): paths + `#[cfg]` facility wiring.
  **Lighter than Windows** — `process.rs`/`qemu_profile.rs` already carry the
  *Unix* path; the EasyOCR / ffmpeg-next / wgpu-on-Vulkan facilities are already
  anticipated (ADR-0002/0005/0006). Memory [[rust-port-conditional-facilities]].
- **Windows-host support** (cross-platform pass): process spawning, paths, the
  `#[cfg]` facility seams (`qemu_profile.rs`, `process.rs` stubs — "backlog task
  14"). The heavier of the two host passes.
- **Self-hosted verification harness**: run up Linux/Windows guests via
  TestAnyware, install the cross-compiled host binary, smoke-test the
  non-`vm-start` surface against a VM endpoint. Decide what a "host-under-test"
  VM is (vanilla guest vs reuse of the agent golden) and how the endpoint is
  provided. `vm start`/lifecycle-in-guest (nested virt) only if cheap.
- **`ffmpeg-next` encoders** for linux/windows `screen record` (ADR-0006 seam).
- **linux/windows `vm create-golden`** (full Rust port, per Q3).
- **linux/windows distribution** (Homebrew Linux + Windows zip), shaped by `080`.

## Done when

- Tier-2 leaves/nodes materialized with clear briefs (via
  `grove-llm leaf-add`/`leaf-insert`).
- Sequencing decided — Linux-host (lighter) likely before Windows-host, and
  **whether the host passes and distribution interleave** (the open question the
  root brief flagged); the self-hosted verification harness gates "done" for both.
- ADRs raised only where hard-to-reverse/surprising.
- The self-hosted verification approach concretized (host-under-test VM shape,
  endpoint provisioning, which surface is smoke-tested vs compile-only).

## Notes

- Grill one question at a time, recommended answer per question (`grilling.md`,
  `driving.md`). Best in a **fresh session**.
- Acceptance gate for resulting work leaves stays the **CLI design contract**.
