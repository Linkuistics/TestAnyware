# Connection Spec Schema

The JSON file `testanyware vm start` writes atomically (mode 0600) to
`$XDG_STATE_HOME/testanyware/vms/<id>.json` on boot. `testanyware vm stop`
removes it. The CLI resolves `--vm <id>` and `TESTANYWARE_VM_ID` by loading
this file. Downstream tooling that reads it directly can rely on the schema
below.

Source of truth: `cli/Sources/TestAnywareDriver/VM/VMSpec.swift`, which
reuses the `VNCSpec`, `AgentSpec`, and `Platform` types from
`cli/Sources/TestAnywareDriver/Connection/ConnectionSpec.swift`.

## Schema

```json
{
  "vnc":      { "host": "127.0.0.1", "port": 63530, "password": "..." },
  "agent":    { "host": "192.168.64.2", "port": 8648 },
  "platform": "macos"
}
```

### Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `vnc` | object | yes | VNC endpoint. Always present. |
| `vnc.host` | string | yes | VNC host (loopback for tart, LAN for QEMU). |
| `vnc.port` | integer | yes | VNC TCP port. Dynamically assigned per VM. |
| `vnc.password` | string | optional | VNC password. Present on tart (macOS/Linux) and QEMU (Windows, fixed `testanyware`). |
| `agent` | object | optional | Agent HTTP endpoint. Present when the agent reached health during startup (expected for all golden images). Absent if the boot wait timed out. |
| `agent.host` | string | yes (when `agent` present) | Agent host (VM LAN IP). |
| `agent.port` | integer | yes (when `agent` present) | Agent TCP port. Defaults to 8648 but can be overridden per-platform. |
| `platform` | string enum | yes | One of `macos`, `linux`, `windows`. |

Spec files written by VMs that started before SSH was disabled in the
goldens may still contain a top-level `ssh` field. The decoder ignores
unknown keys, so legacy specs continue to load; new specs do not emit it.

### Canonical writer / reader

- **Writer:** `VMSpec.writeAtomic(to:)` — pretty-printed, sorted keys,
  atomic rename from `<path>.tmp` to ensure readers never see a partial
  file. Mode 0600.
- **Reader:** `VMSpec.load(from:)` and `ConnectionSpec.load(from:)`.

### Sidecar: `<id>.meta.json`

`testanyware vm start` also writes a sibling `<id>.meta.json` in the
same directory. It contains PID, tool (`tart` or `qemu`), clone dir,
and viewer window id — consumed only by `testanyware vm stop`. **Clients
should not read or depend on its shape.**
