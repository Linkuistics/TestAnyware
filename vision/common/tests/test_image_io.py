import tempfile
from pathlib import Path

import numpy as np
import pytest

from testanyware_common.image_io import load_image, save_image, crop_image
from testanyware_common.types import BoundingBox


class TestImageIO:
    def _make_test_image(self, width=200, height=100) -> np.ndarray:
        """Create a simple test image with a known pattern."""
        img = np.zeros((height, width, 3), dtype=np.uint8)
        img[10:50, 10:90] = [255, 0, 0]  # red rectangle
        img[60:80, 110:190] = [0, 255, 0]  # green rectangle
        return img

    def test_save_and_load_png(self, tmp_path):
        img = self._make_test_image()
        path = tmp_path / "test.png"
        save_image(img, path)
        assert path.exists()
        loaded = load_image(path)
        assert loaded.shape == img.shape
        np.testing.assert_array_equal(loaded, img)

    def test_load_nonexistent_raises(self, tmp_path):
        with pytest.raises(FileNotFoundError):
            load_image(tmp_path / "nope.png")

    def test_crop_image(self):
        img = self._make_test_image(200, 100)
        bbox = BoundingBox(x1=10, y1=10, x2=90, y2=50)
        cropped = crop_image(img, bbox)
        assert cropped.shape == (40, 80, 3)
        # The entire crop should be red
        assert np.all(cropped[:, :, 0] == 255)
        assert np.all(cropped[:, :, 1] == 0)
        assert np.all(cropped[:, :, 2] == 0)

    def test_crop_clamps_to_bounds(self):
        img = self._make_test_image(200, 100)
        bbox = BoundingBox(x1=-10, y1=-10, x2=300, y2=200)
        cropped = crop_image(img, bbox)
        assert cropped.shape == (100, 200, 3)  # clamped to image size
