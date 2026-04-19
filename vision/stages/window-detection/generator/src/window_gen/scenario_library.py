from __future__ import annotations

from window_gen.scenarios import WindowScenario, WindowSpec

_SCREEN_W = 1920
_SCREEN_H = 1080


def build_scenario_library() -> list[WindowScenario]:
    """Build the complete library of window detection training scenarios."""
    return [
        _empty_desktop(),
        _single_centered_window(),
        _single_maximized_window(),
        _single_small_window(),
        _two_side_by_side(),
        _two_overlapping(),
        _three_cascaded(),
        _four_tiled(),
        _many_small_windows(),
    ]


def _empty_desktop() -> WindowScenario:
    return WindowScenario(
        name="empty-desktop",
        description="Desktop with no application windows open",
        screen_width=_SCREEN_W,
        screen_height=_SCREEN_H,
        windows=[],
    )


def _single_centered_window() -> WindowScenario:
    return WindowScenario(
        name="single-centered",
        description="One medium window centered on screen",
        screen_width=_SCREEN_W,
        screen_height=_SCREEN_H,
        windows=[
            WindowSpec("TextEdit", "Untitled.txt", 360, 140, 1200, 800, z_order=0),
        ],
    )


def _single_maximized_window() -> WindowScenario:
    return WindowScenario(
        name="single-maximized",
        description="One window filling the entire screen (below menu bar)",
        screen_width=_SCREEN_W,
        screen_height=_SCREEN_H,
        windows=[
            WindowSpec("Safari", "Google", 0, 25, 1920, 1055, z_order=0),
        ],
    )


def _single_small_window() -> WindowScenario:
    return WindowScenario(
        name="single-small",
        description="One small dialog-sized window",
        screen_width=_SCREEN_W,
        screen_height=_SCREEN_H,
        windows=[
            WindowSpec("Finder", "Info", 600, 300, 300, 400, z_order=0),
        ],
    )


def _two_side_by_side() -> WindowScenario:
    return WindowScenario(
        name="two-side-by-side",
        description="Two windows side by side, no overlap",
        screen_width=_SCREEN_W,
        screen_height=_SCREEN_H,
        windows=[
            WindowSpec("TextEdit", "Left.txt", 0, 25, 960, 1055, z_order=0),
            WindowSpec("Safari", "Right Page", 960, 25, 960, 1055, z_order=1),
        ],
    )


def _two_overlapping() -> WindowScenario:
    return WindowScenario(
        name="two-overlapping",
        description="Two windows with significant overlap",
        screen_width=_SCREEN_W,
        screen_height=_SCREEN_H,
        windows=[
            WindowSpec("TextEdit", "Background.txt", 100, 100, 900, 700, z_order=0),
            WindowSpec("Safari", "Foreground", 400, 200, 900, 700, z_order=1),
        ],
    )


def _three_cascaded() -> WindowScenario:
    return WindowScenario(
        name="three-cascaded",
        description="Three windows cascaded diagonally",
        screen_width=_SCREEN_W,
        screen_height=_SCREEN_H,
        windows=[
            WindowSpec("TextEdit", "First.txt", 100, 100, 800, 600, z_order=0),
            WindowSpec("Safari", "Second", 250, 200, 800, 600, z_order=1),
            WindowSpec("Finder", "Third", 400, 300, 800, 600, z_order=2),
        ],
    )


def _four_tiled() -> WindowScenario:
    return WindowScenario(
        name="four-tiled",
        description="Four windows in a 2x2 grid",
        screen_width=_SCREEN_W,
        screen_height=_SCREEN_H,
        windows=[
            WindowSpec("TextEdit", "TopLeft.txt", 0, 25, 960, 528, z_order=0),
            WindowSpec("Safari", "TopRight", 960, 25, 960, 528, z_order=1),
            WindowSpec("Finder", "BottomLeft", 0, 553, 960, 527, z_order=2),
            WindowSpec("Terminal", "BottomRight", 960, 553, 960, 527, z_order=3),
        ],
    )


def _many_small_windows() -> WindowScenario:
    return WindowScenario(
        name="many-small-windows",
        description="Six small overlapping windows — stress test",
        screen_width=_SCREEN_W,
        screen_height=_SCREEN_H,
        windows=[
            WindowSpec("Finder", "Info 1", 50, 50, 350, 300, z_order=0),
            WindowSpec("Finder", "Info 2", 150, 100, 350, 300, z_order=1),
            WindowSpec("Finder", "Info 3", 250, 150, 350, 300, z_order=2),
            WindowSpec("TextEdit", "Note 1", 800, 50, 400, 350, z_order=3),
            WindowSpec("TextEdit", "Note 2", 900, 150, 400, 350, z_order=4),
            WindowSpec("Safari", "Page", 500, 500, 700, 500, z_order=5),
        ],
    )
