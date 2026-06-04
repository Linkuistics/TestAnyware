# 190-linux-verification-harness

**Kind:** work (may decompose if the smoke driver + provisioning split)

## Goal

Build the **self-hosted verification harness** (ADR-0009) and run it green for
**Linux aarch64**: provision a stock Ubuntu ARM64 guest with the cross-compiled
`testanyware`, forward a real tart macOS golden's endpoint through the host, and
run the three-band smoke suite. This is the harness machinery that the deferred
Windows harness later reuses with the provisioning channel swapped. Gates "done"
for the Linux-host work (`180`).

## Context

Design fully concretized in `140`'s grilling (ADR-0009). Build it for Linux:

- **HUT VM:** stock **tart Ubuntu ARM64** (cheap `tart pull`; no dependency on
  the deferred Linux golden — the HUT is the *host*, not a target, and needs no
  agent). Provision with **only the cross binary** over **ssh**, reusing the
  ADR-0007 `russh` layer in `testanyware-vm` (`SshSession`:
  `connect_password`/`connect_key`/`exec`/`upload`).
- **Endpoint:** drive a real, kept-built **tart macOS golden**'s agent (`:8648`)
  + VNC through a **macOS-host port-forward** (`socat` / `ssh -L`): the guest CLI
  targets `host-gateway:PORT`; the host forwards to the golden. (Guest→host-
  gateway is the reliable NAT edge — ADR-0009.)
- **Three-band smoke** (run the in-guest cross CLI, assert its `--json`
  envelopes):
  - *endpoint-free* (no target): `capabilities`, `schema`, `llm-instructions`,
    `doctor`, `--help`, dry-runs.
  - *endpoint-driven* (→ forwarded golden): `agent` HTTP actions, `input *`,
    `screen capture`/`size`/`find-text` (OCR), `screen record`→mp4 (the `170`
    ffmpeg encoder's runtime proof).
  - *build/compile-only* (not run in-guest): `vm start/stop/list/delete`,
    `vm create-golden` (nested virt / host-orchestration).
- **Arch:** aarch64 gets full in-guest smoke; **x86_64 is build-verified only**
  (no native x86_64 guest on this Mac) — the gap is **logged**, not silently
  treated as covered (ADR-0009 no-silent-caps).

Infra to build on: `testanyware-vm` (`TartRunner`, `paths.rs`, the russh
`SshSession`), the macOS golden produced by node `110`, and the live-VM-gate
pattern (`tests/live-vm-gate.rs`: env-gated + `#[ignore]`d so it's opt-in).

## Done when

- A Linux harness (an `#[ignore]`d/env-gated test or a `scripts/` driver,
  matching the live-vm-gate convention) that, in one invocation: clones+starts a
  stock Ubuntu ARM64 HUT, ssh-installs the aarch64-linux `testanyware`, stands up
  the host→golden forward, runs the three-band smoke, and asserts results.
- It runs **green on this Mac** for Linux aarch64 (cheap — [[vm-costs]]).
- The harness **machinery is factored for reuse** by the deferred Windows
  harness (provisioning channel and HUT image are the swap points; the forward +
  smoke driver are shared).
- The x86_64 build-verified-only gap is **logged** where a reader will see it.
- Record the Linux green back into `180`'s "done when" runtime line and the root
  brief's Tier-2 checklist.

## Notes

- The harness *consumes* a macOS golden + the russh layer; it does not build
  them. If no golden is kept-built, create one first (`vm create-golden
  --platform macos`, from node `110`).
- Keep it **opt-in/env-gated** like the existing live-VM gate — it needs real
  VMs and must not run in a plain `cargo test`.
- Don't bake test tooling into images ([[minimal-images]]); provision the binary
  at run time.

## Handoff from `180` (source pass green, 2026-06-04)

The Linux source pass is complete and verified: `testanyware` cross-builds clean
for **both** `aarch64-` and `x86_64-unknown-linux-gnu` (full binary — no
`monitor.rs` gap on the Unix path), all facility `#[cfg]` arms select correctly
(OCR→daemon, encoder→ffmpeg-next, QEMU→KVM per-arch, paths→XDG), and the macOS
suite stays green. Two **provisioning prerequisites** the smoke bands depend on —
each will *silently fail its band* if the HUT lacks it, so wire them when
provisioning the guest:

- **`screen record` (endpoint-driven band) needs ffmpeg 8 at runtime.** The
  binary binds `libavcodec.so.62 / libavformat.so.62 / libavutil.so.60 /
  libswscale.so.9` (ffmpeg **8.1** sonames). **Stock Ubuntu 24.04 ships ffmpeg
  6.1** (`libav*.so.60`) — a soname mismatch, so `screen record` will fail to
  load libav unless you provision the **BtbN ffmpeg-8 `gpl-shared` `.so`s**
  beside the binary (rpath `$ORIGIN`, or `LD_LIBRARY_PATH`). Recipe + the three
  bundle/static/distro options: `docs/research/170-ffmpeg-cross-link.md`
  ("Runtime ABI"). This gates **only** `record`; the rest of the CLI has no
  ffmpeg dependency, so the other bands run without it.
- **`screen find-text` (OCR, endpoint-driven band) needs a Python venv with
  `easyocr`.** Linux routes through the EasyOCR daemon (`OcrChildBridge`);
  `resolve_interpreter()` looks for `$TESTANYWARE_OCR_PYTHON`, then
  `<prefix>/libexec/venv/bin/python`, then a `pipeline/.venv`. Provision that
  venv (or set `TESTANYWARE_OCR_PYTHON`), else `find-text` latches
  permanently-unavailable.

Not in scope for `180` or `190`'s bands: the **wgpu/Vulkan viewer** opening on a
Linux host (llvmpipe or a real GPU) — it cross-links (`eframe`/`egui-wgpu`
compiled; Vulkan is `dlopen`-ed per `080`) but its *runtime* on Linux stays
unverified after `190`. Flag as a possible follow-up if a Linux GUI path matters.

When `190` runs the bands green, record the Linux runtime green back into the
root brief's Tier-2 checklist (the `180` leaf will already be retired into
`done/`).
