---
title: CLI
---

# Component: `cli-rs/` — Host CLI

Rust Cargo workspace that produces the `testanyware` host-CLI binary. It
orchestrates VM lifecycle and exposes a stable, scriptable surface over the
in-VM agents (HTTP) and the VNC framebuffer (RFB). This is *the* Host CLI; the
original macOS-only Swift implementation under `cli/` was retired 2026-06-03
(recoverable from git history).

## Layout

```
cli-rs/
├── Cargo.toml                       # workspace manifest
├── crates/
│   ├── testanyware-protocol/        # serde wire types + formatters (no I/O)
│   ├── testanyware-agent-client/    # HTTP client for the in-VM agents (reqwest)
│   ├── testanyware-rfb/             # pure-Rust RFB/VNC client: capture + input
│   ├── testanyware-ocr-client/      # OcrEngine seam; EasyOCR daemon bridge (ADR-0002)
│   ├── testanyware-video/           # VideoEncoder seam; screen record (ADR-0006)
│   ├── testanyware-vm/              # VM lifecycle: QEMU/tart, golden images, paths
│   └── testanyware-cli/             # clap binary `testanyware` + command surface
└── tests/
    ├── cli-contract.rs              # offline full-surface contract gate
    ├── live-vm-gate.rs              # live-VM-gated checks (env + #[ignore])
    └── fixtures/protocol/           # cross-language wire-format fixtures
```

## Key files

| File | Role |
|------|------|
| `crates/testanyware-cli/src/surface.rs` | The canonical command surface (`CANONICAL_COMMANDS`) and the stable `ERROR_CODES` catalogue. The authoritative list both `capabilities` and `schema` derive from. |
| `crates/testanyware-cli/src/commands/` | One module per command group (`vm`, `input`, `screen`, `agent`, `file`, `doctor`, `menu_bar`, `window`, …). |
| `crates/testanyware-cli/src/discoverability.rs` | `--json` envelope shape, help-text template enforcement, schema discovery. |
| `crates/testanyware-vm/src/lifecycle.rs` | `vm start` / `stop` / `list` / `delete` / `create-golden` entry points. |
| `crates/testanyware-rfb/src/input.rs`, `keymap.rs` | RFB input events + per-platform keymap (powers all `input *`). |
| `crates/testanyware-protocol/src/lib.rs` | serde wire types — third copy of the agent protocol (alongside `agents/macos/`); kept in sync by the fixtures contract test. |

## Build / test

```bash
cd cli-rs

# Build
cargo build --workspace                  # debug
cargo build --workspace --release        # release — binary at target/release/testanyware

# Offline tests (no VM required) — includes the full-surface contract gate
cargo test  --workspace
cargo test  --test cli-contract          # the CLI design contract gate specifically

# Live-VM-gated checks (require a running golden — opt in via env)
# See tests/live-vm-gate.rs for the #[ignore] reasons and env switches.
```

## Per-platform facilities

Native capability is selected per host via `#[cfg(target_os = ...)]` rather than
a lowest-common-denominator everywhere:

- **OCR** (`screen find-text`): Apple Vision on macOS; EasyOCR daemon
  (`OcrChildBridge`) on Linux/Windows — the `OcrEngine` seam (ADR-0002).
- **Video** (`screen record`): AVFoundation/VideoToolbox via `objc2` on macOS;
  `ffmpeg-next` on Linux/Windows — the `VideoEncoder` seam (ADR-0006).

## Common pitfalls

- **No persistent VNC server.** Unlike the retired Swift `_server`, every
  command opens its own short-lived RFB connection (ADR-0004). The two
  long-lived RFB consumers are the embedded viewer (`viewer` / `vm start
  --viewer`, ADR-0005) and the bounded `screen record` sampler.
- **The contract is the gate.** Every command must satisfy
  `docs/architecture/cli-design-contract.md` (stable error codes, `--json` for
  data commands, `--dry-run` for mutating commands, the §7 help template,
  schema discovery). `cargo test --test cli-contract` enforces the offline
  surface; happy-path checks that need a running VM live in `live-vm-gate.rs`.
- **`testanyware-protocol` has sibling copies** — one here, one under
  `agents/macos/`. They must stay in sync by hand; the fixtures contract test
  catches wire-shape drift.
- **First-run macOS Automation permission** still applies to
  `vm start --viewer` and `vm stop` paths that drive System Events. macOS binds
  the grant to the binary's path — install `testanyware` at a stable path
  (e.g. `/usr/local/bin/testanyware`) to rebuild without re-granting.
