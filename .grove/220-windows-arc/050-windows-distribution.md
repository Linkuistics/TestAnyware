# 050-windows-distribution

**Kind:** work

## Goal

Ship the **Windows** `testanyware` distribution: a **zip** per Windows triple
(`cargo-zigbuild`), bundling the **`ocr_analyzer` EasyOCR venv** into
`<prefix>/libexec/venv` — reusing the shared distribution machinery `210` built.

## Context

- **Trails `040`** (`200`-Q1 / root BRIEF): never ship a binary the harness has
  not run green. aarch64-windows ships once the Windows harness (`040`) is green;
  **x86_64-windows ships build-verified-only**, runtime gap **logged** (no native
  x86_64 guest here, no-silent-caps).
- **Reuses `210`'s machinery:** the same `cargo-zigbuild`-per-triple + OCR-venv-
  bundling (`resolve_interpreter()` → `<prefix>/libexec/venv`, `engine.rs:40`) +
  the ffmpeg-8 DLL staging (`040` proved the Windows libav link runs). The
  delivery format differs — **Windows zip**, not Homebrew (carried-in default,
  root BRIEF). Extends `scripts/release-build.sh` / `release-publish.sh`.
- **Local release, no CI** ([[local-release-no-ci]]) — from `scripts/` on an arm64
  Mac.
- By the time this lands, `020`/`230` have deleted the `vm-create-golden-
  {windows,linux}.sh` scripts, so the release bundle no longer ships them (it
  currently does — `release-build.sh` header).

## Done when

- `cargo-zigbuild` produces both Windows triples; the **aarch64-windows** zip (CLI
  + ffmpeg-8 DLLs + OCR venv) is the harness-green artifact; **x86_64-windows**
  build-verified, gap logged.
- The zip is publishable via `scripts/release-publish.sh`; root BRIEF distribution
  checklist updated.

## Notes

- This is the **last** Windows-arc leaf; landing it (with `040` green) lets the
  `220` node retire.
- Acceptance gate: **CLI design contract**.
