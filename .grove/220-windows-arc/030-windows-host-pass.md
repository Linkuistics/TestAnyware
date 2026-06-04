# 030-windows-host-pass

**Kind:** work

## Goal

The Windows-host **source pass**: the `#[cfg(windows)]` facility wiring that makes
the cross-compiled `testanyware.exe` *functionally* correct (it already
*compiles* — `160` built all four triples fail-fast). Analogous to `180-linux-
host-pass`, but net-new beyond parity (the Swift CLI was macOS-only).

## Context

- **Net-new beyond parity.** The Swift CLI was `platforms: [.macOS(.v14)]`; the
  Windows arms are "backlog task 14" stubs the Rust CLI carries. The heavier of
  the two host passes (Linux mainly needed paths because `process.rs`/
  `qemu_profile.rs` already carried the Unix path; Windows has no such head start).
- **Known work items** (root BRIEF "Deferred" + `200` brief):
  - `monitor.rs` **AF_UNIX → named-pipe / TCP** (the Unix-domain-socket monitor
    channel has no direct Windows equivalent).
  - the already-`#[cfg]`-paired **`process` / `spec` / `detached` / `doctor`**
    arms — fill in the Windows side.
  - paths + any Windows-specific facility seams (cf. the EasyOCR / ffmpeg-next /
    wgpu facility pattern already anticipated by ADR-0002/0005/0006; the OCR path
    is the bundled `ocr_analyzer` venv, same as Linux).
- **Independent of `020`** (one is macOS-host golden work, one is source wiring) —
  either order; both must land before `040` (the harness verifies *this* pass's
  facilities run on the target).
- **Verification is `040`'s job, not this leaf's.** This leaf makes the facilities
  *correct*; the harness proves they *run* in-guest. A compiling `cargo-zigbuild
  --target aarch64-pc-windows-*` is the bar to *finish* this leaf; runtime-green is
  `040`.

## Done when

- All `#[cfg(windows)]` facility arms implemented (no Windows stubs/`unimplemented`
  in the host surface); `cargo-zigbuild` builds `aarch64-pc-windows-*` (and
  `x86_64-pc-windows-*`, build-verified) clean.
- The full offline surface (`cli-contract.rs`) reasoning holds for Windows — the
  contract (error codes, `--json`, `--dry-run`, help template, schema) is
  satisfied by the Windows arms.
- Anything the harness must exercise to *prove* a facility is noted for `040`.

## Notes

- Windows targets use the cross-friendly `-gnu`/`-gnullvm` variants (msvc can't
  cross from a Mac) — `200` carried-in default, ADR-0009.
- Acceptance gate: **CLI design contract**.
