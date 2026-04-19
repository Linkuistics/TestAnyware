from __future__ import annotations

import enum
from dataclasses import dataclass


@dataclass(frozen=True)
class BoundingBox:
    """Axis-aligned bounding box in pixel coordinates (top-left origin)."""

    x1: int
    y1: int
    x2: int
    y2: int

    @classmethod
    def from_xywh(cls, x: int, y: int, w: int, h: int) -> BoundingBox:
        return cls(x1=x, y1=y, x2=x + w, y2=y + h)

    @classmethod
    def from_center(cls, cx: float, cy: float, w: float, h: float) -> BoundingBox:
        return cls(
            x1=int(cx - w / 2),
            y1=int(cy - h / 2),
            x2=int(cx + w / 2),
            y2=int(cy + h / 2),
        )

    @property
    def width(self) -> int:
        return self.x2 - self.x1

    @property
    def height(self) -> int:
        return self.y2 - self.y1

    @property
    def center(self) -> tuple[float, float]:
        return (self.x1 + self.width / 2, self.y1 + self.height / 2)

    @property
    def area(self) -> int:
        return self.width * self.height

    def iou(self, other: BoundingBox) -> float:
        ix1 = max(self.x1, other.x1)
        iy1 = max(self.y1, other.y1)
        ix2 = min(self.x2, other.x2)
        iy2 = min(self.y2, other.y2)
        if ix2 <= ix1 or iy2 <= iy1:
            return 0.0
        intersection = (ix2 - ix1) * (iy2 - iy1)
        union = self.area + other.area - intersection
        return intersection / union if union > 0 else 0.0

    def contains(self, other: BoundingBox) -> bool:
        return (
            self.x1 <= other.x1
            and self.y1 <= other.y1
            and self.x2 >= other.x2
            and self.y2 >= other.y2
        )


class GroundTruthSource(enum.Enum):
    """How ground truth was obtained."""
    PROGRAMMATIC = "programmatic"  # Known from what the test harness did
    AGENT = "agent"                # Reported by guest agent (accessibility/window APIs)
    MANUAL = "manual"              # Human-annotated


@dataclass
class Detection:
    """A single detected object with label, bounding box, confidence, and optional metadata."""

    label: str
    bbox: BoundingBox
    confidence: float
    metadata: dict | None = None

    def to_dict(self) -> dict:
        d = {
            "label": self.label,
            "bbox": [self.bbox.x1, self.bbox.y1, self.bbox.x2, self.bbox.y2],
            "confidence": self.confidence,
        }
        if self.metadata:
            d["metadata"] = self.metadata
        return d

    @classmethod
    def from_dict(cls, d: dict) -> Detection:
        bbox = BoundingBox(x1=d["bbox"][0], y1=d["bbox"][1], x2=d["bbox"][2], y2=d["bbox"][3])
        return cls(
            label=d["label"],
            bbox=bbox,
            confidence=d["confidence"],
            metadata=d.get("metadata"),
        )


@dataclass
class DetectionSet:
    """A set of detections from a single pipeline stage on a single image."""

    stage: str
    image_width: int
    image_height: int
    detections: list[Detection]

    def filter_by_confidence(self, threshold: float) -> DetectionSet:
        return DetectionSet(
            stage=self.stage,
            image_width=self.image_width,
            image_height=self.image_height,
            detections=[d for d in self.detections if d.confidence >= threshold],
        )

    def to_dict(self) -> dict:
        return {
            "stage": self.stage,
            "image_width": self.image_width,
            "image_height": self.image_height,
            "detections": [d.to_dict() for d in self.detections],
        }

    @classmethod
    def from_dict(cls, d: dict) -> DetectionSet:
        return cls(
            stage=d["stage"],
            image_width=d["image_width"],
            image_height=d["image_height"],
            detections=[Detection.from_dict(det) for det in d["detections"]],
        )


@dataclass
class GroundTruth:
    """Ground truth for a single image — used for training and evaluation."""

    stage: str
    image_path: str
    image_width: int
    image_height: int
    detections: list[Detection]
    sources: list[GroundTruthSource]

    def to_dict(self) -> dict:
        return {
            "stage": self.stage,
            "image_path": self.image_path,
            "image_width": self.image_width,
            "image_height": self.image_height,
            "detections": [d.to_dict() for d in self.detections],
            "sources": [s.value for s in self.sources],
        }

    @classmethod
    def from_dict(cls, d: dict) -> GroundTruth:
        return cls(
            stage=d["stage"],
            image_path=d["image_path"],
            image_width=d["image_width"],
            image_height=d["image_height"],
            detections=[Detection.from_dict(det) for det in d["detections"]],
            sources=[GroundTruthSource(s) for s in d["sources"]],
        )
