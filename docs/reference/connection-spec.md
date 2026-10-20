# Connection Spec Schema

The JSON file `testanyware vm start` writes atomically (mode 0600) to
`$XDG_STATE_HOME/testanyware/vms/<id>.json` on boot. `testanyware vm stop`
removes it. The CLI resolves `--vm <id>` and `TESTANYWARE_VM_ID` by loading
this file.

**The file is not a contract for direct readers.** On a macOS host, the
TestAnyware app reads the file of each VM, and it alone can reach the endpoints
in it. Other tools must not read the file, and must not depend on its location
or its shape.

Use `--vm <id>` or `TESTANYWARE_VM_ID` with a `testanyware`
command. `--connect <path>` still accepts a file of the shape below. Each field
of `vm start --json` and `vm list --json` keeps its meaning. An endpoint in
them is the guest's own endpoint.

Source of truth: `cli-rs/crates/testanyware-vm/src/spec.rs` (the on-disk
`<id>.json` writer/reader), which carries the VNC, agent, and platform
fields the host CLI resolves.

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
| `vnc.host` | string | yes | VNC host. Loopback for tart, and `localhost` for QEMU. |
| `vnc.port` | integer | yes | VNC TCP port. Dynamically assigned per VM. |
| `vnc.password` | string | optional | VNC password. Present on tart (macOS/Linux) and QEMU (Windows, fixed `testanyware`). |
| `agent` | object | optional | Agent HTTP endpoint. Present in every spec a successful `vm start` writes, on every backend: a successful start means the agent reached health. A spec kept by `vm start --keep-on-failure` names the agent endpoint if it is known, even though the agent never became healthy. |
| `agent.host` | string | yes (when `agent` present) | Agent host. The guest's IP for tart, and `localhost` for QEMU, which forwards the agent port from the host. |
| `agent.port` | integer | yes (when `agent` present) | Agent TCP port. Defaults to 8648 but can be overridden per-platform. |
| `platform` | string enum | yes | One of `macos`, `linux`, `windows`. |

Spec files written by VMs that started before SSH was disabled in the
goldens may still contain a top-level `ssh` field. The decoder ignores
unknown keys, so legacy specs continue to load. New specs do not emit it.

### Canonical writer / reader

- **Writer:** `VMSpec.writeAtomic(to:)` — pretty-printed, sorted keys,
  atomic rename from `<path>.tmp` to ensure readers never see a partial
  file. Mode 0600.
- **Reader:** `VMSpec.load(from:)` and `ConnectionSpec.load(from:)`.

### Sidecar: `<id>.meta.json`

`testanyware vm start` also writes a sibling `<id>.meta.json` in the
same directory. It contains PID, tool (`tart` or `qemu`), clone dir,
and viewer window id — consumed only by `testanyware vm stop`. **Clients
should not read or depend on its shape.** The same holds for `<id>.json`, as
described at the top of this page.
