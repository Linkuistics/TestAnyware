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

## Disposition — **GREEN (build-verified)** (2026-06-05)

The empirical baseline collapsed the predicted six-file pass to **one compile
blocker + one boundary**: with the BtbN windows ffmpeg-8 sysroots (already cached
from `170` at `/tmp/taw-ffmpeg-sr/{aarch64,x86_64}-windows`) on
`PKG_CONFIG_LIBDIR` + `BINDGEN_EXTRA_CLANG_ARGS=--target=<arch>-pc-windows-gnu`,
the *only* Windows compile error in our crates was `monitor.rs:12` `UnixStream`
(rfb/ocr/agent/video already compiled clean). This matches the `170` doc's "full
windows bin remains blocked only at `monitor.rs`".

**What landed (per the `030` grilling-free work scope + the chosen "honest
boundary + log gap" disposition):**

- **`monitor.rs`** — `UnixStream` import + connecting `send` body are now
  `#[cfg(unix)]`; the Windows `send` returns `ErrorKind::Unsupported`. Pure HMP
  parsers (`parse_agent_port`/`parse_vnc_port`) stay shared. *Compiles* on
  Windows; unreachable behind the boundary below.
- **Honest boundary (the disposition Antony picked):** local-QEMU on a Windows
  host is **build/link-verified only** (no AF_UNIX monitor, no Unix process
  control), so rather than a large unverifiable WinAPI block, `vm start` /
  `vm start --dry-run` **fail fast and loud** with the new
  `VmError::HostUnsupported` (`VM_HOST_UNSUPPORTED`, exit 1, `details.detail`,
  remediation → use a Linux/macOS host). Gate is
  `preflight::check_host_supports_local_qemu()` — a normal `Result` fn (not a
  `#[cfg]`'d early return) so the QEMU body stays warning-clean on every target.
  Added to the `surface.rs` error-code catalogue. `process`/`spec`/`detached`/
  `qemu_profile` were already `#[cfg]`-paired and compile; they sit unreachable
  behind this gate (left as-is, not fake-implemented — matches no-silent-caps).
  Golden creation is already `#[cfg(target_os="macos")]`-gated (it builds the
  windows golden *from the Mac*), so it needed no boundary.
- **Harness-reachable Windows correctness (the half `040` *will* exercise):**
  `paths.rs` → `%LOCALAPPDATA%`/`%TEMP%` (+`%USERPROFILE%`) fallbacks;
  `resolve.rs::state_dir` → `%USERPROFILE%` fallback; `engine.rs::resolve_
  interpreter` → Windows venv layout (`Scripts\python.exe`, fallback `python`).
  `doctor.rs` was already Windows-correct (`where`, empty brew → `NoHomebrew`,
  `is_file` exec fallback). The OCR `bridge.rs` was already portable (plain
  `tokio::process`, Windows-aware temp-file note).
- **Build proof:** `cargo-zigbuild` builds **`aarch64-pc-windows-gnullvm`**
  (first-class) **and `x86_64-pc-windows-gnu`** (build-verified) clean, zero
  our-crate warnings, full 382 MB `.exe` produced. No macOS regression: native
  build clean, `testanyware-vm`/`-ocr-client` units green, `cli-contract.rs`
  22/0 (+4 live-gated), `testanyware-cli --lib` 92/0.

## Handoff to `040` (what the harness must *prove* in-guest, Windows aarch64)

This leaf made the facilities *correct*; `040` proves they *run* in the Windows
HUT. Per the band split (the HUT drives a **forwarded** macOS-golden endpoint via
`--agent`/`--vnc`; it never runs local QEMU, so the `vm start` boundary above is
**not** exercised and that is expected):

- **Endpoint-free:** `--help`, `capabilities`, `schema`, `--dry-run`, and
  **`doctor --json`** run green on Windows (the `where`/no-Homebrew path).
- **Endpoint-driven:** `agent health/snapshot/windows/wait`, `input *`,
  `screen size/capture`, and **`screen record` → mp4** — the ffmpeg-8 **link** is
  proven; `040` must prove the **runtime**: the BtbN `avcodec-62.dll`/
  `avformat-62.dll`/`avutil-60.dll`/`swscale-9.dll` load (Windows same-dir DLL
  search — co-locate them; a `050` distribution concern the harness provisions at
  run time) and libx264 encodes. The Windows analogue of Linux's "ffmpeg-8
  libx264 runtime-proven".
- **OCR:** `screen find-text` via the EasyOCR daemon, interpreter pointed at the
  in-guest venv through **`TESTANYWARE_OCR_PYTHON`** (Windows venv python is
  `Scripts\python.exe`; `resolve_interpreter`'s auto-discovery now matches, but
  the harness sets the env explicitly).
- **`ProvisionChannel` 2nd impl:** ssh → in-VM agent `file upload`/`exec` (the
  `010` spike proved the round-trip works) + the Windows agent-golden HUT.
- **x86_64-windows stays build/link-verified only** (no native x86_64 guest on
  this Mac) — log the gap (ADR-0009 no-silent-caps), as `190` did for Linux.
