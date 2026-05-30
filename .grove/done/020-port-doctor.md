# 020-port-doctor

**Kind:** work

## Goal

Port `testanyware doctor` to the Rust CLI — the preflight/diagnostics command
that checks the host environment is ready to run VMs and agents. Retire the
`doctor` stub so it emits real check results satisfying the contract.

## Context

- Stub to retire: `cli-rs/crates/testanyware-cli/src/main.rs` (`doctor` →
  `unimplemented("doctor")`). Surface entry: `doctor`, schema `doctor`,
  non-mutating, data-producing.
- Swift reference: `cli/Sources/testanyware/DoctorCommand.swift` plus the
  individual checks under `cli/Sources/TestAnywareDriver/Diagnostics/`
  (`ToolAvailabilityCheck`, `InstallPathCheck`, `BundledAgentsCheck`,
  `BundledScriptsCheck`, `ProvisionerScriptsVersionCheck`, `BrewPrefixResolver`).
- Per-platform-facilities direction applies: the host-preflight set differs by
  host OS (`#[cfg(target_os = ...)]`). The old backlog framed this as
  "Linux-host preflight checks"; with full-retirement scope, cover macOS and
  Windows hosts too — but a host-conditional check set is fine, not every check
  applies everywhere.

## Done when

- `doctor` runs real checks and emits a `--json` envelope against the `doctor`
  schema; human output is readable and lists pass/fail/warn per check.
- Exit code reflects overall health per the contract's error-code catalogue.
- Host-conditional checks compile and run on at least the macOS host
  (the dev host); Linux/Windows branches are `#[cfg]`-gated and unit-covered.
- `cli-contract.rs` passes for `doctor`; `cargo test --workspace` green;
  clippy clean.

## Notes

Keep checks **non-destructive and fast** — `doctor` is a read-only probe. Where
a Swift check shells out to a tool (`qemu-img`, `swtpm`, `brew`, etc.), mirror
the tool list but resolve paths the Rust way (the `directories` crate / explicit
env), not by assuming a Homebrew prefix.
