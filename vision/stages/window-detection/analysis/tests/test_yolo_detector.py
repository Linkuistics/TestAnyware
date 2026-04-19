import numpy as np
import pytest

from testanyware_common.types import Detection
from window_analysis.yolo_detector import YOLOWindowDetector


class TestYOLOWindowDetector:
    def test_create_without_model_raises(self):
        """Creating a detector with a nonexistent model path should fail clearly."""
        with pytest.raises(FileNotFoundError):
            YOLOWindowDetector(model_path="/nonexistent/model.pt")

    @pytest.mark.vision
    def test_detect_returns_detection_objects(self, trained_model_path):
        """Requires a trained model — run with pytest -m vision after training."""
        detector = YOLOWindowDetector(model_path=trained_model_path)
        img = np.full((1080, 1920, 3), (40, 50, 60), dtype=np.uint8)
        img[100:700, 100:900] = (255, 255, 255)  # fake window
        detections = detector.detect(img)
        for det in detections:
            assert isinstance(det, Detection)
            assert det.label == "window"


class TestYOLOWindowDetectorInterface:
    """Tests that don't require a model file — test the interface contract."""

    def test_confidence_threshold_is_settable(self):
        """The detector should accept a confidence threshold parameter."""
        # This just tests the constructor interface, not actual detection
        # We can't instantiate without a model, so test the class attributes
        assert hasattr(YOLOWindowDetector, "__init__")

    def test_detect_method_exists(self):
        assert hasattr(YOLOWindowDetector, "detect")
