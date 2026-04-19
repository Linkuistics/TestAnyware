"""Shape-analysis heuristic for simple geometric icons.

Ported from the Swift `IconClassifier.classifyWithShapeAnalysis` routine in the v1
archive. Handles obvious cases for plus, minus, close-x, checkmark and the four
chevrons; returns (None, 0.0) for anything it can't classify cleanly.

The function operates on the binarized foreground mask derived from an Otsu
threshold over the grayscale version of the input image. "Foreground" is whichever
value (dark or light) deviates from the dominant background.
"""

from __future__ import annotations

import numpy as np
from PIL import Image


def classify_by_shape(image: Image.Image) -> tuple[str | None, float]:
    """Classify a cropped icon image by shape heuristics.

    Returns `(label, confidence)` where confidence is a coarse 0.0-1.0 score
    reflecting how cleanly the shape matched, or `(None, 0.0)` if no shape
    matches confidently.
    """

    gray = np.asarray(image.convert("L"), dtype=np.uint8)
    h, w = gray.shape
    if w < 8 or h < 8:
        return (None, 0.0)

    threshold = _otsu_threshold(gray)
    bg_is_light = float(gray.mean()) > 128.0

    if bg_is_light:
        foreground = gray <= threshold
    else:
        foreground = gray >= threshold

    total = w * h
    fg_count = int(foreground.sum())
    fg_ratio = fg_count / total

    # Very low or very high foreground ratio = no clear shape
    if fg_ratio <= 0.05 or fg_ratio >= 0.7:
        return (None, 0.0)

    mid_x = w // 2
    mid_y = h // 2

    # Horizontal center strip: middle 60% of rows, full width
    h_top = h * 2 // 10
    h_bot = h * 8 // 10
    h_strip = foreground[h_top:h_bot, :]
    h_strip_fg = int(h_strip.sum())
    h_strip_total = h_strip.size
    h_strip_ratio = h_strip_fg / h_strip_total if h_strip_total else 0.0

    # Vertical center strip: middle 60% of columns, full height
    v_left = w * 2 // 10
    v_right = w * 8 // 10
    v_strip = foreground[:, v_left:v_right]
    v_strip_fg = int(v_strip.sum())
    v_strip_total = v_strip.size
    v_strip_ratio = v_strip_fg / v_strip_total if v_strip_total else 0.0

    # Count fg in the vertical strip that lies OUTSIDE the horizontal strip rows.
    # For a minus bar this is ~0; for a plus cross there's a visible vertical bar.
    v_only = foreground[:, v_left:v_right].copy()
    v_only[h_top:h_bot, :] = False
    v_only_fg = int(v_only.sum())
    v_only_ratio = v_only_fg / v_strip_total if v_strip_total else 0.0

    # Quadrant counts: TL, TR, BL, BR
    tl = int(foreground[:mid_y, :mid_x].sum())
    tr = int(foreground[:mid_y, mid_x:].sum())
    bl = int(foreground[mid_y:, :mid_x].sum())
    br = int(foreground[mid_y:, mid_x:].sum())

    left_fg = tl + bl
    right_fg = tr + br
    top_fg = tl + tr
    bottom_fg = bl + br

    # PLUS vs CLOSE-X: strong cross pattern (check before MINUS — a plus with a
    # tall vertical bar fully inside the horizontal strip would otherwise look
    # like a minus to the `v_only_ratio` check).
    corner_fg = tl + tr + bl + br
    cross_fg = h_strip_fg + v_strip_fg
    if (
        h_strip_ratio > 0.15
        and v_strip_ratio > 0.15
        and cross_fg / max(1, corner_fg) > 1.3
    ):
        diag = _diagonal_ratio(foreground)
        if diag > 0.6:
            return ("close-x", _clamp(0.4 + diag * 0.5))
        return ("plus", _clamp(0.4 + (h_strip_ratio + v_strip_ratio) * 0.5))

    # MINUS: horizontal bar only (no vertical extension)
    if h_strip_ratio > 0.15 and fg_ratio < 0.3 and v_only_ratio < 0.02:
        return ("minus", _clamp(0.5 + h_strip_ratio))

    # CHEVRONS: foreground concentrated on one side
    if 0.08 < fg_ratio < 0.4:
        lr_ratio = left_fg / max(1, right_fg)
        tb_ratio = top_fg / max(1, bottom_fg)

        if lr_ratio > 2.0:
            return ("chevron-left", _clamp(0.3 + min(lr_ratio, 5.0) / 10.0))
        if lr_ratio < 0.5:
            return ("chevron-right", _clamp(0.3 + min(1.0 / max(lr_ratio, 1e-3), 5.0) / 10.0))
        if tb_ratio > 2.0:
            return ("chevron-up", _clamp(0.3 + min(tb_ratio, 5.0) / 10.0))
        if tb_ratio < 0.5:
            return ("chevron-down", _clamp(0.3 + min(1.0 / max(tb_ratio, 1e-3), 5.0) / 10.0))

    # CHECKMARK: heavy bottom-left and bottom-right, bottom dominant over top
    if 0.08 < fg_ratio < 0.35:
        if bl > tl and bl > tr and br > tl and bottom_fg / max(1, top_fg) > 1.5:
            return ("checkmark", _clamp(0.3 + bottom_fg / max(1, top_fg) / 10.0))

    return (None, 0.0)


def _clamp(x: float) -> float:
    return max(0.0, min(1.0, x))


def _otsu_threshold(gray: np.ndarray) -> int:
    """Compute Otsu's threshold over a uint8 grayscale array."""

    hist, _ = np.histogram(gray, bins=256, range=(0, 256))
    total = gray.size
    if total == 0:
        return 128

    sum_all = float((np.arange(256) * hist).sum())
    sum_bg = 0.0
    weight_bg = 0
    best_t = 0
    best_var = 0.0

    for t in range(256):
        weight_bg += int(hist[t])
        if weight_bg == 0:
            continue
        weight_fg = total - weight_bg
        if weight_fg == 0:
            break
        sum_bg += float(t) * float(hist[t])
        mean_bg = sum_bg / weight_bg
        mean_fg = (sum_all - sum_bg) / weight_fg
        variance = weight_bg * weight_fg * (mean_bg - mean_fg) ** 2
        if variance > best_var:
            best_var = variance
            best_t = t

    return best_t


def _diagonal_ratio(foreground: np.ndarray) -> float:
    """Ratio of fg pixels near the two diagonals vs near the two axes.

    > 0.6 suggests an X, < 0.4 suggests a plus.
    """

    h, w = foreground.shape
    tolerance = max(2, min(w, h) // 8)
    mid_x = w // 2
    mid_y = h // 2

    ys, xs = np.nonzero(foreground)
    if xs.size == 0:
        return 0.5

    # Distance (scaled) to the two diagonals, following the Swift formulation.
    d1 = np.abs(xs * h - ys * w)
    d2 = np.abs(xs * h - (h - 1 - ys) * w)
    near_diag = np.minimum(d1, d2) < tolerance * max(w, h)

    near_axis = (np.abs(xs - mid_x) < tolerance) | (np.abs(ys - mid_y) < tolerance)

    diag_count = int(near_diag.sum())
    axis_count = int(near_axis.sum())
    total = diag_count + axis_count
    if total == 0:
        return 0.5
    return diag_count / total
