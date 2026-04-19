import numpy as np
import pytest

from testanyware_common.types import BoundingBox, Detection
from window_analysis.heuristic import detect_windows_heuristic


class TestDetectWindowsHeuristic:
    def _make_desktop_with_window(self, x, y, w, h, screen_w=800, screen_h=600):
        """Create a synthetic image with a white window on a dark desktop."""
        img = np.full((screen_h, screen_w, 3), (40, 50, 60), dtype=np.uint8)
        # Title bar
        img[y:y + 28, x:x + w] = (200, 200, 200)
        # Window body
        img[y + 28:y + h, x:x + w] = (255, 255, 255)
        # Border
        img[y:y + h, x:x + 1] = (150, 150, 150)
        img[y:y + h, x + w - 1:x + w] = (150, 150, 150)
        img[y:y + 1, x:x + w] = (150, 150, 150)
        img[y + h - 1:y + h, x:x + w] = (150, 150, 150)
        return img

    def test_detects_single_window(self):
        img = self._make_desktop_with_window(100, 100, 400, 300)
        detections = detect_windows_heuristic(img)
        assert len(detections) >= 1
        # Check that at least one detection overlaps significantly with the actual window
        expected = BoundingBox(100, 100, 500, 400)
        best_iou = max(d.bbox.iou(expected) for d in detections)
        assert best_iou > 0.5, f"Best IoU was only {best_iou}"

    def test_detects_no_windows_on_empty_desktop(self):
        img = np.full((600, 800, 3), (40, 50, 60), dtype=np.uint8)
        # Add some noise so it's not trivially uniform
        noise = np.random.default_rng(42).integers(-5, 6, size=img.shape, dtype=np.int16)
        img = np.clip(img.astype(np.int16) + noise, 0, 255).astype(np.uint8)
        detections = detect_windows_heuristic(img)
        assert len(detections) == 0

    def test_returns_detection_objects(self):
        img = self._make_desktop_with_window(100, 100, 400, 300)
        detections = detect_windows_heuristic(img)
        for det in detections:
            assert isinstance(det, Detection)
            assert det.label == "window"
            assert 0.0 <= det.confidence <= 1.0
            assert det.bbox.width > 0
            assert det.bbox.height > 0

    def test_minimum_window_size_filter(self):
        """Very small rectangles should not be detected as windows."""
        img = np.full((600, 800, 3), (40, 50, 60), dtype=np.uint8)
        # Draw a tiny 20x20 white square — too small to be a window
        img[100:120, 100:120] = (255, 255, 255)
        detections = detect_windows_heuristic(img, min_width=50, min_height=50)
        assert len(detections) == 0
