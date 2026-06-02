# 080-crosscompile-spike

**Kind:** work (feasibility spike)

## Goal

Prove or disprove the **single-Mac `zig cc` cross-build** path for the Rust
`testanyware` binary, targeting **linux x86_64** and **windows** — *before* the
Tier-2 distribution work commits to it. Fail-fast: a clear yes/no with the
blockers documented.

## Context

Distribution decision (070, Q4) is **cross-compile via `zig cc`** because the
user wants to prove it out (memory [[reference_linux_crosscheck_zig]]) rather
than build-on-target. This spike de-risks it. **Doubly load-bearing:** the
cross-compiled binaries are also what the Tier-2 **self-hosted verification**
runs (run up Linux/Windows host-VMs with TestAnyware, install the binary, test
it) — so a green spike unblocks both distribution *and* Tier-2 host verification.

- Run on **current HEAD**, which already links the two hardest native deps:
  `wgpu` (the embedded viewer — ADR-0005, Metal/Vulkan/DX12 backends) and
  `ring` (known to fail `cargo check` cross-builds on its C build — that memory
  is the *check* workaround, not a proven release link).
- `ffmpeg-next` is **not yet** in the tree (arrives with leaf `100`); add it to
  the spike matrix once the macOS encoder lands, but a successful wgpu+ring
  release link is already the strong signal.
- Releases run **locally from `scripts/`, no CI** (memory
  [[local_release_no_ci]]).

## Done when

- A documented result (in `docs/research/` or a note the `140-tier2-plan` leaf
  can read) answering: can the arm64 Mac, via `zig cc` as the C cross-compiler,
  produce a **runnable** linux-x86_64 and a windows binary of today's
  `testanyware`? Capture exact blockers (linker, sysroot, wgpu backend,
  ring/openssl, etc.) and any that are showstoppers.
- If **feasible:** sketch the `scripts/` changes Tier-2 distribution will make.
- If **infeasible:** record the fallback of record — **build-on-target via VMs**
  (TestAnyware's own goldens as build hosts) — so `140` re-plans distribution
  around it.

## Notes

- This is a spike: the output is *knowledge*, not shippable code. Don't gold-plate.
- Acceptance gate for any eventual distribution code: the **CLI design contract**
  (`docs/architecture/cli-design-contract.md`) is about command behaviour, not
  packaging — packaging has no contract bar, only "the binary runs on target".
