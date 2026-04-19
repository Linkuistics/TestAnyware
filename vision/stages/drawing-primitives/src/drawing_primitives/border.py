"""Border detection via Canny edge detection.

Simplified port of Redraw tier3/border.py::detect_border. MVP returns only
``{"has_border": bool, "width": int}`` — color and radius are deferred.
"""

from __future__ import annotations

import cv2
import numpy as np
from PIL import Image


def detect_border(img: Image.Image) -> dict:
    """Return a simple border descriptor for ``img``.

    Runs Canny edge detection on the grayscale crop and, if a meaningful
    proportion of edge pixels cluster near the perimeter, reports a border.

    Args:
        img: Element image crop.

    Returns:
        Dict with ``has_border`` (bool) and ``width`` (int pixels).
    """
    if img.mode != "RGB":
        img = img.convert("RGB")

    arr = np.asarray(img)
    if arr.size == 0 or arr.shape[0] < 4 or arr.shape[1] < 4:
        return {"has_border": False, "width": 0}

    gray = cv2.cvtColor(arr, cv2.COLOR_RGB2GRAY)
    edges = cv2.Canny(gray, 50, 150)

    h, w = edges.shape
    ring = max(1, min(h, w) // 10)
    perimeter_mask = np.zeros_like(edges, dtype=bool)
    perimeter_mask[:ring, :] = True
    perimeter_mask[-ring:, :] = True
    perimeter_mask[:, :ring] = True
    perimeter_mask[:, -ring:] = True

    perim_edges = edges[perimeter_mask]
    if perim_edges.size == 0:
        return {"has_border": False, "width": 0}

    perim_density = float((perim_edges > 0).mean())
    interior = edges[~perimeter_mask]
    interior_density = float((interior > 0).mean()) if interior.size else 0.0

    has_border = perim_density > 0.15 and perim_density > interior_density * 1.5

    if not has_border:
        return {"has_border": False, "width": 0}

    # Estimate width by scanning inward rows/cols for where edge density
    # drops off from the perimeter maximum.
    width = 1
    max_width = max(1, min(h, w) // 4)
    for bw in range(2, max_width + 1):
        row_top = edges[bw - 1, :]
        row_bot = edges[h - bw, :]
        col_l = edges[:, bw - 1]
        col_r = edges[:, w - bw]
        band_density = float(
            (
                (row_top > 0).sum()
                + (row_bot > 0).sum()
                + (col_l > 0).sum()
                + (col_r > 0).sum()
            )
            / (2 * h + 2 * w)
        )
        if band_density > 0.1:
            width = bw
        else:
            break

    return {"has_border": True, "width": width}
