# Component: `agents/windows/` — Windows in-VM agent

C# / ASP.NET Core project that runs **inside** a Windows 11 ARM64 VM.
Implements the TestAnyware agent HTTP surface on port 8648 using UI
Automation via [FlaUI](https://github.com/FlaUI/FlaUI).

## Layout

```
agents/windows/
├── TestAnywareAgent.csproj
├── Program.cs                       # app bootstrap, endpoint registration
├── AccessibilityEndpoints.cs        # /windows, /snapshot, /inspect, /press, ...
├── SystemEndpoints.cs               # /health, /exec, /upload, /download, /shutdown
├── Services/                        # UIA helpers, window enumerator, tree walker
├── Models/
│   ├── ElementInfo.cs
│   ├── Requests.cs                  # ElementQuery, SnapshotRequest, ExecRequest, ...
│   ├── Responses.cs
│   ├── UnifiedRole.cs               # C# mirror of UnifiedRole
│   └── WindowInfo.cs
├── bin/, obj/                       # build outputs (gitignored)
```

## Key design notes

- **ASP.NET minimal APIs** — one `WebApplication`, endpoints wired via
  `app.MapGet` / `app.MapPost`.
- **FlaUI for UIA** — abstracts the differences between UIA2, UIA3,
  and various native COM quirks.
- **Self-contained publish.** Installed into the Windows golden image
  as a single-folder `.NET 9 win-arm64` publish; no runtime install
  required.

## Endpoint wiring

From `Program.cs` / `SystemEndpoints.cs` / `AccessibilityEndpoints.cs`:

```
GET  /health
POST /windows, /snapshot, /inspect
POST /press, /set-value, /focus, /show-menu
POST /window-focus, /window-resize, /window-move, /window-close, /window-minimize
POST /wait
POST /exec, /upload, /download, /shutdown
```

Wire shapes are documented in `docs/architecture/agent-protocol.md`.

## Build / test

**Cross-built on the macOS host** (no Windows build machine needed):

```bash
cd agents/windows
dotnet build -r win-arm64 --no-self-contained   # produces ARM64 binaries
dotnet publish -c Release -r win-arm64 --no-self-contained
# Publish output ends up at bin/Release/net9.0/win-arm64/publish/
dotnet test                                      # unit tests (if present)
```

The `--no-self-contained` flag keeps the publish small — the Windows
golden image has the .NET 9 runtime installed, so the agent doesn't
need to bundle it.

The publish directory is copied into the autounattend media that
installs the Windows golden image; Task Scheduler registers a logon
task named `TestAnywareAgent` that starts the binary as the `admin`
user at desktop login.

## Common pitfalls

- **UIA ids differ from AX ids.** `ElementInfo.id` will never match a
  macOS AX `id` for "the same" element — they come from different
  subsystems. Use role + label + window for cross-platform selectors.
- **Virtio networking driver.** Without the VirtIO net driver
  installed during OOBE (via `autounattend.xml`), the agent will be
  unreachable from the host. This is handled in the golden image
  build; worth remembering if you customise the autounattend.
- **First-logon OOBE animation.** Disabled in the golden. If you
  start a VM with `--viewer` and the animation appears, the golden
  image build skipped a customisation step.
- **Shell-style exec.** `/exec` runs via `cmd.exe /c` so shell
  metacharacters behave as expected; this differs from Linux
  (`sh -c`) only in quoting rules.
