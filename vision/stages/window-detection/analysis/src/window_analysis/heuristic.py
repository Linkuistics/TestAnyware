from __future__ import annotations

import cv2
import numpy as np

from testanyware_common.nms import non_maximum_suppression
from testanyware_common.types import BoundingBox, Detection


def detect_windows_heuristic(
    image: np.ndarray,
    min_width: int = 100,
    min_height: int = 80,
    canny_low: int = 30,
    canny_high: int = 100,
) -> list[Detection]:
    gray = cv2.cvtColor(image, cv2.COLOR_RGB2GRAY)
    blurred = cv2.GaussianBlur(gray, (5, 5), 0)
    edges = cv2.Canny(blurred, canny_low, canny_high)

    # Dilate edges to close small gaps
    kernel = cv2.getStructuringElement(cv2.MORPH_RECT, (3, 3))
    edges = cv2.dilate(edges, kernel, iterations=2)

    contours, _ = cv2.findContours(edges, cv2.RETR_EXTERNAL, cv2.CHAIN_APPROX_SIMPLE)

    detections: list[Detection] = []
    img_h, img_w = image.shape[:2]
    img_area = img_h * img_w

    for contour in contours:
        x, y, w, h = cv2.boundingRect(contour)

        if w < min_width or h < min_height:
            continue

        # Filter by aspect ratio — windows are typically wider than tall or roughly square
        aspect = w / h
        if aspect < 0.3 or aspect > 5.0:
            continue

        # Filter out near-full-screen detections (likely the desktop itself)
        area_ratio = (w * h) / img_area
        if area_ratio > 0.95:
            continue

        # Confidence heuristic based on rectangularity and size
        rect_area = w * h
        contour_area = cv2.contourArea(contour)
        rectangularity = contour_area / rect_area if rect_area > 0 else 0
        size_score = min(1.0, (w * h) / (400 * 300))  # bigger = more likely a window
        confidence = 0.3 * rectangularity + 0.3 * size_score + 0.4 * min(1.0, aspect / 2.0)
        confidence = min(1.0, max(0.0, confidence))

        detections.append(
            Detection(
                label="window",
                bbox=BoundingBox(x1=x, y1=y, x2=x + w, y2=y + h),
                confidence=confidence,
            )
        )

    return non_maximum_suppression(detections, iou_threshold=0.3)
