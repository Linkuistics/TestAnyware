# agents/macos/ — macOS in-VM agent

Swift package that runs **inside** a macOS VM and exposes the
TestAnyware agent HTTP surface on port 8648 using native
`ApplicationServices` / `AXUIElement`. Transport is Hummingbird.

## Working on this component

```bash
cd agents/macos
swift build -c release      # binary at .build/release/testanyware-agent
swift test
```

The binary is installed at `/usr/local/bin/testanyware-agent` inside
the macOS golden image and launched by a LaunchAgent
(`com.linkuistics.testanyware-agent`).

## Notes

- Self-contained: vendors its own copy of the wire-format sources in
  `Sources/TestAnywareAgentProtocol/`. Does **not** path-depend on
  `cli/`. Keep it in sync with the host copy by hand.
- Wire shapes are documented in
  [`docs/architecture/agent-protocol.md`](../../docs/architecture/agent-protocol.md).
- TCC grant is tied to the binary path and signature — installing
  over an existing binary invalidates the grant.

See [`docs/components/agents-macos.md`](../../docs/components/agents-macos.md)
for module layout, key files, and common pitfalls.
