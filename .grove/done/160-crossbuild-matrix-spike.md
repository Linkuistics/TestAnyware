# 160-crossbuild-matrix-spike

**Kind:** work (feasibility spike)

## Goal

**Fail-fast** the residual cross-build risks before Tier-2 distribution and the
verification harness commit to them. Extend the `080` spike (which proved only
`x86_64-unknown-linux-gnu` fully + `x86_64-pc-windows-gnu` toolchain) to the
**full four-target matrix** decided in `140`, and **fold in `ffmpeg-next`** —
the one dependency `080` flagged as unproven. A clear yes/no per target with
blockers documented; the output is *knowledge*, not shippable code.

## Context

`080` (`docs/research/080-crosscompile-spike.md`) proved `cargo-zigbuild` as the
cross toolchain (supersedes the hand-rolled `zcc`; bundles zig-cc-as-C-compiler
+ glibc pinning). Two gaps remain before distribution (`140` Q2 + ADR-0009):

- **New triples, not yet link-proven.** Only x86_64-linux got a full release
  link. The matrix now commits to all four:
  - `x86_64-unknown-linux-gnu` (proven in `080`)
  - `aarch64-unknown-linux-gnu` (`080` said "same path, not built")
  - `x86_64-pc-windows-gnu` (`080`: toolchain proven, blocked at `monitor.rs`)
  - `aarch64-pc-windows-*` — **the genuinely new risk.** Use the cross-friendly
    `-gnu`/`-gnullvm` variant (msvc can't cross from a Mac); confirm
    `cargo-zigbuild` supports it and that `ring`/`wgpu` produce aarch64-windows
    rlibs.
- **`ffmpeg-next` is the one link-time risk.** Unlike `dlopen`-ed `wgpu`,
  `ffmpeg-next` links system `libav*` at link time via `pkg-config`, so it needs
  a **target sysroot** with the ffmpeg dev libs for each triple. This is the
  load-bearing question for the `170` encoder + linux/win `screen record`.
  `ffmpeg-next` is **not in the tree until `170` lands** — so either run this
  spike with a throwaway `ffmpeg-next` dep added to the graph, or **sequence the
  ffmpeg half of this spike after `170`** (see Notes). The non-ffmpeg matrix
  (all 4 triples linking today's HEAD) can be proven now.

The Windows source gap `080` found (`monitor.rs:12` unconditional
`tokio::net::UnixStream`) is **Windows-host source work (deferred)**, not a
toolchain blocker — expect the windows builds to stop there until that pass; the
spike's job is to confirm everything *up to* that point links.

## Done when

- A documented result (append to `docs/research/080-crosscompile-spike.md` or a
  sibling `160-...md`) stating, per target: does `cargo-zigbuild` produce a
  link-complete build of today's `testanyware` (modulo the known deferred
  `monitor.rs` windows gap)? Capture exact blockers.
- **`aarch64-pc-windows-*` resolved**: which variant works with `cargo-zigbuild`,
  or what blocks it.
- **`ffmpeg-next` cross-link resolved** per target: does the pkg-config/sysroot
  story work via `cargo-zigbuild` for linux x86_64+aarch64 and windows
  x86_64+aarch64? If a target sysroot is needed, record how it's obtained.
- If a target is **infeasible**, record the fallback (build-on-target via VMs,
  or drop that target) so the distribution leaf re-plans around it.
- The `scripts/` sketch in `080` updated for the four-triple reality (per-target
  SHA placeholders, `set -o pipefail`, glibc floor pin).

## Notes

- Spike discipline — don't gold-plate; knowledge, not code (cf. `080`).
- `080` warned: `… | tail …; EXIT=$?` reports the pipe's exit, not cargo's. Use
  `set -o pipefail` / `${PIPESTATUS[0]}`.
- **Sequencing note:** the ffmpeg half genuinely depends on `170` having added
  `ffmpeg-next`. Two valid orders — (a) prove the non-ffmpeg 4-triple matrix
  now, then re-run the ffmpeg half right after `170`; or (b) add a throwaway
  `ffmpeg-next` dep here to front-load the link risk. Prefer (b) if cheap, since
  fail-fast is this leaf's whole purpose. If (a), this leaf retires on the
  non-ffmpeg matrix and `170` carries the ffmpeg cross-link check.
- Acceptance gate for eventual distribution code is the **CLI design contract**
  for *behaviour*; packaging has no contract bar beyond "the binary runs".
