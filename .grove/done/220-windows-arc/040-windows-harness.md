# 040-windows-harness

**Kind:** work

## Goal

Run the **self-hosted verification harness** GREEN for **Windows aarch64**:
provision the Windows agent-golden HUT with the cross-compiled `testanyware.exe`
over the in-VM agent channel, forward a real tart macOS golden's endpoint through
the host, and run the three-band smoke suite. **Reuses `190`'s machinery
verbatim** — the only net-new code is the 2nd `ProvisionChannel` impl + a Windows
HUT.

## Context

- **Depends on `020` (windows golden HUT) and `030` (the functionally-correct
  windows binary).** This is where `030`'s `#[cfg(windows)]` facilities are first
  proven to *run* on the target (dynamic loader, OCR daemon, libav link, RFB
  client) — a green cross-*build* is not proof it runs (ADR-0009).
- **Reuse seam (built in `190`, see `done/190-linux-verification-harness/
  BRIEF.md`):**
  - **Swapped:** the `ProvisionChannel` — Linux `ssh` (russh) → **Windows in-VM
    agent `/upload` + `/exec`** (Windows ships no sshd). The trait was factored in
    `190`'s `linux-host-harness.rs` precisely so this leaf writes only the 2nd
    impl. The HUT image swaps stock-Ubuntu → the Windows agent-golden.
  - **Shared unchanged:** the in-process host→golden TCP forward, host-gateway
    discovery, the band-agnostic `run_band` driver, `--agent`/`--vnc` endpoint
    targeting, and the `ocr_analyzer` daemon + venv recipe (`190/030`).
- **Three-band surface** (ADR-0009): endpoint-free (caps/schema/llm-instructions/
  doctor/--help/dry-runs); endpoint-driven (agent HTTP, `input *`, `screen
  capture`/`size`/`record`→mp4); OCR (`screen find-text` via the bundled EasyOCR
  venv). Same bands `190` ran green on Linux.
- **Arch:** aarch64-windows gets full in-guest smoke (QEMU+swtpm Win11 ARM64, the
  only Windows guest this Mac boots natively); **x86_64-windows build-verified
  only**, gap logged (no-silent-caps).
- **The libav load-time-dependency lesson applies** (`190` BRIEF "CRITICAL"): the
  Windows binary `NEEDED`s the ffmpeg-8 DLLs — they must be staged beside the
  binary or even `--help` won't exec. Stage them in provisioning like `190` did.

## Done when

- The Windows harness (`cli-rs/crates/testanyware-cli/tests/windows-host-harness.rs`
  or an extension of the factored harness) runs **all three bands green** on
  aarch64-windows: opt-in/env-gated + `#[ignore]` like `linux-host-harness.rs`
  (`TESTANYWARE_WINDOWS_HARNESS=1`), pure helpers unit-tested offline.
- x86_64-windows build-verified-only gap logged (harness doc-comment + root BRIEF).
- The provisioning-channel trait now has both impls; note any reuse-seam
  refinements back into the `220` BRIEF.

## Notes

- This is the **gate** for the Windows-host work — `050` (distribution) trails it.
- Acceptance gate: **CLI design contract**.
