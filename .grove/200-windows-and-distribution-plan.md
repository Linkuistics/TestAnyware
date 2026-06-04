# 200-windows-and-distribution-plan

**Kind:** planning

## Goal

Plan and decompose the **remaining Tier-2 wave** of `port-swift-cli-to-rust`,
now that the Linux-host arc is runtime-green (nodes `160`–`190` retired; the
Linux aarch64 host CLI passes all three harness bands incl. EasyOCR). Grill the
sequencing and materialize the next work leaves/nodes. The grove's "Done when"
(root BRIEF) still needs the platform/distribution backlog complete.

## What remains (root BRIEF Tier-2 "Deferred")

Four chunks, with real ordering constraints — the grilling settles the order:

1. **linux/win distribution** — `cargo-zigbuild` per triple; Homebrew formula
   for Linux + a Windows zip (`080` sketched `scripts/`; releases run locally on
   an arm64 Mac, no CI — [[local-release-no-ci]]). **Linux distribution is
   UNBLOCKED right now** (Linux host-pass + harness are green; "never ship a
   binary the harness hasn't run green" is satisfied for linux-aarch64). Must
   bundle the **`ocr_analyzer` EasyOCR daemon venv** (`vision/stages/text-ocr`)
   into `<prefix>/libexec/venv` — that is the Linux/Windows OCR path
   (`resolve_interpreter()` in `testanyware-ocr-client`), and the macOS bundle
   ships none today (macOS uses native Vision). **x86_64-linux ships
   build-verified-only** (no native x86_64 runtime check here — log the gap).

2. **Windows-host pass** — the cfg/paths/facility wiring analogous to
   `180-linux-host-pass`: `monitor.rs` AF_UNIX→named-pipe/TCP, and the already-
   `#[cfg]`-paired `process`/`spec`/`detached`/`doctor`. Net-new beyond parity
   (the Swift CLI was macOS-only).

3. **Windows verification harness** — **reuses `190`'s machinery verbatim**: the
   in-process host→golden TCP forward, host-gateway discovery, the band-agnostic
   `run_band` driver, the `--agent`/`--vnc` endpoint targeting, *and* the
   `ocr_analyzer` daemon + venv recipe. The factored `ProvisionChannel` trait
   (`linux-host-harness.rs`) gets a **2nd impl** (ssh → in-VM agent `file
   upload`/`exec`, since Windows ships no sshd); plus a Windows HUT image.
   **Depends on a Windows golden + a working Windows in-VM agent** (the agent is
   a separate, out-of-scope workstream — confirm its readiness in the grilling).

4. **linux/win `vm create-golden`** — full Rust port reusing `110`'s russh layer
   (macOS-host work, no cross binary needed). Builds on the same `testanyware-vm`
   foundation the macOS golden creation (node `110`) already proved.

## Sequencing seeds for the grilling

- **Distribution trails its OS's host-pass + harness** (root BRIEF) — so
  **Linux distribution can proceed now**; Windows distribution waits on chunks
  2+3.
- **Windows trails Linux** and depends on the Windows golden + agent — the
  hardest external dependency. Worth front-loading a feasibility check (is the
  Windows agent green? is a Windows golden kept-built or creatable?).
- A natural decomposition: a distribution node (Linux first, Windows later) and
  a Windows node (host-pass → harness), with `vm create-golden` slotted by
  dependency. The grilling decides whether to interleave or do Linux-distribution
  first as the cheapest unblocked win.

## Open questions to grill

- Priority: ship **Linux distribution** first (unblocked, cheapest), or drive
  **Windows-host** first (longest pole, external deps)?
- Windows agent/golden readiness — does the harness's reuse assumption hold, or
  does the agent gap block the Windows harness?
- Does `vm create-golden` for linux/win need its own ADR (reusing ADR-0007/0008
  russh+recovery), or is it a straight port?

## Pointers

- Root BRIEF Tier-2 section (decomposition + the "Deferred" list this plans).
- ADR-0009 (harness), the retired `140-tier2-plan` (how the Linux wave was
  decomposed — the template for this one), `done/180-linux-host-pass` (the
  host-pass template Windows mirrors), `done/190-linux-verification-harness/`
  (the harness machinery + reuse seam).
- `vision/stages/text-ocr` (the `ocr_analyzer` daemon distribution must bundle).
- Glossary terms: Self-hosted verification harness, Host-under-test (HUT) VM,
  In-VM agent, Golden image, Host CLI.
