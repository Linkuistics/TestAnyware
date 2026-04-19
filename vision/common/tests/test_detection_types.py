import json
import pytest
from testanyware_common.types import (
    BoundingBox,
    Detection,
    DetectionSet,
    GroundTruth,
    GroundTruthSource,
)


class TestDetection:
    def test_create(self):
        det = Detection(
            label="window",
            bbox=BoundingBox(x1=0, y1=0, x2=800, y2=600),
            confidence=0.95,
        )
        assert det.label == "window"
        assert det.confidence == 0.95

    def test_to_dict(self):
        det = Detection(
            label="button",
            bbox=BoundingBox(x1=10, y1=20, x2=110, y2=50),
            confidence=0.88,
            metadata={"text": "Save"},
        )
        d = det.to_dict()
        assert d["label"] == "button"
        assert d["bbox"] == [10, 20, 110, 50]
        assert d["confidence"] == 0.88
        assert d["metadata"]["text"] == "Save"

    def test_from_dict(self):
        d = {"label": "button", "bbox": [10, 20, 110, 50], "confidence": 0.88}
        det = Detection.from_dict(d)
        assert det.label == "button"
        assert det.bbox.x1 == 10

    def test_roundtrip_json(self):
        det = Detection(
            label="window",
            bbox=BoundingBox(x1=0, y1=0, x2=800, y2=600),
            confidence=0.95,
            metadata={"title": "TextEdit"},
        )
        json_str = json.dumps(det.to_dict())
        restored = Detection.from_dict(json.loads(json_str))
        assert restored.label == det.label
        assert restored.bbox == det.bbox
        assert restored.confidence == det.confidence
        assert restored.metadata == det.metadata


class TestDetectionSet:
    def test_create_and_filter(self):
        dets = DetectionSet(
            stage="window-detection",
            image_width=1920,
            image_height=1080,
            detections=[
                Detection("window", BoundingBox(0, 0, 800, 600), 0.95),
                Detection("window", BoundingBox(100, 100, 500, 400), 0.3),
            ],
        )
        high_conf = dets.filter_by_confidence(0.5)
        assert len(high_conf.detections) == 1
        assert high_conf.detections[0].confidence == 0.95

    def test_to_dict_and_back(self):
        dets = DetectionSet(
            stage="window-detection",
            image_width=1920,
            image_height=1080,
            detections=[
                Detection("window", BoundingBox(0, 0, 800, 600), 0.95),
            ],
        )
        d = dets.to_dict()
        restored = DetectionSet.from_dict(d)
        assert restored.stage == "window-detection"
        assert len(restored.detections) == 1
        assert restored.image_width == 1920


class TestGroundTruth:
    def test_create_with_sources(self):
        gt = GroundTruth(
            stage="window-detection",
            image_path="screenshot_001.png",
            image_width=1920,
            image_height=1080,
            detections=[
                Detection("window", BoundingBox(0, 0, 800, 600), 1.0,
                          metadata={"title": "TextEdit"}),
            ],
            sources=[GroundTruthSource.PROGRAMMATIC, GroundTruthSource.AGENT],
        )
        assert GroundTruthSource.PROGRAMMATIC in gt.sources
        assert gt.image_path == "screenshot_001.png"

    def test_roundtrip_json(self):
        gt = GroundTruth(
            stage="window-detection",
            image_path="img.png",
            image_width=1920,
            image_height=1080,
            detections=[
                Detection("window", BoundingBox(0, 0, 800, 600), 1.0),
            ],
            sources=[GroundTruthSource.PROGRAMMATIC],
        )
        json_str = json.dumps(gt.to_dict())
        restored = GroundTruth.from_dict(json.loads(json_str))
        assert restored.stage == gt.stage
        assert restored.sources == gt.sources
        assert len(restored.detections) == 1
