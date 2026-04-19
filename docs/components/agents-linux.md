# Component: `agents/linux/` — Linux in-VM agent

Python 3.12+ package that runs **inside** a Linux VM. Implements the
TestAnyware agent HTTP surface on port 8648 using `http.server` (no
framework dependency) and AT-SPI2 via `python3-pyatspi` for
accessibility.

## Layout

```
agents/linux/
└── testanyware_agent/
    ├── __init__.py
    ├── __main__.py                # python -m testanyware_agent
    ├── server.py                  # HTTPRequestHandler, routing
    ├── accessibility.py           # /windows, /snapshot, /inspect, /press, ...
    ├── system_endpoints.py        # /exec, /upload, /download, /shutdown, /health
    ├── tree_walker.py             # AT-SPI2 tree traversal
    ├── query_resolver.py          # ElementQuery → element resolution
    ├── role_mapper.py             # AT-SPI role → UnifiedRole mapping
    └── models.py                  # Request/response dataclasses
```

## Key design notes

- **No framework** — deliberately uses `http.server` to keep the
  dependency graph tiny. Ubuntu's stock Python is the runtime.
- **AT-SPI2 + xdotool fallback** — AT-SPI2 is the primary API. For
  GTK4 apps that return `(0,0)` for all coordinates, the agent
  offsets via `xdotool` window search + `_GTK_FRAME_EXTENTS`
  (requires `WaylandEnable=false` in GDM).
- **No build** — ships as Python source. A wrapper script invokes
  `python3 -m testanyware_agent`.

## Endpoint wiring

Routing lives in `server.py`:

```python
GET  /health
POST /windows, /snapshot, /inspect
POST /press, /set-value, /focus, /show-menu
POST /window-focus, /window-resize, /window-move, /window-close, /window-minimize
POST /wait
POST /exec, /upload, /download, /shutdown
```

Wire shapes are documented in `docs/architecture/agent-protocol.md`.

## Build / test / run

**No build step.** Installed into the Linux golden image as Python
source; started by a systemd user service
(`testanyware-agent.service`).

```bash
cd agents/linux
python3 -m testanyware_agent                 # run locally for dev
python3 -m unittest discover testanyware_agent   # (unit tests if present)
```

Systemd unit (installed by the golden-image script):

```
systemctl --user start  testanyware-agent.service
systemctl --user status testanyware-agent.service
journalctl --user -u testanyware-agent.service
```

## Common pitfalls

- **Wayland vs X11.** AT-SPI2 works on both, but `xdotool` (used for
  window coordinate fallback) does not work on native Wayland
  sessions. The golden image forces X11 via `WaylandEnable=false`.
- **Python 3.12+ required.** `pyatspi` bindings match the system
  Python; don't try to run this inside a venv with a different
  version.
- **Electron apps render black.** See
  [`docs/user/troubleshooting.md`](../user/troubleshooting.md).
  Launch with `--disable-gpu`.
- **Role mapping lives in code.** Any addition to the host-side
  `UnifiedRole` enum needs a corresponding mapping in
  `role_mapper.py`; otherwise the agent returns `unknown`.
