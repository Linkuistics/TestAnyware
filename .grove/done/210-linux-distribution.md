# 210-linux-distribution

**Kind:** work

## Goal

Ship the **Linux** `testanyware` distribution: `cargo-zigbuild` per Linux triple,
a Homebrew formula for Linux (Linuxbrew), bundling the **`ocr_analyzer` EasyOCR
daemon venv** into `<prefix>/libexec/venv` — the Linux/Windows OCR path. This is
the **first unblocked win** of the remaining Tier-2 wave (Linux host-pass +
harness are green, `190` retired) and it **builds the shared distribution
machinery** (zigbuild-per-triple, OCR-venv bundling, formula rendering) that the
Windows distribution (`220/050`) later reuses.

## Context

- **Unblocked:** "never ship a binary the harness has not run green" is satisfied
  for **linux-aarch64** (`190` GREEN — all three bands incl. EasyOCR `screen
  find-text`). So Linux distribution may proceed now (`200`-Q1).
- **Extends the existing macOS release path, doesn't replace it.** `scripts/
  release-build.sh` already builds the macOS `aarch64-apple-darwin` tarball and
  renders `scripts/templates/testanyware.rb.tmpl` (node `120`). This leaf adds the
  **Linux** triples: `aarch64-unknown-linux-gnu` (first-class, harness-green) and
  `x86_64-unknown-linux-gnu` (**build-verified-only**, runtime gap **logged** —
  no native x86_64 guest on this Mac, ADR-0009 no-silent-caps).
- **Cross-build:** `cargo-zigbuild` per triple (supersedes hand-rolled `zcc`; the
  `080`/`160` spikes proved the matrix, incl. the `ffmpeg-next` system-libav link
  — see `docs/research/170-ffmpeg-cross-link.md`). The Linux binary carries hard
  `NEEDED libav*.so` (ffmpeg 8 sonames) — the **BtbN ffmpeg-8 `gpl-shared` `.so`
  bundle** must ship beside the binary (rpath `$ORIGIN`), exactly as `190`'s
  harness staged at run time (`190` BRIEF "CRITICAL — libav is a load-time dep").
- **OCR venv bundle (the novel bit):** macOS ships **none** (native Vision), but
  Linux/Windows need the EasyOCR `ocr_analyzer` daemon. `resolve_interpreter()`
  (`cli-rs/crates/testanyware-ocr-client/src/engine.rs:40`) resolves
  `<prefix>/libexec/venv/bin/python` relative to `<prefix>/bin/testanyware`. The
  daemon source is `vision/stages/text-ocr/src/ocr_analyzer` (a `pyproject.toml`
  project pulling torch). The formula/bundle must materialize that venv at
  `<prefix>/libexec/venv` — the same venv `190/030` provisioned at run time.
- **Local release, no CI** ([[local-release-no-ci]]) — runs from `scripts/` on an
  arm64 Mac; do not reintroduce GitHub Actions.

## Done when

- `cargo-zigbuild` produces both Linux triples; the **aarch64-linux** bundle (CLI
  + ffmpeg-8 `.so`s + OCR venv) installs via the Homebrew formula and **runs on a
  Linux aarch64 host** including `screen find-text` (OCR via the bundled venv) —
  verify against the same Ubuntu ARM64 HUT the `190` harness uses ([[vm-costs]]).
- **x86_64-linux** is build/link-verified and the runtime gap is **logged** where
  a reader sees it (release doc + root BRIEF Tier-2 checklist x86_64 line).
- The Linux Homebrew formula renders from a template (extend
  `testanyware.rb.tmpl` or a sibling) and is publishable via
  `scripts/release-publish.sh`.
- Root BRIEF distribution checklist line updated; CONTEXT.md gets a distribution
  glossary entry only if a term genuinely needs pinning.

## Notes

- Decision context: `200` running log Q1 (Linux-first) + carried-in defaults
  (four triples, aarch64 first-class, x86_64 build-verified, Linux=Homebrew).
- Acceptance gate: the **CLI design contract** stays the bar for any command
  behavior touched.
