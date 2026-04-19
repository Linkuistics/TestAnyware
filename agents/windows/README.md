# agents/windows/ — Windows in-VM agent

C# / ASP.NET Core project that runs **inside** a Windows 11 ARM64 VM
and exposes the TestAnyware agent HTTP surface on port 8648 using UI
Automation via FlaUI.

## Working on this component

**Cross-built from a macOS host** (no Windows machine required):

```bash
cd agents/windows
dotnet build -r win-arm64 --no-self-contained
dotnet publish -c Release -r win-arm64 --no-self-contained
# publish output: bin/Release/net9.0/win-arm64/publish/
```

The publish directory is copied into the autounattend media at
`provisioner/autounattend/` and installed on first boot. A Task
Scheduler logon task named `TestAnywareAgent` starts the binary as
the `admin` user.

## Notes

- `--no-self-contained` is intentional — the Windows golden has the
  .NET 9 runtime installed, so the agent doesn't bundle it.
- Wire shapes are documented in
  [`docs/architecture/agent-protocol.md`](../../docs/architecture/agent-protocol.md).
- The autounattend XML also installs VirtIO networking drivers;
  without them the agent is unreachable from the host.

See [`docs/components/agents-windows.md`](../../docs/components/agents-windows.md)
for module layout, key files, and common pitfalls.
