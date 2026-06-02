# 120-macos-distribution

**Kind:** work

## Goal

Distribute the **Rust** `testanyware` on macOS via **Homebrew (arm64)** — revise
the `scripts/` release pipeline, which currently builds the **Swift** bundle.

## Context

- `scripts/release-build.sh` today builds `testanyware (host CLI, Swift,
  arm64-apple-darwin)` + the in-VM agents + `vm-*.sh` + `helpers/*`, and renders
  `scripts/templates/testanyware.rb.tmpl`. `release-publish.sh` creates the
  GitHub release + pushes the formula to the tap.
- Rework for the Rust binary: build `cli-rs` (`cargo build --release`,
  arm64-apple-darwin, native — easy), regenerate version-from-git for
  `--version` (Rust equivalent of the `Version.swift` dance), keep bundling the
  in-VM agents + `helpers/*`, and **drop the retired `vm-*.sh`** — `vm
  start/stop/list/delete` are already ported and `vm create-golden` is built into
  the binary (leaf `110`).
- Revisit the **binary-size** cost ADR-0005 flagged for `wgpu` (and note `ffmpeg`
  is *not* in the macOS build per ADR-0006).
- Update `testanyware doctor`'s tool floors (the `# testanyware-min-tool:` lines
  in the release script) to the Rust toolchain.
- Releases run **locally on an arm64 Mac, no CI** (memory [[local-release-no-ci]]).

## Done when

- `scripts/release-build.sh` produces a Rust `testanyware` arm64 tarball + a
  rendered Homebrew formula; `release-publish.sh` (release + tap push) path works.
- `brew install` from the tap yields a working `testanyware` on a clean-ish macOS.
- `doctor` floors and any Swift-specific release logic updated/removed.

## Notes

- This is **Tier-1 (macOS) distribution only** — native arm64. Linux/Windows
  distribution (cross-compile via `zig cc`, gated on the `080` spike) is Tier 2,
  planned by `140`.
