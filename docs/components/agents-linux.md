# Component: `agents/linux/` — Linux in-VM agent

This Python 3.12+ package runs **inside** a Linux VM. It implements the
TestAnyware agent HTTP surface on port 8648 with `http.server`, so it
needs no framework. It uses AT-SPI2 through `python3-pyatspi` for
accessibility.

## Layout

```
agents/linux/
└── testanyware_agent/
    ├── __init__.py
    ├── __main__.py                # python -m testanyware_agent
    ├── server.py                  # threaded HTTP server, routing, SIGTERM shutdown
    ├── accessibility.py           # /windows, /snapshot, /inspect, /press, ...
    ├── system_endpoints.py        # /exec, /exec/cancel, /upload, /download, /shutdown, /health
    ├── exec_owner.py              # the lifecycle owner of one guest command
    ├── exec_registry.py           # exec ids, retained records, tombstones
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
- **One thread for each request** — a running guest command does not
  block health, cancellation or transfer. AT-SPI is not safe across
  threads, so the accessibility endpoints share one lock. The system
  endpoints never take it. Health reads `accessible` from a probe thread
  that holds the lock.
- **Bounded shutdown** — at SIGTERM the agent stops admission and
  cancels each running guest command. It waits up to 4 s for their
  records and then exits. The spec is
  `docs/specs/guest-command-lifecycle.md`.

## Endpoint wiring

Routing lives in `server.py`:

```python
GET  /health
POST /windows, /snapshot, /inspect
POST /press, /set-value, /focus, /show-menu
POST /window-focus, /window-resize, /window-move, /window-close, /window-minimize
POST /wait
POST /exec, /exec/cancel, /upload, /download, /shutdown
```

`docs/architecture/agent-protocol.md` documents the wire shapes.

## Build / test / run

**No build step.** The Linux golden image holds the agent as Python
source. A systemd user service (`testanyware-agent.service`) starts it.

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
- **Python 3.12+ required.** The `pyatspi` bindings match the system
  Python. Do not run the agent inside a venv with a different
  version.
- **Electron apps render black.** See
  [`docs/user/troubleshooting.md`](../user/troubleshooting.md).
  Start them with `--disable-gpu`.
- **Role mapping lives in code.** Each addition to the host-side
  `UnifiedRole` enum needs a matching entry in `role_mapper.py`.
  Without it, the agent returns `unknown`.
