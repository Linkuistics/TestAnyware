from __future__ import annotations

from pathlib import Path

import cv2
import numpy as np

from testanyware_common.types import BoundingBox


def load_image(path: str | Path) -> np.ndarray:
    path = Path(path)
    if not path.exists():
        raise FileNotFoundError(f"Image not found: {path}")
    img = cv2.imread(str(path), cv2.IMREAD_COLOR)
    if img is None:
        raise ValueError(f"Failed to decode image: {path}")
    return cv2.cvtColor(img, cv2.COLOR_BGR2RGB)


def save_image(image: np.ndarray, path: str | Path) -> None:
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    bgr = cv2.cvtColor(image, cv2.COLOR_RGB2BGR)
    cv2.imwrite(str(path), bgr)


def crop_image(image: np.ndarray, bbox: BoundingBox) -> np.ndarray:
    h, w = image.shape[:2]
    x1 = max(0, bbox.x1)
    y1 = max(0, bbox.y1)
    x2 = min(w, bbox.x2)
    y2 = min(h, bbox.y2)
    return image[y1:y2, x1:x2].copy()
