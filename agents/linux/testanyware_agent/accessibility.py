"""Accessibility endpoint handlers using AT-SPI2 via pyatspi2."""

from __future__ import annotations

import os
import re
import subprocess
import time

import pyatspi

from testanyware_agent.models import ElementInfo, WindowInfo
from testanyware_agent.query_resolver import resolve
from testanyware_agent.role_mapper import map_from_string, map_role
from testanyware_agent.tree_walker import walk


# Interactive roles — same set as macOS and Windows agents
_INTERACTIVE_ROLES = frozenset([
    "button", "checkbox", "radio", "textfield", "editable-text", "slider",
    "combo-box", "switch", "link", "menu-item", "tab", "disclosure-triangle",
    "color-well", "date-picker", "spin-button",
])


def is_accessible() -> bool:
    """Check if AT-SPI2 accessibility is working.

    Checks API capability (can we reach the desktop root?), not transient
    state. An empty desktop with no apps running is still accessible.
    """
    try:
        pyatspi.Registry.getDesktop(0)
        return True
    except Exception:
        return False


def _xdotool_search(args: list[str], env: dict) -> list[str]:
    """Run `xdotool search <args>` and return the resulting xids."""
    try:
        result = subprocess.run(
            ["xdotool", "search", *args],
            capture_output=True, timeout=5, env=env,
        )
    except Exception:
        return []
    if result.returncode != 0:
        return []
    out = result.stdout.strip()
    if not out:
        return []
    return out.decode().split("\n")


def _xdotool_window_geometry(xid: str, env: dict) -> tuple[int, int, int, int] | None:
    """Return (x, y, width, height) for an X11 window via xdotool, or None."""
    try:
        result = subprocess.run(
            ["xdotool", "getwindowgeometry", xid],
            capture_output=True, timeout=5, env=env,
        )
    except Exception:
        return None
    if result.returncode != 0:
        return None
    text = result.stdout.decode()
    pos = re.search(r"Position:\s*(\d+),(\d+)", text)
    size = re.search(r"Geometry:\s*(\d+)x(\d+)", text)
    if not pos or not size:
        return None
    return int(pos.group(1)), int(pos.group(2)), int(size.group(1)), int(size.group(2))


def _pick_matching_window(
    xids: list[str], atspi_w: float, atspi_h: float, env: dict,
) -> str | None:
    """Pick the X11 window whose geometry best matches AT-SPI dimensions.

    A single PID can own several X11 windows (popovers, tooltips, IPC
    helpers). Score each candidate by `|w-atspi_w| + |h-atspi_h|`, with a
    64px hard cap on each dimension to reject obviously-wrong matches and
    drop xdotool's 1x1 sentinel for unmapped windows.
    """
    best_xid: str | None = None
    best_score = float("inf")
    for xid in xids:
        geom = _xdotool_window_geometry(xid, env)
        if geom is None:
            continue
        _, _, w, h = geom
        if w <= 1 and h <= 1:
            continue
        dw = abs(w - atspi_w)
        dh = abs(h - atspi_h)
        if dw > 64 or dh > 64:
            continue
        score = dw + dh
        if score < best_score:
            best_score = score
            best_xid = xid
    return best_xid


def _compute_screen_offset(
    accessible: pyatspi.Accessible,
    atspi_x: float,
    atspi_y: float,
    atspi_w: float,
    atspi_h: float,
) -> tuple[float, float]:
    """Compute screen coordinate offset to fix GTK4's AT-SPI bug.

    GTK4 returns (0,0) for DESKTOP_COORDS instead of actual screen positions.
    When detected, we query the X11 window position and GTK frame extents to
    compute the correct offset from content-area origin to screen coordinates.

    Window lookup tries the application's PID first (covers GTK4 CSD apps
    whose X11 WM_NAME does not match the AT-SPI title — Nautilus shows "Home"
    in a GTK headerbar widget while WM_NAME is unset/different) and falls
    back to title-based search for GTK3 / non-CSD apps.

    Returns (offset_x, offset_y) to add to all AT-SPI coordinates.
    Returns (0, 0) if coordinates appear correct or cannot be corrected.
    """
    if atspi_x != 0.0 or atspi_y != 0.0:
        return 0.0, 0.0
    if atspi_w <= 0 or atspi_h <= 0:
        return 0.0, 0.0

    env = {**os.environ, "DISPLAY": ":0"}

    pid: int | None = None
    try:
        app = accessible.getApplication()
        if app is not None:
            pid = int(app.get_process_id())
    except Exception:
        pid = None

    title = ""
    try:
        title = accessible.name or ""
    except Exception:
        title = ""

    xid: str | None = None

    if pid is not None and pid > 0:
        candidates = _xdotool_search(["--pid", str(pid)], env)
        if candidates:
            xid = _pick_matching_window(candidates, atspi_w, atspi_h, env)

    if xid is None and title:
        candidates = _xdotool_search(["--name", title], env)
        if candidates:
            xid = _pick_matching_window(candidates, atspi_w, atspi_h, env)
            if xid is None:
                xid = candidates[0]

    if xid is None:
        return 0.0, 0.0

    geom = _xdotool_window_geometry(xid, env)
    if geom is None:
        return 0.0, 0.0
    win_x, win_y, _, _ = geom

    frame_left = 0
    frame_top = 0
    try:
        result = subprocess.run(
            ["xprop", "-id", xid, "_GTK_FRAME_EXTENTS"],
            capture_output=True, timeout=5, env=env,
        )
        if result.returncode == 0:
            ext_match = re.search(
                r"=\s*(\d+),\s*(\d+),\s*(\d+),\s*(\d+)",
                result.stdout.decode(),
            )
            if ext_match:
                frame_left = int(ext_match.group(1))
                frame_top = int(ext_match.group(3))
    except Exception:
        pass

    return float(win_x + frame_left), float(win_y + frame_top)


def _compute_screen_offset_for_accessible(
    accessible: pyatspi.Accessible,
) -> tuple[float, float]:
    """Convenience wrapper: extract AT-SPI extents, then compute offset."""
    try:
        component = accessible.queryComponent()
        extents = component.getExtents(pyatspi.DESKTOP_COORDS)
        return _compute_screen_offset(
            accessible,
            float(extents.x), float(extents.y),
            float(extents.width), float(extents.height),
        )
    except (NotImplementedError, AttributeError):
        return 0.0, 0.0


def enumerate_windows() -> list[WindowInfo]:
    """Enumerate all visible windows via AT-SPI2."""
    windows: list[WindowInfo] = []
    try:
        desktop = pyatspi.Registry.getDesktop(0)
    except Exception:
        return windows

    # Determine focused window
    focused_app_name = None
    focused_window_name = None
    try:
        for i in range(desktop.childCount):
            try:
                app = desktop.getChildAtIndex(i)
                if app is None:
                    continue
                for j in range(app.childCount):
                    try:
                        win = app.getChildAtIndex(j)
                        if win is None:
                            continue
                        state = win.getState()
                        if state.contains(pyatspi.STATE_ACTIVE):
                            focused_app_name = app.name
                            focused_window_name = win.name
                    except Exception:
                        continue
            except Exception:
                continue
    except Exception:
        pass

    for i in range(desktop.childCount):
        try:
            app = desktop.getChildAtIndex(i)
            if app is None:
                continue
            app_name = app.name or "Unknown"
        except Exception:
            continue

        for j in range(app.childCount):
            try:
                win = app.getChildAtIndex(j)
                if win is None:
                    continue

                role_name = win.getRoleName()
                if role_name not in ("frame", "window", "dialog"):
                    continue

                title = win.name or None
                window_type = _window_type_from_role(role_name)

                position_x = 0.0
                position_y = 0.0
                size_width = 0.0
                size_height = 0.0
                try:
                    component = win.queryComponent()
                    extents = component.getExtents(pyatspi.DESKTOP_COORDS)
                    position_x = float(extents.x)
                    position_y = float(extents.y)
                    size_width = float(extents.width)
                    size_height = float(extents.height)
                except (NotImplementedError, AttributeError):
                    pass

                # Fix GTK4 AT-SPI coordinate bug: apply screen offset
                off_x, off_y = _compute_screen_offset(
                    win, position_x, position_y, size_width, size_height)
                position_x += off_x
                position_y += off_y

                is_focused = (app_name == focused_app_name
                              and title == focused_window_name)

                windows.append(WindowInfo(
                    title=title,
                    window_type=window_type,
                    size_width=size_width,
                    size_height=size_height,
                    position_x=position_x,
                    position_y=position_y,
                    app_name=app_name,
                    focused=is_focused,
                ))
            except Exception:
                continue

    return windows


def handle_windows() -> tuple[int, dict]:
    windows = enumerate_windows()
    return 200, {"windows": [w.to_dict() for w in windows]}


def handle_snapshot(body: dict) -> tuple[int, dict]:
    # Clear the pyatspi cache so that element states (STATE_SHOWING, etc.)
    # reflect the current AT-SPI tree, not stale values from process startup.
    # Without this, a long-running agent process returns cached state because
    # it doesn't run a GLib main loop to process D-Bus state-change signals.
    pyatspi.clearCache()

    mode = body.get("mode", "interact")
    depth = body.get("depth", 3)
    role_filter = body.get("role")
    label_filter = body.get("label")
    window_filter = body.get("window")

    if role_filter:
        role_filter = map_from_string(role_filter)

    windows = enumerate_windows()
    if window_filter:
        windows = [w for w in windows if _window_matches(w, window_filter)]

    result_windows: list[dict] = []
    for win in windows:
        win_accessible = _find_window_accessible(win)
        if win_accessible is None:
            result_windows.append(win.to_dict())
            continue

        # Compute screen offset for GTK4 coordinate fix
        off_x, off_y = _compute_screen_offset_for_accessible(win_accessible)
        elements = walk(win_accessible, depth, role_filter, label_filter,
                        off_x, off_y)

        if mode == "interact":
            elements = _filter_interactive(elements)
        elif mode == "layout":
            elements = _filter_layout(elements)

        win_dict = win.to_dict()
        win_dict["elements"] = [e.to_dict() for e in elements]
        result_windows.append(win_dict)

    # Include the focused app's menu bar as a pseudo-window.
    # On Linux/GNOME, the menu bar is a sibling of the window frame in the
    # AT-SPI2 tree, so enumerate_windows() skips it.  Walk with depth=1 to
    # capture only top-level menu items (not dropdown contents).
    menu_bar_win = _focused_app_menu_bar(role_filter, label_filter, mode)
    if menu_bar_win is not None:
        if window_filter:
            if _window_matches(menu_bar_win, window_filter):
                result_windows.append(menu_bar_win.to_dict())
        else:
            result_windows.append(menu_bar_win.to_dict())

    return 200, {"windows": result_windows}


def handle_inspect(body: dict) -> tuple[int, dict]:
    role = body.get("role")
    label = body.get("label")
    element_id = body.get("id")
    index = body.get("index")
    window_filter = body.get("window")

    if role:
        role = map_from_string(role)

    windows = enumerate_windows()
    if window_filter:
        windows = [w for w in windows if _window_matches(w, window_filter)]

    all_elements: list[ElementInfo] = []
    # Track per-window offset for bounds correction
    _window_offsets: list[tuple[float, float]] = []
    for win in windows:
        win_accessible = _find_window_accessible(win)
        if win_accessible is None:
            continue
        off_x, off_y = _compute_screen_offset_for_accessible(win_accessible)
        _window_offsets.append((off_x, off_y))
        all_elements.extend(walk(win_accessible, 10, role, label, off_x, off_y))

    result_type, element, matches = resolve(all_elements, role, label,
                                            element_id, index)

    if result_type == "not_found":
        return 400, {"error": "No element found matching query"}
    if result_type == "multiple":
        return 400, {
            "error": "Multiple elements matched",
            "details": "\n".join(_describe_element(e) for e in (matches or [])),
        }

    element_dict = element.to_dict()  # type: ignore[union-attr]
    response: dict = {"element": element_dict}

    # Query live element for fresh bounding rect (mirrors macOS/Windows behavior).
    # All four bounds fields must be present or all omitted — the Swift decoder
    # requires all-or-nothing to reconstruct CGRect.
    live = _find_live_element(element)  # type: ignore[arg-type]
    if live is not None:
        try:
            component = live.queryComponent()
            extents = component.getExtents(pyatspi.DESKTOP_COORDS)
            if extents.width > 0 or extents.height > 0:
                # Apply GTK4 offset correction to bounds
                bounds_off_x, bounds_off_y = 0.0, 0.0
                if _window_offsets:
                    bounds_off_x, bounds_off_y = _window_offsets[0]
                response["boundsX"] = float(extents.x) + bounds_off_x
                response["boundsY"] = float(extents.y) + bounds_off_y
                response["boundsWidth"] = float(extents.width)
                response["boundsHeight"] = float(extents.height)
        except (NotImplementedError, AttributeError):
            pass

    return 200, response


def handle_action(body: dict, action_name: str) -> tuple[int, dict]:
    """Handle /press, /focus, /show-menu endpoints."""
    role = body.get("role")
    label = body.get("label")
    element_id = body.get("id")
    index = body.get("index")
    window_filter = body.get("window")

    if role:
        role = map_from_string(role)

    windows = enumerate_windows()
    if window_filter:
        windows = [w for w in windows if _window_matches(w, window_filter)]

    if not windows:
        return 400, {"error": "No matching windows found"}

    all_elements: list[ElementInfo] = []
    for win in windows:
        win_accessible = _find_window_accessible(win)
        if win_accessible is None:
            continue
        off_x, off_y = _compute_screen_offset_for_accessible(win_accessible)
        all_elements.extend(walk(win_accessible, 10, role, label, off_x, off_y))

    result_type, element, matches = resolve(all_elements, role, label,
                                            element_id, index)

    if result_type == "not_found":
        return 400, {"error": "No element found matching query"}
    if result_type == "multiple":
        return 400, {
            "error": "Multiple elements matched \u2014 refine your query or use index",
            "details": "\n".join(_describe_element(e) for e in (matches or [])),
        }

    live = _find_live_element(element)  # type: ignore[arg-type]
    if live is None:
        return 400, {"error": "Element found in snapshot but could not locate live AT-SPI2 element"}

    try:
        if action_name == "press":
            _perform_press(live)
        elif action_name == "focus":
            _perform_focus(live)
        elif action_name == "show-menu":
            _perform_show_menu(live)
        return 200, {"success": True, "message": f"{action_name} performed successfully"}
    except Exception as e:
        return 200, {"success": False, "message": f"{action_name} failed: {e}"}


def handle_set_value(body: dict) -> tuple[int, dict]:
    value = body.get("value", "")
    query_body = {k: v for k, v in body.items() if k != "value"}

    role = query_body.get("role")
    label = query_body.get("label")
    element_id = query_body.get("id")
    index = query_body.get("index")
    window_filter = query_body.get("window")

    if role:
        role = map_from_string(role)

    windows = enumerate_windows()
    if window_filter:
        windows = [w for w in windows if _window_matches(w, window_filter)]

    if not windows:
        return 400, {"error": "No matching windows found"}

    all_elements: list[ElementInfo] = []
    for win in windows:
        win_accessible = _find_window_accessible(win)
        if win_accessible is None:
            continue
        off_x, off_y = _compute_screen_offset_for_accessible(win_accessible)
        all_elements.extend(walk(win_accessible, 10, role, label, off_x, off_y))

    result_type, element, matches = resolve(all_elements, role, label,
                                            element_id, index)

    if result_type == "not_found":
        return 400, {"error": "No element found matching query"}
    if result_type == "multiple":
        return 400, {
            "error": "Multiple elements matched \u2014 refine your query or use index",
            "details": "\n".join(_describe_element(e) for e in (matches or [])),
        }

    live = _find_live_element(element)  # type: ignore[arg-type]
    if live is None:
        return 400, {"error": "Element found in snapshot but could not locate live AT-SPI2 element"}

    try:
        _perform_set_value(live, value)
        return 200, {"success": True, "message": "set-value performed successfully"}
    except Exception as e:
        return 200, {"success": False, "message": f"set-value failed: {e}"}


def handle_window_action(body: dict, action_name: str) -> tuple[int, dict]:
    """Handle /window-focus, /window-close, /window-minimize."""
    window_filter = body.get("window", "")

    windows = enumerate_windows()
    matching = [w for w in windows if _window_matches(w, window_filter)]

    if not matching:
        return 400, {"error": f"No window matching '{window_filter}'"}

    win = matching[0]
    win_accessible = _find_window_accessible(win)
    if win_accessible is None:
        return 400, {"error": f"No window matching '{window_filter}'"}

    try:
        if action_name == "window-focus":
            try:
                component = win_accessible.queryComponent()
                component.grabFocus()
            except (NotImplementedError, AttributeError):
                pass
            # Also try raising via atspi action
            try:
                action_iface = win_accessible.queryAction()
                for j in range(action_iface.nActions):
                    if action_iface.getName(j) in ("activate", "raise"):
                        action_iface.doAction(j)
                        break
            except (NotImplementedError, AttributeError):
                pass
            return 200, {"success": True, "message": "Window focused successfully"}

        elif action_name == "window-close":
            try:
                action_iface = win_accessible.queryAction()
                for j in range(action_iface.nActions):
                    if action_iface.getName(j) == "close":
                        action_iface.doAction(j)
                        return 200, {"success": True, "message": "Window closed successfully"}
            except (NotImplementedError, AttributeError):
                pass
            # Fallback: xdotool (X11 only — will fail under Wayland)
            xid = _get_window_xid(win_accessible)
            if xid == 0:
                return 200, {"success": False, "message": "window-close: no X11 window ID (Wayland session?)"}
            result = subprocess.run(["xdotool", "windowclose", str(xid)],
                                    capture_output=True, timeout=5)
            if result.returncode != 0:
                return 200, {"success": False, "message": f"window-close: xdotool failed: {result.stderr.decode().strip()}"}
            return 200, {"success": True, "message": "Window closed successfully"}

        elif action_name == "window-minimize":
            try:
                action_iface = win_accessible.queryAction()
                for j in range(action_iface.nActions):
                    if action_iface.getName(j) == "minimize":
                        action_iface.doAction(j)
                        return 200, {"success": True, "message": "Window minimized successfully"}
            except (NotImplementedError, AttributeError):
                pass
            xid = _get_window_xid(win_accessible)
            if xid == 0:
                return 200, {"success": False, "message": "window-minimize: no X11 window ID (Wayland session?)"}
            result = subprocess.run(["xdotool", "windowminimize", str(xid)],
                                    capture_output=True, timeout=5)
            if result.returncode != 0:
                return 200, {"success": False, "message": f"window-minimize: xdotool failed: {result.stderr.decode().strip()}"}
            return 200, {"success": True, "message": "Window minimized successfully"}

        return 200, {"success": False, "message": f"Unknown action: {action_name}"}

    except Exception as e:
        return 200, {"success": False, "message": f"{action_name} failed: {e}"}


def handle_window_resize(body: dict) -> tuple[int, dict]:
    window_filter = body.get("window", "")
    width = body.get("width", 0)
    height = body.get("height", 0)

    windows = enumerate_windows()
    matching = [w for w in windows if _window_matches(w, window_filter)]

    if not matching:
        return 400, {"error": f"No window matching '{window_filter}'"}

    win = matching[0]
    win_accessible = _find_window_accessible(win)
    if win_accessible is None:
        return 400, {"error": f"No window matching '{window_filter}'"}

    xid = _get_window_xid(win_accessible)
    if xid == 0:
        return 200, {"success": False, "message": "window-resize: no X11 window ID (Wayland session?)"}
    try:
        result = subprocess.run(["xdotool", "windowsize", str(xid), str(width), str(height)],
                                capture_output=True, timeout=5)
        if result.returncode != 0:
            return 200, {"success": False, "message": f"window-resize: xdotool failed: {result.stderr.decode().strip()}"}
        return 200, {"success": True, "message": f"Window resized to {width}\u00d7{height}"}
    except Exception as e:
        return 200, {"success": False, "message": f"window-resize failed: {e}"}


def handle_window_move(body: dict) -> tuple[int, dict]:
    window_filter = body.get("window", "")
    x = body.get("x", 0)
    y = body.get("y", 0)

    windows = enumerate_windows()
    matching = [w for w in windows if _window_matches(w, window_filter)]

    if not matching:
        return 400, {"error": f"No window matching '{window_filter}'"}

    win = matching[0]
    win_accessible = _find_window_accessible(win)
    if win_accessible is None:
        return 400, {"error": f"No window matching '{window_filter}'"}

    xid = _get_window_xid(win_accessible)
    if xid == 0:
        return 200, {"success": False, "message": "window-move: no X11 window ID (Wayland session?)"}
    try:
        result = subprocess.run(["xdotool", "windowmove", str(xid), str(x), str(y)],
                                capture_output=True, timeout=5)
        if result.returncode != 0:
            return 200, {"success": False, "message": f"window-move: xdotool failed: {result.stderr.decode().strip()}"}
        return 200, {"success": True, "message": f"Window moved to ({x}, {y})"}
    except Exception as e:
        return 200, {"success": False, "message": f"window-move failed: {e}"}


def handle_wait(body: dict) -> tuple[int, dict]:
    timeout = body.get("timeout", 10)
    window_filter = body.get("window")
    deadline = time.time() + timeout

    while time.time() < deadline:
        windows = enumerate_windows()
        if window_filter:
            windows = [w for w in windows if _window_matches(w, window_filter)]
        if windows:
            return 200, {"success": True, "message": "Accessibility ready"}
        time.sleep(0.5)

    return 200, {"success": False, "message": "Timed out waiting for accessibility"}


# --- Internal helpers ---


def _focused_app_menu_bar(
    role_filter: str | None,
    label_filter: str | None,
    mode: str,
) -> WindowInfo | None:
    """Return the focused app's menu bar as a pseudo-window, if available.

    On Linux/GNOME, the menu bar is a child of the AT-SPI2 application with
    role "menu bar" — a sibling of the window frames.  Uses depth=1 so only
    the top-level menu items are included, not dropdown contents.
    """
    try:
        desktop = pyatspi.Registry.getDesktop(0)
    except Exception:
        return None

    # Find the focused app
    focused_app = None
    for i in range(desktop.childCount):
        try:
            app = desktop.getChildAtIndex(i)
            if app is None:
                continue
            for j in range(app.childCount):
                try:
                    child = app.getChildAtIndex(j)
                    if child is None:
                        continue
                    state = child.getState()
                    if state.contains(pyatspi.STATE_ACTIVE):
                        focused_app = app
                        break
                except Exception:
                    continue
            if focused_app is not None:
                break
        except Exception:
            continue

    if focused_app is None:
        return None

    app_name = focused_app.name or "Unknown"

    # Find the menu bar among the app's children
    for i in range(focused_app.childCount):
        try:
            child = focused_app.getChildAtIndex(i)
            if child is None:
                continue
            if child.getRoleName() != "menu bar":
                continue

            elements = walk(child, 1, role_filter, label_filter)

            if mode == "interact":
                elements = _filter_interactive(elements)
            elif mode == "layout":
                elements = _filter_layout(elements)

            position_x = 0.0
            position_y = 0.0
            size_width = 0.0
            size_height = 0.0
            try:
                component = child.queryComponent()
                extents = component.getExtents(pyatspi.DESKTOP_COORDS)
                position_x = float(extents.x)
                position_y = float(extents.y)
                size_width = float(extents.width)
                size_height = float(extents.height)
            except (NotImplementedError, AttributeError):
                pass

            win = WindowInfo(
                title="Menu Bar",
                window_type="menuBar",
                size_width=size_width,
                size_height=size_height,
                position_x=position_x,
                position_y=position_y,
                app_name=app_name,
                focused=False,
                elements=elements,
            )
            return win
        except Exception:
            continue

    return None


def _window_matches(window: WindowInfo, filter_str: str) -> bool:
    f = filter_str.lower()
    if window.title and f in window.title.lower():
        return True
    if f in window.app_name.lower():
        return True
    return False


def _window_type_from_role(role_name: str) -> str:
    if role_name == "dialog":
        return "dialog"
    return "standard"


def _find_window_accessible(win: WindowInfo) -> pyatspi.Accessible | None:
    """Find the live AT-SPI2 accessible matching a WindowInfo.

    Matches by app name + title + size. Position is not used for matching
    because enumerate_windows() may have applied a GTK4 offset correction
    that makes the stored position differ from the raw AT-SPI position.
    """
    try:
        desktop = pyatspi.Registry.getDesktop(0)
    except Exception:
        return None

    for i in range(desktop.childCount):
        try:
            app = desktop.getChildAtIndex(i)
            if app is None:
                continue
            app_name = app.name or "Unknown"
            if app_name != win.app_name:
                continue

            for j in range(app.childCount):
                try:
                    child = app.getChildAtIndex(j)
                    if child is None:
                        continue
                    role_name = child.getRoleName()
                    if role_name not in ("frame", "window", "dialog"):
                        continue

                    # Match by title and size (not position — may have offset)
                    if child.name != win.title:
                        continue
                    try:
                        component = child.queryComponent()
                        extents = component.getExtents(pyatspi.DESKTOP_COORDS)
                        if (extents.width == win.size_width and
                                extents.height == win.size_height):
                            return child
                    except (NotImplementedError, AttributeError):
                        # Size check failed, match by title alone
                        return child
                except Exception:
                    continue
        except Exception:
            continue

    return None


def _find_live_element(info: ElementInfo) -> pyatspi.Accessible | None:
    """Find a live AT-SPI2 accessible matching an ElementInfo."""
    try:
        desktop = pyatspi.Registry.getDesktop(0)
    except Exception:
        return None

    for i in range(desktop.childCount):
        try:
            app = desktop.getChildAtIndex(i)
            if app is None:
                continue
            for j in range(app.childCount):
                try:
                    win = app.getChildAtIndex(j)
                    if win is None:
                        continue
                    found = _search_live_tree(win, info)
                    if found is not None:
                        return found
                except Exception:
                    continue
        except Exception:
            continue

    return None


def _search_live_tree(root: pyatspi.Accessible,
                      info: ElementInfo) -> pyatspi.Accessible | None:
    """Depth-first search for a matching live element."""
    try:
        child_count = root.childCount
    except Exception:
        return None

    for i in range(child_count):
        try:
            child = root.getChildAtIndex(i)
            if child is None:
                continue
            if _live_element_matches(child, info):
                return child
            found = _search_live_tree(child, info)
            if found is not None:
                return found
        except Exception:
            continue

    return None


def _live_element_matches(accessible: pyatspi.Accessible,
                          info: ElementInfo) -> bool:
    """Check if a live accessible matches an ElementInfo.

    Matches by role and label only — not by position, because the ElementInfo
    may have offset-corrected coordinates that don't match raw AT-SPI values
    (GTK4 coordinate fix).
    """
    try:
        atk_role_name = accessible.getRoleName()
        unified_role = map_role(atk_role_name)
        if unified_role != info.role:
            return False
    except Exception:
        return False

    try:
        name = accessible.name or None
        if info.label != name:
            return False
    except Exception:
        return False

    return True


def _perform_press(accessible: pyatspi.Accessible) -> None:
    """Press/click an element via AT-SPI2 action interface."""
    action_iface = accessible.queryAction()
    for j in range(action_iface.nActions):
        name = action_iface.getName(j)
        if name in ("click", "press", "activate", "invoke"):
            action_iface.doAction(j)
            return
    # Fallback: do action 0
    if action_iface.nActions > 0:
        action_iface.doAction(0)
        return
    raise RuntimeError("Element has no actionable press/click action")


def _perform_focus(accessible: pyatspi.Accessible) -> None:
    """Focus an element via AT-SPI2 component interface."""
    try:
        component = accessible.queryComponent()
        component.grabFocus()
    except (NotImplementedError, AttributeError):
        # Try via action
        action_iface = accessible.queryAction()
        for j in range(action_iface.nActions):
            if action_iface.getName(j) == "focus":
                action_iface.doAction(j)
                return
        raise RuntimeError("Element does not support focus")


def _perform_show_menu(accessible: pyatspi.Accessible) -> None:
    """Show context menu for an element."""
    action_iface = accessible.queryAction()
    for j in range(action_iface.nActions):
        name = action_iface.getName(j)
        if name in ("menu", "showMenu", "show-menu", "popup"):
            action_iface.doAction(j)
            return
    raise RuntimeError("Element has no menu action")


def _perform_set_value(accessible: pyatspi.Accessible, value: str) -> None:
    """Set the value of an element via AT-SPI2 editable text or value interface."""
    # Try editable text first
    try:
        text_iface = accessible.queryEditableText()
        text_iface.setTextContents(value)
        return
    except (NotImplementedError, AttributeError):
        pass

    # Try value interface (for sliders, etc.)
    try:
        value_iface = accessible.queryValue()
        value_iface.currentValue = float(value)
        return
    except (NotImplementedError, AttributeError, ValueError):
        pass

    raise RuntimeError("Element does not support setting value")


def _get_window_xid(accessible: pyatspi.Accessible) -> int:
    """Get the X11 window ID for xdotool fallback."""
    # Try to get from accessible attributes
    try:
        attrs = accessible.getAttributes()
        attr_dict = dict(a.split(":", 1) for a in attrs if ":" in a)
        xid = attr_dict.get("window-id")
        if xid:
            return int(xid)
    except Exception:
        pass
    return 0


def _filter_interactive(elements: list[ElementInfo]) -> list[ElementInfo]:
    """Keep only interactive elements and their ancestors."""
    result: list[ElementInfo] = []
    for element in elements:
        filtered = _filter_interactive_element(element)
        if filtered is not None:
            result.append(filtered)
    return result


def _filter_interactive_element(element: ElementInfo) -> ElementInfo | None:
    filtered_children = (
        _filter_interactive(element.children) if element.children else []
    )
    self_interactive = _is_interactive(element)
    if not self_interactive and not filtered_children:
        return None

    return ElementInfo(
        role=element.role,
        label=element.label,
        value=element.value,
        description=element.description,
        id=element.id,
        enabled=element.enabled,
        focused=element.focused,
        showing=element.showing,
        position_x=element.position_x,
        position_y=element.position_y,
        size_width=element.size_width,
        size_height=element.size_height,
        child_count=element.child_count,
        actions=element.actions,
        platform_role=element.platform_role,
        children=filtered_children if filtered_children else None,
    )


def _is_interactive(element: ElementInfo) -> bool:
    if element.actions:
        return True
    if element.focused:
        return True
    return element.role in _INTERACTIVE_ROLES


def _filter_layout(elements: list[ElementInfo]) -> list[ElementInfo]:
    """Keep only elements with geometry and their ancestors."""
    result: list[ElementInfo] = []
    for element in elements:
        filtered = _filter_layout_element(element)
        if filtered is not None:
            result.append(filtered)
    return result


def _filter_layout_element(element: ElementInfo) -> ElementInfo | None:
    filtered_children = (
        _filter_layout(element.children) if element.children else []
    )
    has_geometry = element.position_x is not None and element.size_width is not None
    if not has_geometry and not filtered_children:
        return None

    return ElementInfo(
        role=element.role,
        label=element.label,
        value=element.value,
        description=element.description,
        id=element.id,
        enabled=element.enabled,
        focused=element.focused,
        showing=element.showing,
        position_x=element.position_x,
        position_y=element.position_y,
        size_width=element.size_width,
        size_height=element.size_height,
        child_count=element.child_count,
        actions=element.actions,
        platform_role=element.platform_role,
        children=filtered_children if filtered_children else None,
    )


def _describe_element(info: ElementInfo) -> str:
    parts = [info.role]
    if info.label:
        parts.append(f"label={info.label}")
    if info.id:
        parts.append(f"id={info.id}")
    if info.position_x is not None and info.position_y is not None:
        parts.append(f"pos=({int(info.position_x)},{int(info.position_y)})")
    return " ".join(parts)
