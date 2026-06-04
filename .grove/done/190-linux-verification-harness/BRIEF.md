# 190-linux-verification-harness — brief

**Kind:** node (decomposed from a work leaf, 2026-06-04)

## Goal

Build the **self-hosted verification harness** (ADR-0009) and run it green for
**Linux aarch64**: provision a stock Ubuntu ARM64 guest with the cross-compiled
`testanyware`, forward a real tart macOS golden's endpoint through the host, and
run the three-band smoke suite. The machinery the deferred Windows harness later
reuses with the provisioning channel swapped (ssh → in-VM agent). Gates "done"
for the Linux-host work (`180`, already retired into `done/`).

## Why this decomposed (2026-06-04)

Too big for one focused session, and live-VM green is inherently multi-attempt.
Split **risk-ordered**, each leaf a focused session that lands *verified* value:

- **`010-machinery-and-endpoint-free`** — the reusable harness skeleton +
  cross-build + ffmpeg-8 baseline bundle + ssh-provision a stock Ubuntu HUT +
  the **endpoint-free band green**. Proves ADR-0009's #1 claim — *the
  cross-compiled binary executes on aarch64-linux* — with no golden, no forward,
  no OCR. Cheapest, highest-information.
- **`020-endpoint-driven-bands`** — the macOS golden + the in-process host→golden
  TCP forward + the endpoint-driven bands **minus OCR**: `agent` HTTP actions,
  `input *`, `screen capture`/`size`, `screen record`→mp4 (the `170` ffmpeg
  encoder's runtime proof).
- **`030-ocr-band`** — `screen find-text` (OCR). Isolated because it is the one
  heavy, independently-riskable provisioning step (EasyOCR pulls **torch**, and
  the `ocr_analyzer` daemon module is **not in this tree** — must be located/
  ported first).

`020` and `030` share the golden + forward, so a session that lands `020` green
can often roll straight into `030` while the golden is still up — but they are
separate leaves so the torch rabbit hole cannot block recording the rest green.

## Shared design (ADR-0009)

- **HUT VM = stock tart Ubuntu ARM64** (`ghcr.io/cirruslabs/ubuntu:24.04`,
  already pulled locally). The HUT is the *host*, not a target — no agent, no
  dependency on the deferred Linux golden. Provision with **only the cross
  binary (+ runtime libs)** over **ssh**, reusing the ADR-0007 `russh`
  `SshSession` in `testanyware-vm` (`connect_password`/`connect_key`/
  `wait_for_password`/`exec`/`upload`). Stock Cirrus Ubuntu first-contact auth is
  **`admin`/`admin`** (verify in `010`); install the harness pubkey over a
  password session, key-auth thereafter — exactly the macOS-golden pattern.
- **Endpoint = a real kept-built tart macOS golden** (`testanyware-golden-macos-
  tahoe`, present locally). Drive its agent (`:8648`) + VNC through a **macOS-host
  port-forward**: guest CLI targets `host-gateway:PORT`; host forwards to the
  golden. **Guest→host-gateway is the reliable NAT edge** (ADR-0009); guest→guest
  is not routable.
- **The forward is an in-process tokio TCP proxy in the harness itself** — *not*
  `socat`/`ssh -L`. `socat` is not installed on this host, and the harness is
  already a Rust process that can bind `0.0.0.0:PORT` and splice to
  `golden_ip:8648` / `golden_ip:VNC`. Pure Rust, no external dep, and it *is* the
  reusable machinery the Windows harness inherits. (This supersedes the original
  brief's "`socat`/`ssh -L`" sketch.)
- **The endpoint seam needs no spec files.** `resolve.rs` lets the in-guest CLI
  target an arbitrary endpoint directly: `--agent <host:port>` (agent HTTP) and
  `--vnc <host:port>` + `TESTANYWARE_VNC_PASSWORD` (RFB). So the in-guest calls
  are e.g. `testanyware agent health --agent <gw>:<AFWD> --json` and
  `testanyware screen capture --vnc <gw>:<VFWD> --json`.

### Three-band surface split

- *endpoint-free* (no target): `capabilities`, `schema`, `llm-instructions`,
  `doctor`, `--help`, dry-runs. → **`010`**.
- *endpoint-driven* (→ forwarded golden): `agent` HTTP actions, `input *`,
  `screen capture`/`size` → **`020`**; `screen record`→mp4 → **`020`**;
  `screen find-text` (OCR) → **`030`**.
- *build/compile-only* (not run in-guest): `vm start/stop/list/delete`,
  `vm create-golden` (nested virt / host-orchestration) — asserted by the
  existing macOS cross-build, not exercised in the HUT.

### Arch coverage

aarch64 gets full in-guest smoke; **x86_64 is build-verified only** (no native
x86_64 guest on this Mac). The gap is **logged**, never silently treated as
covered (ADR-0009 no-silent-caps). `010` owns logging it (it owns the build).

## CRITICAL — libav is a load-time dependency, not a record-band one

`testanyware-video` does `use ffmpeg_next as ffmpeg` (a normal link, *not*
`dlopen`), so the `testanyware` ELF carries hard `NEEDED libavcodec.so.62 /
libavformat.so.62 / libavutil.so.60 / libswscale.so.9` (ffmpeg **8.1** sonames).
**Stock Ubuntu 24.04 ships ffmpeg 6.1 (`libav*.so.60`)**, so the dynamic loader
fails to resolve `NEEDED` before `main` — **even `testanyware --help` will not
exec** until the ffmpeg-8 `.so`s are staged (rpath `$ORIGIN` or
`LD_LIBRARY_PATH`). The 180 handoff note framed this as gating *only* `record`;
that conflated *functional* use (only `record` calls the encoder) with
*load-time* linkage. **Staging the BtbN ffmpeg-8 `gpl-shared` `.so` bundle is a
baseline requirement for the binary to run at all → owned by `010`.** Recipe:
`docs/research/170-ffmpeg-cross-link.md` ("Runtime ABI", option 1: bundle the
`linuxarm64-gpl-shared` `.so`s beside the binary).

## On retire (promote upward, then the node retires)

When all three leaves are green, before retiring this node:
- Record the **Linux aarch64 runtime green** into the **root brief's Tier-2
  checklist** (the "Self-hosted host verification harness" line). The `180` leaf
  is already in `done/`; note the green there is historical.
- Ensure the **x86_64 build-verified-only gap** is logged where a reader sees it
  (harness doc-comment + the x86_64 line in the root Tier-2 checklist).
- Promote any durable harness-reuse notes (the Windows swap points) into the root
  brief's "Deferred" Windows-harness line, so the deferred leaf inherits them.

## Constraints

- **Opt-in/env-gated** like `tests/live-vm-gate.rs` (`#[ignore]` + a
  `TESTANYWARE_LINUX_HARNESS=1` gate) — it needs real VMs, must not run in a plain
  `cargo test`. The harness file: `cli-rs/crates/testanyware-cli/tests/
  linux-host-harness.rs`. Pure helpers (band classification, host-gateway parse,
  endpoint formatting) get offline unit tests in the same file, à la live-vm-gate.
- **Don't bake test tooling into images** ([[minimal-images]]) — provision the
  binary, the ffmpeg `.so` bundle, and (in `030`) the OCR venv at run time, into
  a throwaway clone. VM clone+start is cheap ([[vm-costs]]).
- The harness **consumes** a macOS golden + the russh layer; it does not build
  them. If no golden is kept-built, create one (`vm create-golden --platform
  macos`, node `110`).

## Reuse seam (for the deferred Windows harness)

Built once here, swapped there: **the provisioning channel** (Linux ssh →
Windows in-VM agent `file upload`/`exec`, since Windows ships no sshd) and **the
HUT image**. **Shared unchanged:** the in-process host→golden forward, the
host-gateway discovery, the three-band smoke driver, and the `--agent`/`--vnc`
endpoint targeting. Factor the channel behind a small trait/enum so the Windows
leaf only writes a second impl.
