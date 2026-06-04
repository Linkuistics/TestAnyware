# 220-windows-arc — brief

**Kind:** node (decomposed at materialization, 2026-06-04, planning leaf `200`)

## Goal

Take the **Windows-host** story from nothing to **runtime-green + shipped**: a
Windows golden the harness can use as its HUT, the `#[cfg(windows)]` facility
wiring that makes the cross-compiled `testanyware.exe` functionally correct, the
**self-hosted verification harness** running green for **Windows aarch64**
(reusing `190`'s machinery with the provisioning channel swapped ssh → in-VM
agent), and finally the Windows distribution zip. This is the **longest pole** of
the remaining Tier-2 wave (root BRIEF "Deferred" list) — it carries the hardest
external dependency (a working Windows golden + in-VM agent, a *separate,
out-of-scope* workstream).

## Why this order (set by `200` grilling, 2026-06-04)

Risk-ordered, each leaf a focused session landing verified value:

- **`010-windows-readiness-spike`** — **fail-fast, zero Rust.** Use the *existing*
  `provisioner/scripts/vm-create-golden-windows.sh` to build a Win11 ARM64 golden,
  boot it, and confirm the in-VM agent reaches health and `/exec`/`/upload`/
  `/download` respond. Gates the whole arc: if the agent/golden is broken, the
  Windows harness (`040`) is blocked on the agents workstream and we learn it for
  ~1 cheap session ([[vm-costs]]) instead of after porting golden + host-pass.
  Mirrors the `160` fail-fast spike.
- **`020-vm-create-golden-windows`** — full **Rust port** of the Windows golden
  creation into `vm create-golden --platform windows` (mirrors node `110`'s macOS
  port; reuses the `testanyware-vm` russh/recovery/finalize/qemu layers). This
  golden **is the harness HUT** — it gates `040`. Delete
  `vm-create-golden-windows.sh` once ported + live-verified.
- **`030-windows-host-pass`** — the source `#[cfg(windows)]` facility wiring:
  `monitor.rs` AF_UNIX → named-pipe/TCP, plus the already-`#[cfg]`-paired
  `process`/`spec`/`detached`/`doctor` arms. Net-new beyond parity (Swift was
  macOS-only). Independent of `020`; both precede `040`.
- **`040-windows-harness`** — **reuses `190`'s machinery verbatim** (the
  in-process host→golden TCP forward, host-gateway discovery, the band-agnostic
  `run_band` driver, `--agent`/`--vnc` endpoint targeting, the `ocr_analyzer`
  daemon + venv recipe). The factored `ProvisionChannel` trait gets a **2nd impl**
  (ssh → in-VM agent `file upload`/`exec`) + a Windows HUT. Needs the windows
  binary (`030`) *and* the windows golden HUT (`020`).
- **`050-windows-distribution`** — the Windows zip (`cargo-zigbuild` per windows
  triple + OCR-venv bundle, reusing `210`'s shared machinery). **Trails `040`** —
  never ship a binary the harness has not run green.

`020` and `030` are independent (one is macOS-host golden work, one is source
`#[cfg]` wiring) and may be done in either order; both must land before `040`.

## Shared design (ADR-0009, inherited from `190`)

- **HUT = the Windows agent-golden** (from `020`), provisioned over the **in-VM
  agent's `/upload` + `/exec`** HTTP surface — **Windows ships no sshd**, so the
  agent is the only in-guest control channel. Confirmed present in code:
  `agents/windows/SystemEndpoints.cs` exposes `/exec`, `/upload`, `/download`;
  agent cross-builds from this Mac (`dotnet -r win-arm64 --no-self-contained`).
- **Endpoint = the same kept-built tart macOS golden** the Linux harness used,
  driven through the **in-process host→golden TCP forward** (guest targets
  `host-gateway:PORT`, the reliable NAT edge). Unchanged from `190`.
- **Arch coverage:** **aarch64-windows** gets full in-guest smoke (the only
  Windows guest this Apple-Silicon Mac boots natively — QEMU+swtpm Win11 ARM64);
  **x86_64-windows is build/link-verified only**, gap logged (no-silent-caps).
  Windows targets use the cross-friendly `-gnu`/`-gnullvm` variants (msvc can't
  cross from a Mac).

## Reuse seam (built in `190`, consumed here)

Swapped: **the `ProvisionChannel`** (Linux ssh → Windows in-VM agent
`upload`/`exec`) and **the HUT image**. **Shared unchanged:** the in-process
host→golden forward, host-gateway discovery, the three-band `run_band` driver,
the `--agent`/`--vnc` endpoint targeting, and the `ocr_analyzer` daemon + venv
recipe. `190`'s `linux-host-harness.rs` factored the channel behind a trait so
this node only writes the second impl. See `done/190-linux-verification-harness/
BRIEF.md` "Reuse seam".

## Constraints

- **Don't bake test tooling into images** ([[minimal-images]]) — provision the
  binary + runtime libs + (OCR band) the venv at run time into a throwaway clone.
- The harness **consumes** the windows golden + macOS golden; it does not build
  them at test time.
- Acceptance gate for the host-pass + distribution work stays the **CLI design
  contract**.

## `010-windows-readiness-spike` disposition — **GREEN** (2026-06-04)

The whole arc's hardest external dependency is **confirmed real**. Findings:

- **The Windows golden already exists** (`$XDG_DATA_HOME/testanyware/golden/
  testanyware-golden-windows-11.qcow2`, built 2026-05-29 via the existing
  `vm-create-golden-windows.sh`; ISO + virtio-win cached). The spike did not need
  to rebuild it — it cloned and booted it ([[vm-costs]]: per-task cost is just
  clone+boot).
- **A fresh clone boots green and the in-VM agent is reachable fast.** Booting a
  *finalized* golden (autologin + the `TestAnywareAgent` Task Scheduler logon
  task) reaches `agent health` → `reachable:true, accessibility_status:granted`
  within ~15s — not the 20-40 min the *install* takes. swtpm TPM + COW clone via
  the Rust path, single QEMU process confirmed.
- **The `/upload`→`/exec`→`/download` round-trip works** (the `040`
  `ProvisionChannel` operations): uploaded a token, PowerShell uppercased it
  in-guest (`exit_code:0`), downloaded the transformed result back, byte-verified.
- **Bonus finding for `020`/`040`:** the Windows VM *runtime* lifecycle is
  **already ported to Rust** — `testanyware vm start --platform windows`
  (`QemuRunner`/`lifecycle.rs`: `Platform::Windows`, `needs_tpm`, QEMU+swtpm,
  `hostfwd tcp::0-:8648`) + `agent health` + `file upload/exec/download` all ran
  green against the golden with the release binary, **no Rust written**. So `020`'s
  remaining gap is the **golden *creation*** port (the shell script), not the
  start/agent-channel runtime, which already exists; and `040`'s endpoint
  machinery has a proven agent channel to build its `ProvisionChannel` 2nd impl on.

**Arc proceeds to `020`/`030`.** No agent/golden gap to file against the agents
workstream — the channel is intact.

## On retire (promote upward, then the node retires)

- Record **Windows aarch64 runtime green** into the root BRIEF's Tier-2 checklist
  (the harness + Windows-host lines), and the **x86_64-windows build-verified-only
  gap** where a reader sees it.
- Promote the Windows golden's autounattend/agent provisioning model into
  CONTEXT.md (Q4 of `200`: documented inline, ADR only if it surprised).
- Note any harness reuse-seam refinements back so they stay legible.
