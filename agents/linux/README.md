# agents/linux/ — Linux in-VM agent

Python 3.12+ package that runs **inside** a Linux VM and exposes the
TestAnyware agent HTTP surface on port 8648 using stock `http.server`
and AT-SPI2 via `python3-pyatspi`.

## Working on this component

No build step. Run directly for development:

```bash
cd agents/linux
python3 -m testanyware_agent       # starts listening on 0.0.0.0:8648
```

In the golden image, the agent runs as a systemd user service named
`testanyware-agent.service`.

## Notes

- No venv: uses the system Python so `pyatspi` bindings match the
  interpreter.
- Wayland is disabled in the golden (`WaylandEnable=false` in GDM)
  because `xdotool` (used for the GTK4 coordinate fallback) needs
  X11.
- Wire shapes are documented in
  [`docs/architecture/agent-protocol.md`](../../docs/architecture/agent-protocol.md).
- Role mapping (AT-SPI → `UnifiedRole`) lives in `role_mapper.py`;
  add mappings there when the host-side `UnifiedRole` enum grows.

See [`docs/components/agents-linux.md`](../../docs/components/agents-linux.md)
for module layout, key files, and common pitfalls.
