"""Dominant color extraction via MiniBatchKMeans clustering.

Simplified port of Redraw tier3/color.py::extract_fill_color that operates
on an image crop only (no mask, no text exclusion) for the MVP smoke test.
"""

from __future__ import annotations

import numpy as np
from PIL import Image
from sklearn.cluster import MiniBatchKMeans


def extract_dominant_color(img: Image.Image, n_clusters: int = 4) -> tuple[int, int, int]:
    """Return the dominant (r, g, b) color of ``img`` via k-means clustering.

    Args:
        img: RGB (or convertible) PIL image crop.
        n_clusters: Number of color clusters to fit.

    Returns:
        ``(r, g, b)`` tuple of ints in ``[0, 255]``.
    """
    if img.mode != "RGB":
        img = img.convert("RGB")

    arr = np.asarray(img)
    if arr.size == 0:
        return (0, 0, 0)

    pixels = arr.reshape(-1, 3)
    if len(pixels) == 0:
        return (0, 0, 0)

    k = max(1, min(n_clusters, len(pixels)))

    kmeans = MiniBatchKMeans(n_clusters=k, n_init=1, random_state=0)
    kmeans.fit(pixels)

    labels, counts = np.unique(kmeans.labels_, return_counts=True)
    dominant_idx = labels[int(np.argmax(counts))]
    center = kmeans.cluster_centers_[dominant_idx]
    r, g, b = (int(round(v)) for v in center)
    return (r, g, b)
