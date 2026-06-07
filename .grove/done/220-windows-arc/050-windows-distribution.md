# 050-windows-distribution

**Kind:** work

## UNGATED — `215-docker-host-unification` reported REJECT (2026-06-07)

The hoisted docker host-unification spike (`215`, formerly `240`) **rejected**
containerizing the host binary (`docs/research/240-docker-host-unification.md`) —
it fails the host-side-framebuffer gate (ADR-0010) on macOS/Windows and adds
native surface on Windows. So the **native cross-compiled Windows host binary
this leaf distributes is NOT being replaced** — ship it. This leaf is unblocked.

**OCR-venv scope removed.** This leaf previously assumed bundling the
`ocr_analyzer` EasyOCR venv into `<prefix>/libexec/venv` (reusing `210`'s Linux
recipe). That is **impossible on win-arm64** (EasyOCR/opencv has no `win_arm64`
wheel) and the spike confirmed the fix is *not* an in-host venv. **Ship the
OCR-less 2/3-green surface here**; the Windows OCR engine is decided separately in
`060-windows-ocr-band` (containerized Linux EasyOCR vs native `Windows.Media.Ocr`
vs accept-the-gap) as an additive band. `screen find-text` is documented
unsupported on win-arm64 until `060` lands an engine.

## Goal

Ship the **Windows** `testanyware` distribution: a **zip** per Windows triple
(`cargo-zigbuild`) — CLI + ffmpeg-8 DLLs (the `040`-proven libav link), **no OCR
venv** (see above) — reusing the shared distribution machinery `210` built.

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
  + ffmpeg-8 DLLs, **no OCR venv**) is the harness-green artifact; **x86_64-windows**
  build-verified, gap logged.
- The zip is publishable via `scripts/release-publish.sh`; root BRIEF distribution
  checklist updated.

## Notes

- Once the **last** live Windows-arc leaf (this + `060-windows-ocr-band`) lands,
  the `220` node retires. `050` and `060` are independent — `060` (OCR band) does
  not block this distribution.
- Acceptance gate: **CLI design contract**.

## Pre-publish gate (deferred-in from `210` inbox, 2026-06-04)

`210` proved the Linux bundle's RUNPATH self-location, the EasyOCR venv recipe,
and `resolve_interpreter` on a real aarch64-linux HUT via the
`linux_dist_install_layout` harness test — but **never ran the formula's literal
`brew install`**. The wheel-only `easyocr` pin uses
`resource("easyocr").cached_download` (standard but unexercised), and Homebrew's
keg symlink / RUNPATH relocation of the binary is untested. **Before the first
Linux `gh release` + tap push**, do one real `brew install` of a `file://`-URL
rendering of `testanyware.rb` in a Linuxbrew aarch64 HUT and run `testanyware
screen find-text` green.

**This same gate applies to the Windows zip** — but the Windows delivery is a
**zip, not Homebrew**, so the keg-relocation half doesn't transfer; what *does*
transfer is "exercise the actual published artifact (unzip + run from a clean
prefix), not just the in-tree layout test." Land that real-artifact smoke as
part of this leaf's `Done when`, and (separately) close the open **Linux**
`brew install` gate before publishing the Linux release.
