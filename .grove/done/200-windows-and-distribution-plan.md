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

## Feasibility probe (2026-06-04, this session)

- **Windows agent control surface exists in code:** `agents/windows/SystemEndpoints.cs`
  exposes `/exec`, `/upload`, `/download` — the `ProvisionChannel` the harness's
  2nd impl needs. Agent cross-builds from this Mac (`dotnet -r win-arm64
  --no-self-contained`), installed via `provisioner/autounattend/` into a Win11
  ARM64 golden. Harness reuse assumption holds *in code*; agent *runtime*-green is
  the separate workstream.
- **No Windows golden kept-built** (`tart list`: only `testanyware-golden-linux-24.04`
  + `testanyware-golden-macos-tahoe`; Windows = QEMU+swtpm, not tart). Both
  `provisioner/scripts/vm-create-golden-{linux,windows}.sh` still exist — exactly
  what chunk 4 ports.
- **Distribution sketch present:** `scripts/release-{build,doctor,publish}.sh` +
  `scripts/templates` (the `080` sketch).
- **Dependency insight:** the Windows harness HUT *is* the Windows agent-golden
  (ADR-0009 / 140-Q4), so **`vm create-golden --platform windows` gates the Windows
  harness** — chunk 4(win) precedes chunk 3, not parallel. The **Linux** harness
  uses a *stock* Ubuntu image (ADR-0009), so the **linux golden has no downstream
  gate in this wave** — independent, low-urgency macOS-host work.

## Decisions (running log — 200 grilling, 2026-06-04)

- **Q1 — top-level sequencing: Linux-distribution first, then the Windows arc.**
  Linux distribution is fully unblocked (Linux host-pass + harness green, `scripts/`
  sketch exists) and is the cheapest win; doing it first also de-risks the **shared
  distribution machinery** (cargo-zigbuild per triple, OCR-venv bundling into
  `libexec/venv`, the Homebrew formula) that Windows distribution later reuses. Then
  open the Windows critical path (golden → host-pass → harness → win-dist), with
  `vm create-golden` slotted by dependency. Rejected: Windows-arc-first (front-loads
  the longest pole but risks stalling on external agent/golden readiness with no
  shipped value); fully-parallel Linux-dist + Windows-golden (throughput vs focus —
  one-task-per-session makes the split costly, and golden isn't on Linux's path).

- **Q2 — `vm create-golden` slotting: split by dependency.** The Windows golden is
  the harness HUT (ADR-0009), so it gates chunk 3; the Linux golden gates nothing in
  this wave (Linux harness uses a stock Ubuntu image, 140-Q4). So the **Windows golden
  port is a leaf inside the Windows arc** (HUT prerequisite), and the **Linux golden
  port is a standalone low-urgency leaf** (`230`) with loose timing — land it any time
  before grove-finish. Both reuse node-110's russh/recovery/finalize/qemu machinery;
  the *whether/how-verified* is already settled (140 carried-in: full Rust port mirroring
  110, live-verified by creating each golden on this Mac). Rejected: one unified golden
  node before the arc (couples independent linux-golden timing to the windows critical
  path); linux-golden-explicitly-last (no benefit over "loose").

- **Q3 — Windows readiness: front-load a cheap fail-fast spike (arc leaf `010`).**
  Before any Rust port, run the *existing* `provisioner/scripts/vm-create-golden-windows.sh`
  to create a Win11 ARM64 golden, boot it, and confirm the agent reaches health and
  `/exec`/`/upload`/`/download` respond — the channel the harness's 2nd `ProvisionChannel`
  impl needs. Zero Rust; mirrors the `160` fail-fast spike. If the agent/golden is broken,
  the whole Windows arc is blocked on the (out-of-scope) agents workstream and we learn it
  for ~1 cheap session ([[vm-costs]]) instead of after porting golden + host-pass. Rejected:
  fold readiness into the golden-port leaf (discovers a broken agent only after investing in
  the Rust port); defer to the harness leaf (latest discovery, highest rework risk).

- **Q4 — `vm create-golden` ADR: no new ADR up front.** Linux golden is a tart-based
  straight port under ADR-0007 (ssh-via-russh) + ADR-0008 (recovery-over-RFB/OCR). The
  Windows golden diverges (no SSH → autounattend unattended-install + the agent's
  `/exec`/`/upload` channel; QEMU+swtpm not tart), but that approach is already
  established in `provisioner/scripts/vm-create-golden-windows.sh` + `provisioner/
  autounattend/` — porting *documents* it, not decides it. The windows-golden leaf
  captures the agent/autounattend provisioning model inline (CONTEXT.md/brief) and raises
  an ADR only if a genuinely new trade-off surfaces (grove constraint 4 — lazy ADRs).
  Rejected: pre-committing a Windows-golden ADR (premature — no open decision yet).

- **Carried-in defaults (root BRIEF + ADR-0009, not re-grilled — re-asking would be
  theatre):** distribution matrix = **four triples** (`x86_64`/`aarch64` ×
  `linux-gnu`/`windows-{gnu,gnullvm}`); **aarch64 first-class** (harness-green),
  **x86_64 ships build-verified-only with the runtime gap logged** (no native x86_64
  guest on this Mac — no-silent-caps); **Linux = Homebrew formula, Windows = zip**.
  These shape the leaf briefs as defaults; a work leaf may revisit if something surfaces.

## Materialized tree (this session)

```
210-linux-distribution.md          leaf  — UNBLOCKED NOW (cheapest win; builds the
                                            shared dist machinery Windows reuses)
220-windows-arc/                    node  — the Windows critical path
  010-windows-readiness-spike.md    leaf  — fail-fast: existing .sh golden + agent smoke
  020-vm-create-golden-windows.md   leaf  — Rust port (HUT prerequisite for the harness)
  030-windows-host-pass.md          leaf  — #[cfg] facility wiring (monitor.rs etc.)
  040-windows-harness.md            leaf  — reuse 190 machinery + 2nd ProvisionChannel
  050-windows-distribution.md       leaf  — trails the Windows harness (zip)
230-vm-create-golden-linux.md       leaf  — standalone, loose (no gate; before finish)
```

Ordering rationale: `210` first (unblocked, de-risks shared machinery). Within `220`:
`010` gates the whole arc; `020` (golden) and `030` (host-pass) are independent but both
precede `040` (harness needs the windows binary *and* the windows golden HUT); `050`
trails `040` (never ship un-green). `230` is dependency-free, land any time before finish.

## Pointers

- Root BRIEF Tier-2 section (decomposition + the "Deferred" list this plans).
- ADR-0009 (harness), the retired `140-tier2-plan` (how the Linux wave was
  decomposed — the template for this one), `done/180-linux-host-pass` (the
  host-pass template Windows mirrors), `done/190-linux-verification-harness/`
  (the harness machinery + reuse seam).
- `vision/stages/text-ocr` (the `ocr_analyzer` daemon distribution must bundle).
- Glossary terms: Self-hosted verification harness, Host-under-test (HUT) VM,
  In-VM agent, Golden image, Host CLI.
