# cli-rs — the TestAnyware host CLI

This workspace is the TestAnyware host CLI: the `testanyware` binary plus its
supporting crates. It replaced the original macOS-only Swift CLI, which was
retired (and `cli/` deleted) on 2026-06-03 once the Rust port reached macOS
parity against the CLI design contract.

## Layout

```
cli-rs/
├── Cargo.toml                           # workspace manifest
├── crates/
│   ├── testanyware-protocol/            # serde wire types + formatters (no I/O)
│   ├── testanyware-agent-client/        # HTTP client for in-VM agents (reqwest)
│   ├── testanyware-rfb/                 # pure-Rust RFB/VNC client: capture + input
│   ├── testanyware-ocr-client/          # OcrEngine seam; EasyOCR daemon bridge
│   ├── testanyware-video/               # VideoEncoder seam; screen record
│   ├── testanyware-vm/                  # VM lifecycle: QEMU/tart, golden images
│   └── testanyware-cli/                 # clap binary `testanyware` + surface.rs
└── tests/
    ├── cli-contract.rs                  # offline full-surface contract gate
    ├── live-vm-gate.rs                  # live-VM-gated checks (env + #[ignore])
    └── fixtures/protocol/               # cross-language wire-format fixtures
```

## Design constraints

- **Cross-platform host.** Linux, macOS, and Windows are all supported hosts.
  Avoid OS-locked crates where a portable path exists — prefer `std::process` /
  `tokio::process` so platform work stays additive.
- **Pure-Rust RFB.** No `royalvnc` FFI — the RFB/VNC client
  (`testanyware-rfb`) is pure Rust so it builds on every host.
- **Per-platform native facilities** via `#[cfg(target_os = ...)]`: Apple
  Vision / AVFoundation on macOS, EasyOCR / `ffmpeg-next` on Linux/Windows
  (the `OcrEngine` and `VideoEncoder` seams — ADR-0002, ADR-0006).
- **`directories` for paths** — never hard-code `~/.local/state` or
  `%LOCALAPPDATA%`.

## Protocol parity

`crates/testanyware-protocol/` is the wire-format type definitions, kept in
sync with the macOS agent's copy under
`agents/macos/Sources/TestAnywareAgentProtocol/`. The contract test in
`crates/testanyware-protocol/tests/fixtures.rs` reads canonical JSON from
`tests/fixtures/protocol/` and verifies the Rust types decode + re-encode to
the same key set; the matching test in the macOS agent verifies the same
fixtures against Swift's `JSONEncoder`. If either side drifts, one of the
suites fails loudly.

## The contract gate

Every command must satisfy `docs/architecture/cli-design-contract.md` — stable
error codes, `--json` for data commands, `--dry-run` for mutating commands, the
§7 help template, and schema discovery. The offline surface is gated by
`tests/cli-contract.rs`; happy-path checks that need a running VM live in
`tests/live-vm-gate.rs`.

## Building

```bash
cd cli-rs
cargo build --workspace
cargo test  --workspace
cargo run -- --help
```
