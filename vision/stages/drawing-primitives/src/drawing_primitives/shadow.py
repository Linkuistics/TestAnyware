"""Drop-shadow detection via gradient-magnitude comparison.

Simplified port of Redraw tier3/shadow.py::detect_shadow. MVP returns only
``{"has_shadow": bool}`` — offset/blur/color are deferred.
"""

from __future__ import annotations

import numpy as np
from PIL import Image

from testanyware_common.types import BoundingBox


def _luminance(rgb: np.ndarray) -> np.ndarray:
    return 0.299 * rgb[..., 0] + 0.587 * rgb[..., 1] + 0.114 * rgb[..., 2]


def detect_shadow(img: Image.Image, bbox: BoundingBox, margin: int = 12) -> dict:
    """Return a simple drop-shadow descriptor for an element.

    Compares luminance in the strips immediately below and to the right of
    ``bbox`` against the image's estimated background luminance. If either
    strip is materially darker than the background (and the region outside
    the strip), we report a shadow.

    Args:
        img: Full screenshot.
        bbox: Element bounding box in image coordinates.
        margin: How many pixels below/right of the element to sample.

    Returns:
        Dict with ``has_shadow`` (bool).
    """
    if img.mode != "RGB":
        img = img.convert("RGB")

    arr = np.asarray(img, dtype=np.float64)
    if arr.size == 0:
        return {"has_shadow": False}

    img_h, img_w = arr.shape[:2]
    lum = _luminance(arr)

    # Background luminance from the four image corners.
    corner = max(4, min(img_w, img_h) // 20)
    bg_lum = float(
        np.mean(
            [
                lum[:corner, :corner].mean(),
                lum[:corner, -corner:].mean(),
                lum[-corner:, :corner].mean(),
                lum[-corner:, -corner:].mean(),
            ]
        )
    )

    # Strip below the element.
    y_top = min(bbox.y2, img_h)
    y_bot = min(bbox.y2 + margin, img_h)
    x_lo = max(bbox.x1, 0)
    x_hi = min(bbox.x2, img_w)
    below = lum[y_top:y_bot, x_lo:x_hi]

    # Strip to the right of the element.
    x_lft = min(bbox.x2, img_w)
    x_rgt = min(bbox.x2 + margin, img_w)
    y_lo = max(bbox.y1, 0)
    y_hi = min(bbox.y2, img_h)
    right = lum[y_lo:y_hi, x_lft:x_rgt]

    def _darkness(strip: np.ndarray) -> float:
        if strip.size == 0:
            return 0.0
        return float(bg_lum - strip.mean())

    # Drop shadow = strip darker than the far background by a noticeable
    # margin. 4 luminance units is a conservative threshold for the MVP.
    threshold = 4.0
    has_shadow = _darkness(below) > threshold or _darkness(right) > threshold

    return {"has_shadow": bool(has_shadow)}
