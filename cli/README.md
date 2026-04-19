# cli/ — Host CLI and driver library

Swift package that produces the `testanyware` executable and the
`TestAnywareDriver` library.

## Working on this component

```bash
cd cli
swift build -c release      # binary at .build/release/testanyware
swift test                  # unit tests (no VM required)
```

Integration tests live under `cli/Tests/IntegrationTests/` and need a
live VM — set `TESTANYWARE_VM_ID` first, or `TESTANYWARE_SKIP_INTEGRATION=1`
to skip. See `docs/components/cli.md` for the end-to-end test workflow.

## Notes

- `cli/` is flat (no `cli/macos/` subdirectory).
- **Linux host support is planned via a Rust port;** see
  `LLM_STATE/core/decisions.md` for the rationale.
- The file `cli/Sources/TestAnywareAgentProtocol/` is a copy of the
  sources that also live under `agents/macos/Sources/TestAnywareAgentProtocol/`.
  Keep them in sync by hand. `cli/Tests/TestAnywareAgentProtocolTests/`
  catches drift at the wire level.

See [`docs/components/cli.md`](../docs/components/cli.md) for module
layout, key files, and common pitfalls.
