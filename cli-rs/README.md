# cli-rs — Rust port of the TestAnyware host CLI

This workspace is the in-progress Rust replacement for the Swift CLI under
`cli/`. Both packages live side-by-side until the Rust port reaches parity
and the Swift CLI is retired (see `LLM_STATE/core/` backlog tasks).

## Layout

```
cli-rs/
├── Cargo.toml                           # workspace manifest
├── crates/
│   ├── testanyware-protocol/            # serde wire types (no I/O)
│   ├── testanyware-agent-client/        # HTTP client over reqwest
│   └── testanyware-cli/                 # clap binary `testanyware`
└── tests/
    └── fixtures/protocol/               # cross-language contract fixtures
```

## Design constraints

- **Linux is the primary host**, macOS secondary, Windows tertiary. Avoid
  Linux-only crates (`nix`, `procfs`) — prefer `std::process` and
  `tokio::process` so cross-platform work is additive.
- **No `royalvnc` FFI.** The Swift toolchain on Linux is a non-starter,
  so the RFB client must be pure Rust (lands in a later task).
- **ffmpeg is embedded as a library** via `ffmpeg-next`, not subprocess
  (lands in a later task).
- **`directories` for paths** — never hard-code `~/.local/state` or
  `%LOCALAPPDATA%`.

## Protocol parity

`crates/testanyware-protocol/` is the third copy of the wire-format types
(after `cli/Sources/TestAnywareAgentProtocol/` and
`agents/macos/Sources/TestAnywareAgentProtocol/`). The contract test in
`crates/testanyware-protocol/tests/fixtures.rs` reads canonical JSON from
`tests/fixtures/protocol/` and verifies the Rust types decode + re-encode
to the same key set. The matching Swift test
(`cli/Tests/TestAnywareAgentProtocolTests/CrossLangFixturesTests.swift`)
verifies the fixtures still match Swift's `JSONEncoder` output. If either
side drifts, one of the two suites fails loudly.

## Building

```bash
cd cli-rs
cargo build --workspace
cargo test  --workspace
cargo run -- --help                       # subcommand stubs
```
