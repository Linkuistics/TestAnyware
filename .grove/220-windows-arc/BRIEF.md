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
  triple, reusing `210`'s shared machinery). **Trails `040`** — never ship a binary
  the harness has not run green. **No OCR-venv bundle** — EasyOCR is uninstallable
  on win-arm64; `215` confirmed reject of docker-host-unification, so this ships the
  native OCR-less 2/3-green surface (unblocked).
- **`060-windows-ocr-band`** — **added 2026-06-07** from the `215` spike's reject.
  Decide the Windows OCR engine (containerized Linux EasyOCR vs native
  `Windows.Media.Ocr` vs accept-the-gap, at the ADR-0002 seam) and, if implied,
  build it. **Additive band; does not block `050`.**

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

## `040-windows-harness` disposition — **2/3 bands live-GREEN, OCR deferred** (2026-06-05)

`tests/windows-host-harness.rs` (standalone, self-contained — **duplicated** `190`'s
machinery rather than extracting a shared module, per the node's "standalone"
decision). Live-verified on aarch64-windows (Windows agent-golden QEMU HUT +
macOS golden, all VMs torn down):

- **endpoint-free band GREEN (6/6)** — `--help`, `capabilities`, `schema`,
  `llm-instructions`, `doctor`, dry-run. The cross binary execs; ffmpeg-8 DLLs load.
- **endpoint-driven band GREEN (10/10)** — agent HTTP (health/snapshot/windows/
  wait), RFB `screen size`/`capture`, **`screen record` → ffmpeg-8 libx264 MP4**
  (the Windows DLL runtime proof, analogue of Linux's "libx264 runtime-proven"),
  `input key/type/click` — all through the in-process host→golden forward.
- **x86_64-windows: build/link-verified only**, gap logged (no-silent-caps).
- **OCR band: deferred LOGGED GAP** (see below).

**Reuse-seam refinements** (the brief asked to note these back): the
`ProvisionChannel` 2nd impl is the in-VM agent's `/exec` (`cmd.exe /c`) +
`/upload` + `/download`. Genuinely Windows-specific vs `190`'s ssh path:
(1) HUT lifecycle is `vm start --platform windows` (CLI-managed QEMU+swtpm),
like the macOS golden, not a manual tart clone; (2) the guest reaches the host
at the **fixed slirp gateway `10.0.2.2`** (no `ip route` discovery); (3) ffmpeg
DLLs **co-located beside the .exe** (image-dir DLL search), no `LD_LIBRARY_PATH`;
(4) `cmd.exe` invocations lead with `call`/`set` to dodge the `cmd /c`
quote-strip quirk (a line starting with `"` and holding >2 quotes gets its outer
pair stripped); (5) artifacts are read back via the agent's native `/download`
(simpler than `190`'s `od`-over-ssh); (6) the golden-readiness window is 300s
(vs `190`'s 120s) because the heavy concurrent Windows QEMU HUT slows the macOS
golden's render.

**Windows-OCR finding (drives `240`).** EasyOCR is **uninstallable on
aarch64-windows**: `opencv-python-headless` (a hard dep) has **no `win_arm64`
wheel** on PyPI, conda-forge, or cgohlke's win-arm64 set, and can't be
source-built in a minimal golden (no MSVC toolchain; [[minimal-images]]).
torch+torchvision **do** install (PyTorch's own cpu index, `win_arm64`/`cp312`).
This is a real ecosystem wall — the **low-regret kill signal** that hoisted
`215` (docker host unification, was `240`). **`215` REPORTED REJECT (2026-06-07,
`docs/research/240-docker-host-unification.md`):** don't containerize the whole
host (fails the host-side-framebuffer gate, ADR-0010), but OCR specifically *can*
move to a Linux container because it is host-side compute downstream of capture
with no hypervisor dependency. **Windows OCR is now owned by new leaf
`060-windows-ocr-band`** — containerized Linux EasyOCR vs native
`Windows.Media.Ocr` vs accept-the-gap (ADR-0002 seam). The harness keeps the
experimental in-guest EasyOCR attempt behind `TESTANYWARE_WINDOWS_TRY_OCR=1`.

## `050-windows-distribution` disposition — **DONE, aarch64 zip runtime-smoked** (2026-06-08)

The Windows release zip now ships from the shared `scripts/` pipeline. Reused
`210`'s machinery (`cargo-zigbuild` per triple, BtbN ffmpeg-8 sysroots, the
shared agent/script payload) with the Windows-specific divergences:

- **Two triples:** `aarch64-pc-windows-gnullvm` (first-class) +
  `x86_64-pc-windows-gnu` (build/link-verified only). Both cross-build green from
  this Mac via the `040`-proven recipe (`PKG_CONFIG_LIBDIR` +
  `BINDGEN_EXTRA_CLANG_ARGS=--target=<arch>-pc-windows-gnu`).
- **Delivery format = `.zip`, not Homebrew** — `package_bundle` branches on the
  triple; the formula template is untouched (Windows has no Homebrew entry).
  `release-publish.sh` uploads the zips as GitHub-release assets.
- **ffmpeg DLLs co-located beside the `.exe` in `bin/`** (the five
  `WINDOWS_DLLS`), no `lib/` and no RUNPATH surgery — the PE image-directory
  search is the Windows analogue of Linux's `$ORIGIN/../lib`.
- **No OCR module** — EasyOCR uninstallable on win-arm64; `screen find-text` is
  an unsupported documented gap until `060`.
- **`release-doctor.sh`** extended: Windows rustup targets, Windows ffmpeg
  sysroots, and a `zip` tool check.

**Pre-publish real-artifact gate CLEARED:** `windows_dist_zip_smoke`
(new `#[ignore]`d test in `windows-host-harness.rs`) boots the Windows golden
HUT, uploads the **actual release zip**, `Expand-Archive`s it into a **clean
prefix** (`C:\Users\Public\taw-dist`), and runs the endpoint-free band from the
extracted `bin\` — proving the *shipped layout's* DLL co-location loads (the
`--version` canary + 6/6 contract checks GREEN on aarch64-windows). Scoped
endpoint-free only (no macOS golden): functional correctness is `040`'s result
and a property of the binary, not the packaging; the zip can only break DLL
co-location, which the canary catches. **x86_64-windows zip produced but not
smoke-run** (no native x86_64 Windows guest; ADR-0009 no-silent-caps).

Only `060-windows-ocr-band` remains live in this node.

## On retire (promote upward, then the node retires)

- Record **Windows aarch64 runtime green** into the root BRIEF's Tier-2 checklist
  (the harness + Windows-host lines), and the **x86_64-windows build-verified-only
  gap** where a reader sees it.
- Promote the Windows golden's autounattend/agent provisioning model into
  CONTEXT.md (Q4 of `200`: documented inline, ADR only if it surprised).
- Note any harness reuse-seam refinements back so they stay legible.
