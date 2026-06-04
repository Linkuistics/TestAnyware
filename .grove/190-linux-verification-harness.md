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
