"""Walk the AT-SPI2 accessibility tree and produce ElementInfo dicts."""

from __future__ import annotations

import gi

gi.require_version("Atspi", "2.0")
from gi.repository import Atspi  # noqa: E402

import pyatspi  # noqa: E402

from testanyware_agent.models import ElementInfo  # noqa: E402
from testanyware_agent.role_mapper import map_role  # noqa: E402


def walk(root: pyatspi.Accessible, depth: int,
         role_filter: str | None = None,
         label_filter: str | None = None,
         offset_x: float = 0.0,
         offset_y: float = 0.0) -> list[ElementInfo]:
    """Walk the AT-SPI2 tree from root up to depth levels deep.

    Returns a list of ElementInfo for the root's children (not root itself).
    Filters by role and/or label if provided.

    offset_x/offset_y: screen coordinate offset to add to all element
    positions — fixes GTK4's AT-SPI bug where DESKTOP_COORDS returns
    content-area-relative coordinates instead of screen-absolute.
    """
    results: list[ElementInfo] = []
    try:
        child_count = root.childCount
    except Exception:
        return results

    for i in range(child_count):
        try:
            child = root.getChildAtIndex(i)
            if child is None:
                continue
        except Exception:
            continue

        info = _build_element_info(child, depth - 1, role_filter, label_filter,
                                   offset_x, offset_y)
        if info is not None:
            results.append(info)

    return results


def _build_element_info(
    accessible: pyatspi.Accessible,
    remaining_depth: int,
    role_filter: str | None,
    label_filter: str | None,
    offset_x: float = 0.0,
    offset_y: float = 0.0,
) -> ElementInfo | None:
    """Build an ElementInfo from an AT-SPI2 accessible, recursing into children."""
    try:
        atk_role_name = accessible.getRoleName()
    except Exception:
        return None

    unified_role = map_role(atk_role_name)

    try:
        name = accessible.name or None
    except Exception:
        name = None

    try:
        description = accessible.description or None
    except Exception:
        description = None

    # State — use Atspi GI bindings with clear_cache() to bypass pyatspi's
    # stale cache in this long-running process (no GLib main loop for updates).
    try:
        accessible.clear_cache()
        state_set = accessible.get_state_set()
        enabled = state_set.contains(Atspi.StateType.ENABLED)
        focused = state_set.contains(Atspi.StateType.FOCUSED)
        showing = state_set.contains(Atspi.StateType.SHOWING)
    except Exception:
        enabled = True
        focused = False
        showing = True

    # Value
    value = None
    try:
        value_iface = accessible.queryValue()
        current = value_iface.currentValue
        if current is not None:
            value = str(current)
    except (NotImplementedError, AttributeError):
        pass
    # For text entries, use the text content as value
    if value is None:
        try:
            text_iface = accessible.queryText()
            text_content = text_iface.getText(0, text_iface.characterCount)
            if text_content:
                value = text_content
        except (NotImplementedError, AttributeError):
            pass

    # Position and size — apply screen offset to fix GTK4 AT-SPI coordinate bug
    position_x = None
    position_y = None
    size_width = None
    size_height = None
    try:
        component = accessible.queryComponent()
        extents = component.getExtents(pyatspi.DESKTOP_COORDS)
        position_x = float(extents.x) + offset_x
        position_y = float(extents.y) + offset_y
        size_width = float(extents.width)
        size_height = float(extents.height)
    except (NotImplementedError, AttributeError):
        pass

    # Actions
    actions: list[str] = []
    try:
        action_iface = accessible.queryAction()
        for j in range(action_iface.nActions):
            action_name = action_iface.getName(j)
            if action_name:
                actions.append(action_name)
    except (NotImplementedError, AttributeError):
        pass

    # Automation ID
    element_id = None
    try:
        attrs = accessible.getAttributes()
        attr_dict = dict(a.split(":", 1) for a in attrs if ":" in a)
        element_id = attr_dict.get("id") or attr_dict.get("xml-id") or None
    except Exception:
        pass

    # Children
    try:
        child_count = accessible.childCount
    except Exception:
        child_count = 0

    children = None
    if remaining_depth > 0 and child_count > 0:
        children = []
        for i in range(child_count):
            try:
                child = accessible.getChildAtIndex(i)
                if child is None:
                    continue
            except Exception:
                continue
            child_info = _build_element_info(child, remaining_depth - 1,
                                             role_filter, label_filter,
                                             offset_x, offset_y)
            if child_info is not None:
                children.append(child_info)
        if not children:
            children = None

    # Apply filters — keep element if it matches OR has matching descendants
    if role_filter and unified_role != role_filter:
        if children is None:
            return None
        # Keep as container for matching descendants
    if label_filter and (name is None or label_filter.lower() not in name.lower()):
        if children is None and not (role_filter and unified_role == role_filter):
            return None

    return ElementInfo(
        role=unified_role,
        label=name,
        value=value,
        description=description,
        id=element_id,
        enabled=enabled,
        focused=focused,
        showing=showing,
        position_x=position_x,
        position_y=position_y,
        size_width=size_width,
        size_height=size_height,
        child_count=child_count,
        actions=actions,
        platform_role=atk_role_name if unified_role == "unknown" else None,
        children=children,
    )
