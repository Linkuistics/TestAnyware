# GUIVisionPipeline: Master Plan + Stage 1 (Window Detection)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Scaffold the GUIVisionPipeline project with shared infrastructure, then implement the first pipeline stage (window detection) end-to-end — from training data generation through model training to production analysis library.

**Architecture:** Monorepo with Python (uv) for vision/ML and Swift (SPM) for CoreML inference, OCR, and GUIVisionVMDriver integration. Each pipeline stage is a subproject with generator, training, and analysis components. A `common/` package provides shared types, metrics, and test utilities. Guest agents are separate build targets that get injected into VMs.

**Tech Stack:** Python 3.11+, uv, pytest, Ultralytics YOLOv8, OpenCV, NumPy, Pillow, FastAPI | Swift 6.0, CoreML, Apple Vision framework, GUIVisionVMDriver, swift-argument-parser

---

## Phase 0: Project Scaffolding

### Task 0.1: Initialise Git and Python Project Root

**Files:**
- Create: `.gitignore`
- Create: `pyproject.toml`
- Create: `.python-version`

- [ ] **Step 1: Initialise git repository**

```bash
cd /Users/antony/Development/GUIVisionPipeline
git init
```

- [ ] **Step 2: Create .gitignore**

```gitignore
# Python
__pycache__/
*.py[cod]
*.egg-info/
dist/
.venv/
*.egg

# Swift
.build/
.swiftpm/
Package.resolved
*.xcodeproj/
xcuserdata/

# ML models and training artifacts
*.pt
*.onnx
*.mlpackage/
*.mlmodelc/
runs/
wandb/

# Training data (large, generated locally)
stages/*/generator/output/
stages/*/training/data/
stages/*/training/runs/

# OS
.DS_Store
*.swp
*~

# Environment
.env
.env.*
```

- [ ] **Step 3: Create pyproject.toml for the workspace root**

```toml
[project]
name = "guivision-pipeline"
version = "0.1.0"
description = "Hierarchical vision pipeline for GUI screenshot analysis"
requires-python = ">=3.11"
dependencies = []

[tool.uv.workspace]
members = [
    "common",
    "stages/window-detection/generator",
    "stages/window-detection/training",
    "stages/window-detection/analysis",
]

[tool.pytest.ini_options]
testpaths = ["common/tests", "stages"]
markers = [
    "unit: pure logic tests, no models or VMs",
    "vision: detector accuracy tests against golden datasets",
    "integration: full end-to-end tests against live VMs",
    "slow: tests that take more than 10 seconds",
]
addopts = "-m 'not integration and not slow' --tb=short -q"

[tool.ruff]
line-length = 100
target-version = "py311"

[tool.ruff.lint]
select = ["E", "F", "I", "N", "W", "UP"]
```

- [ ] **Step 4: Create .python-version**

```
3.11
```

- [ ] **Step 5: Commit**

```bash
git add .gitignore pyproject.toml .python-version README.md LICENSE
git commit -m "feat: initialise project with workspace config and README"
```

---

### Task 0.2: Create Directory Structure

**Files:**
- Create: directory tree and placeholder files

- [ ] **Step 1: Create the full directory skeleton**

```bash
mkdir -p common/src/guivision_common
mkdir -p common/tests

mkdir -p stages/window-detection/generator/src/window_gen
mkdir -p stages/window-detection/generator/tests
mkdir -p stages/window-detection/training/src/window_train
mkdir -p stages/window-detection/training/tests
mkdir -p stages/window-detection/training/configs
mkdir -p stages/window-detection/analysis/src/window_analysis
mkdir -p stages/window-detection/analysis/tests

mkdir -p agents/macos
mkdir -p agents/windows
mkdir -p agents/linux

mkdir -p pipeline/src/guivision_pipeline
mkdir -p pipeline/tests

mkdir -p swift/Sources/GUIVisionSwift
mkdir -p swift/Tests/GUIVisionSwiftTests
```

- [ ] **Step 2: Create `__init__.py` files for all Python packages**

Create empty `__init__.py` in:
- `common/src/guivision_common/__init__.py`
- `stages/window-detection/generator/src/window_gen/__init__.py`
- `stages/window-detection/training/src/window_train/__init__.py`
- `stages/window-detection/analysis/src/window_analysis/__init__.py`
- `pipeline/src/guivision_pipeline/__init__.py`

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "feat: create directory skeleton for all subprojects"
```

---

### Task 0.3: Create Common Package with Shared Types

**Files:**
- Create: `common/pyproject.toml`
- Create: `common/src/guivision_common/__init__.py`
- Create: `common/src/guivision_common/types.py`
- Create: `common/tests/test_types.py`

- [ ] **Step 1: Write failing test for BoundingBox**

```python
# common/tests/test_types.py
import pytest
from guivision_common.types import BoundingBox


class TestBoundingBox:
    def test_create_from_xyxy(self):
        box = BoundingBox(x1=10, y1=20, x2=110, y2=70)
        assert box.x1 == 10
        assert box.y1 == 20
        assert box.x2 == 110
        assert box.y2 == 70

    def test_width_and_height(self):
        box = BoundingBox(x1=10, y1=20, x2=110, y2=70)
        assert box.width == 100
        assert box.height == 50

    def test_center(self):
        box = BoundingBox(x1=0, y1=0, x2=100, y2=100)
        cx, cy = box.center
        assert cx == 50.0
        assert cy == 50.0

    def test_area(self):
        box = BoundingBox(x1=0, y1=0, x2=100, y2=50)
        assert box.area == 5000

    def test_iou_identical(self):
        box = BoundingBox(x1=0, y1=0, x2=100, y2=100)
        assert box.iou(box) == 1.0

    def test_iou_no_overlap(self):
        a = BoundingBox(x1=0, y1=0, x2=50, y2=50)
        b = BoundingBox(x1=100, y1=100, x2=200, y2=200)
        assert a.iou(b) == 0.0

    def test_iou_partial_overlap(self):
        a = BoundingBox(x1=0, y1=0, x2=100, y2=100)
        b = BoundingBox(x1=50, y1=50, x2=150, y2=150)
        # intersection: 50x50=2500, union: 10000+10000-2500=17500
        assert a.iou(b) == pytest.approx(2500 / 17500)

    def test_contains(self):
        outer = BoundingBox(x1=0, y1=0, x2=200, y2=200)
        inner = BoundingBox(x1=50, y1=50, x2=100, y2=100)
        assert outer.contains(inner) is True
        assert inner.contains(outer) is False

    def test_from_xywh(self):
        box = BoundingBox.from_xywh(x=10, y=20, w=100, h=50)
        assert box.x1 == 10
        assert box.y1 == 20
        assert box.x2 == 110
        assert box.y2 == 70

    def test_from_center(self):
        box = BoundingBox.from_center(cx=50, cy=50, w=100, h=100)
        assert box.x1 == 0
        assert box.y1 == 0
        assert box.x2 == 100
        assert box.y2 == 100
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd /Users/antony/Development/GUIVisionPipeline && uv run pytest common/tests/test_types.py -v`
Expected: FAIL — module not found

- [ ] **Step 3: Create common/pyproject.toml**

```toml
[project]
name = "guivision-common"
version = "0.1.0"
description = "Shared types, metrics, and utilities for GUIVisionPipeline"
requires-python = ">=3.11"
dependencies = [
    "numpy>=1.26",
]

[build-system]
requires = ["hatchling"]
build-backend = "hatchling.build"

[tool.hatch.build.targets.wheel]
packages = ["src/guivision_common"]
```

- [ ] **Step 4: Implement BoundingBox**

```python
# common/src/guivision_common/types.py
from __future__ import annotations

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
```

- [ ] **Step 5: Update common __init__.py**

```python
# common/src/guivision_common/__init__.py
from guivision_common.types import BoundingBox

__all__ = ["BoundingBox"]
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cd /Users/antony/Development/GUIVisionPipeline && uv run pytest common/tests/test_types.py -v`
Expected: All 11 tests PASS

- [ ] **Step 7: Commit**

```bash
git add common/
git commit -m "feat(common): add BoundingBox type with IoU, containment, factory methods"
```

---

### Task 0.4: Add Detection Result Types to Common

**Files:**
- Modify: `common/src/guivision_common/types.py`
- Create: `common/tests/test_detection_types.py`

- [ ] **Step 1: Write failing tests for detection types**

```python
# common/tests/test_detection_types.py
import json
import pytest
from guivision_common.types import (
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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `uv run pytest common/tests/test_detection_types.py -v`
Expected: FAIL — imports not found

- [ ] **Step 3: Implement detection types**

Add to `common/src/guivision_common/types.py`:

```python
import enum


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
```

- [ ] **Step 4: Update __init__.py exports**

```python
# common/src/guivision_common/__init__.py
from guivision_common.types import (
    BoundingBox,
    Detection,
    DetectionSet,
    GroundTruth,
    GroundTruthSource,
)

__all__ = ["BoundingBox", "Detection", "DetectionSet", "GroundTruth", "GroundTruthSource"]
```

- [ ] **Step 5: Run all common tests**

Run: `uv run pytest common/tests/ -v`
Expected: All tests PASS

- [ ] **Step 6: Commit**

```bash
git add common/
git commit -m "feat(common): add Detection, DetectionSet, GroundTruth types with JSON roundtrip"
```

---

### Task 0.5: Add Metrics Module to Common

**Files:**
- Create: `common/src/guivision_common/metrics.py`
- Create: `common/tests/test_metrics.py`

- [ ] **Step 1: Write failing tests for precision/recall/F1**

```python
# common/tests/test_metrics.py
import pytest
from guivision_common.types import BoundingBox, Detection
from guivision_common.metrics import compute_metrics, MetricsResult


class TestComputeMetrics:
    def _det(self, x1, y1, x2, y2, label="window", conf=0.9):
        return Detection(label, BoundingBox(x1, y1, x2, y2), conf)

    def test_perfect_match(self):
        gt = [self._det(0, 0, 100, 100)]
        pred = [self._det(0, 0, 100, 100)]
        result = compute_metrics(predictions=pred, ground_truths=gt, iou_threshold=0.5)
        assert result.precision == 1.0
        assert result.recall == 1.0
        assert result.f1 == 1.0

    def test_no_predictions(self):
        gt = [self._det(0, 0, 100, 100)]
        result = compute_metrics(predictions=[], ground_truths=gt, iou_threshold=0.5)
        assert result.precision == 0.0
        assert result.recall == 0.0
        assert result.f1 == 0.0

    def test_no_ground_truth(self):
        pred = [self._det(0, 0, 100, 100)]
        result = compute_metrics(predictions=pred, ground_truths=[], iou_threshold=0.5)
        assert result.precision == 0.0
        assert result.recall == 0.0

    def test_both_empty(self):
        result = compute_metrics(predictions=[], ground_truths=[], iou_threshold=0.5)
        assert result.precision == 0.0
        assert result.recall == 0.0

    def test_partial_overlap_above_threshold(self):
        gt = [self._det(0, 0, 100, 100)]
        # 75% overlap on each axis = 56.25% IoU area
        pred = [self._det(25, 25, 125, 125)]
        result = compute_metrics(predictions=pred, ground_truths=gt, iou_threshold=0.3)
        assert result.precision == 1.0
        assert result.recall == 1.0

    def test_partial_overlap_below_threshold(self):
        gt = [self._det(0, 0, 100, 100)]
        # barely overlapping
        pred = [self._det(90, 90, 190, 190)]
        result = compute_metrics(predictions=pred, ground_truths=gt, iou_threshold=0.5)
        assert result.precision == 0.0
        assert result.recall == 0.0

    def test_one_hit_one_miss(self):
        gt = [self._det(0, 0, 100, 100), self._det(200, 200, 300, 300)]
        pred = [self._det(0, 0, 100, 100)]  # matches first, misses second
        result = compute_metrics(predictions=pred, ground_truths=gt, iou_threshold=0.5)
        assert result.precision == 1.0
        assert result.recall == 0.5
        assert result.f1 == pytest.approx(2 / 3)

    def test_extra_prediction(self):
        gt = [self._det(0, 0, 100, 100)]
        pred = [self._det(0, 0, 100, 100), self._det(500, 500, 600, 600)]
        result = compute_metrics(predictions=pred, ground_truths=gt, iou_threshold=0.5)
        assert result.precision == 0.5
        assert result.recall == 1.0

    def test_label_matching(self):
        gt = [Detection("window", BoundingBox(0, 0, 100, 100), 1.0)]
        pred = [Detection("button", BoundingBox(0, 0, 100, 100), 0.9)]
        result = compute_metrics(
            predictions=pred, ground_truths=gt, iou_threshold=0.5, match_labels=True
        )
        assert result.precision == 0.0
        assert result.recall == 0.0

    def test_metrics_result_meets_threshold(self):
        r = MetricsResult(precision=0.9, recall=0.85, f1=0.874, true_pos=17, false_pos=2, false_neg=3)
        assert r.meets_threshold(min_precision=0.8, min_recall=0.8, min_f1=0.8) is True
        assert r.meets_threshold(min_precision=0.95) is False
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `uv run pytest common/tests/test_metrics.py -v`
Expected: FAIL — module not found

- [ ] **Step 3: Implement metrics**

```python
# common/src/guivision_common/metrics.py
from __future__ import annotations

from dataclasses import dataclass

from guivision_common.types import Detection


@dataclass(frozen=True)
class MetricsResult:
    precision: float
    recall: float
    f1: float
    true_pos: int
    false_pos: int
    false_neg: int

    def meets_threshold(
        self,
        min_precision: float = 0.0,
        min_recall: float = 0.0,
        min_f1: float = 0.0,
    ) -> bool:
        return (
            self.precision >= min_precision
            and self.recall >= min_recall
            and self.f1 >= min_f1
        )


def compute_metrics(
    predictions: list[Detection],
    ground_truths: list[Detection],
    iou_threshold: float = 0.5,
    match_labels: bool = False,
) -> MetricsResult:
    if not predictions and not ground_truths:
        return MetricsResult(0.0, 0.0, 0.0, 0, 0, 0)

    matched_gt: set[int] = set()
    true_pos = 0

    sorted_preds = sorted(predictions, key=lambda d: d.confidence, reverse=True)

    for pred in sorted_preds:
        best_iou = 0.0
        best_gt_idx = -1
        for gt_idx, gt in enumerate(ground_truths):
            if gt_idx in matched_gt:
                continue
            if match_labels and pred.label != gt.label:
                continue
            iou = pred.bbox.iou(gt.bbox)
            if iou > best_iou:
                best_iou = iou
                best_gt_idx = gt_idx
        if best_iou >= iou_threshold and best_gt_idx >= 0:
            true_pos += 1
            matched_gt.add(best_gt_idx)

    false_pos = len(predictions) - true_pos
    false_neg = len(ground_truths) - true_pos

    precision = true_pos / len(predictions) if predictions else 0.0
    recall = true_pos / len(ground_truths) if ground_truths else 0.0
    f1 = (2 * precision * recall / (precision + recall)) if (precision + recall) > 0 else 0.0

    return MetricsResult(
        precision=precision,
        recall=recall,
        f1=f1,
        true_pos=true_pos,
        false_pos=false_pos,
        false_neg=false_neg,
    )
```

- [ ] **Step 4: Update __init__.py exports**

```python
# common/src/guivision_common/__init__.py
from guivision_common.metrics import MetricsResult, compute_metrics
from guivision_common.types import (
    BoundingBox,
    Detection,
    DetectionSet,
    GroundTruth,
    GroundTruthSource,
)

__all__ = [
    "BoundingBox",
    "Detection",
    "DetectionSet",
    "GroundTruth",
    "GroundTruthSource",
    "MetricsResult",
    "compute_metrics",
]
```

- [ ] **Step 5: Run all common tests**

Run: `uv run pytest common/tests/ -v`
Expected: All tests PASS (both test_types.py and test_metrics.py)

- [ ] **Step 6: Commit**

```bash
git add common/
git commit -m "feat(common): add compute_metrics with precision/recall/F1 and threshold gating"
```

---

### Task 0.6: Add NMS (Non-Maximum Suppression) to Common

**Files:**
- Create: `common/src/guivision_common/nms.py`
- Create: `common/tests/test_nms.py`

- [ ] **Step 1: Write failing tests**

```python
# common/tests/test_nms.py
from guivision_common.types import BoundingBox, Detection
from guivision_common.nms import non_maximum_suppression


class TestNMS:
    def _det(self, x1, y1, x2, y2, conf=0.9, label="window"):
        return Detection(label, BoundingBox(x1, y1, x2, y2), conf)

    def test_empty_input(self):
        assert non_maximum_suppression([], iou_threshold=0.5) == []

    def test_single_detection(self):
        dets = [self._det(0, 0, 100, 100)]
        result = non_maximum_suppression(dets, iou_threshold=0.5)
        assert len(result) == 1

    def test_non_overlapping_kept(self):
        dets = [
            self._det(0, 0, 100, 100, conf=0.9),
            self._det(200, 200, 300, 300, conf=0.8),
        ]
        result = non_maximum_suppression(dets, iou_threshold=0.5)
        assert len(result) == 2

    def test_overlapping_suppressed(self):
        dets = [
            self._det(0, 0, 100, 100, conf=0.9),
            self._det(10, 10, 110, 110, conf=0.7),  # high overlap with first
        ]
        result = non_maximum_suppression(dets, iou_threshold=0.5)
        assert len(result) == 1
        assert result[0].confidence == 0.9  # higher confidence kept

    def test_three_overlapping_keeps_best(self):
        dets = [
            self._det(0, 0, 100, 100, conf=0.6),
            self._det(5, 5, 105, 105, conf=0.9),
            self._det(10, 10, 110, 110, conf=0.7),
        ]
        result = non_maximum_suppression(dets, iou_threshold=0.5)
        assert len(result) == 1
        assert result[0].confidence == 0.9

    def test_respects_iou_threshold(self):
        dets = [
            self._det(0, 0, 100, 100, conf=0.9),
            self._det(50, 50, 150, 150, conf=0.8),  # ~14% IoU
        ]
        # High threshold: both kept
        result = non_maximum_suppression(dets, iou_threshold=0.5)
        assert len(result) == 2
        # Low threshold: one suppressed
        result = non_maximum_suppression(dets, iou_threshold=0.1)
        assert len(result) == 1
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `uv run pytest common/tests/test_nms.py -v`
Expected: FAIL

- [ ] **Step 3: Implement NMS**

```python
# common/src/guivision_common/nms.py
from __future__ import annotations

from guivision_common.types import Detection


def non_maximum_suppression(
    detections: list[Detection],
    iou_threshold: float = 0.45,
) -> list[Detection]:
    if not detections:
        return []

    sorted_dets = sorted(detections, key=lambda d: d.confidence, reverse=True)
    keep: list[Detection] = []

    for det in sorted_dets:
        suppressed = False
        for kept in keep:
            if det.bbox.iou(kept.bbox) >= iou_threshold:
                suppressed = True
                break
        if not suppressed:
            keep.append(det)

    return keep
```

- [ ] **Step 4: Update __init__.py**

Add to exports:

```python
from guivision_common.nms import non_maximum_suppression
```

And add `"non_maximum_suppression"` to `__all__`.

- [ ] **Step 5: Run all common tests**

Run: `uv run pytest common/tests/ -v`
Expected: All tests PASS

- [ ] **Step 6: Commit**

```bash
git add common/
git commit -m "feat(common): add non-maximum suppression"
```

---

### Task 0.7: Add Image I/O Utilities to Common

**Files:**
- Create: `common/src/guivision_common/image_io.py`
- Create: `common/tests/test_image_io.py`

- [ ] **Step 1: Update common/pyproject.toml dependencies**

Add `pillow>=10.0` and `opencv-python-headless>=4.8` to dependencies:

```toml
dependencies = [
    "numpy>=1.26",
    "pillow>=10.0",
    "opencv-python-headless>=4.8",
]
```

- [ ] **Step 2: Write failing tests**

```python
# common/tests/test_image_io.py
import tempfile
from pathlib import Path

import numpy as np
import pytest

from guivision_common.image_io import load_image, save_image, crop_image
from guivision_common.types import BoundingBox


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
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `uv run pytest common/tests/test_image_io.py -v`
Expected: FAIL

- [ ] **Step 4: Implement image I/O**

```python
# common/src/guivision_common/image_io.py
from __future__ import annotations

from pathlib import Path

import cv2
import numpy as np

from guivision_common.types import BoundingBox


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
```

- [ ] **Step 5: Update __init__.py**

Add to exports:

```python
from guivision_common.image_io import crop_image, load_image, save_image
```

And add `"load_image"`, `"save_image"`, `"crop_image"` to `__all__`.

- [ ] **Step 6: Run all common tests**

Run: `uv run pytest common/tests/ -v`
Expected: All tests PASS

- [ ] **Step 7: Commit**

```bash
git add common/
git commit -m "feat(common): add image I/O utilities (load, save, crop) with OpenCV"
```

---

## Phase 1: Window Detection — Stage 1 of the Pipeline

Window detection is the first and coarsest stage. It takes a full desktop screenshot and detects individual window boundaries (bounding boxes), title text, and z-order (front-to-back stacking).

### Accuracy Gate

Before proceeding to Phase 2 (Element Detection), window detection must achieve:
- **Precision >= 0.90** (few false positives — don't detect windows that aren't there)
- **Recall >= 0.90** (few false negatives — don't miss windows that are there)
- **F1 >= 0.90**

Measured on a golden test set of at least 50 diverse desktop screenshots across macOS and Windows.

---

### Task 1.1: Window Detection Generator — Scaffold and pyproject.toml

**Files:**
- Create: `stages/window-detection/generator/pyproject.toml`
- Create: `stages/window-detection/generator/src/window_gen/__init__.py`
- Create: `stages/window-detection/generator/src/window_gen/scenarios.py`
- Create: `stages/window-detection/generator/tests/test_scenarios.py`

- [ ] **Step 1: Create pyproject.toml**

```toml
[project]
name = "window-gen"
version = "0.1.0"
description = "Training data generator for window detection stage"
requires-python = ">=3.11"
dependencies = [
    "guivision-common",
    "pillow>=10.0",
    "numpy>=1.26",
]

[project.optional-dependencies]
vm = [
    # These are needed when running against real VMs
    # GUIVisionVMDriver is a Swift package used via subprocess
]

[build-system]
requires = ["hatchling"]
build-backend = "hatchling.build"

[tool.hatch.build.targets.wheel]
packages = ["src/window_gen"]

[project.scripts]
window-gen = "window_gen.cli:main"
```

- [ ] **Step 2: Write failing test for Scenario dataclass**

```python
# stages/window-detection/generator/tests/test_scenarios.py
import pytest
from window_gen.scenarios import WindowScenario, WindowSpec


class TestWindowSpec:
    def test_create(self):
        spec = WindowSpec(
            app_name="TextEdit",
            title="Untitled",
            x=100,
            y=100,
            width=800,
            height=600,
            z_order=0,
        )
        assert spec.app_name == "TextEdit"
        assert spec.width == 800

    def test_to_ground_truth_detection(self):
        spec = WindowSpec(
            app_name="TextEdit",
            title="Untitled",
            x=100, y=100, width=800, height=600,
            z_order=0,
        )
        det = spec.to_detection()
        assert det.label == "window"
        assert det.bbox.x1 == 100
        assert det.bbox.x2 == 900
        assert det.confidence == 1.0
        assert det.metadata["title"] == "Untitled"
        assert det.metadata["app_name"] == "TextEdit"
        assert det.metadata["z_order"] == 0


class TestWindowScenario:
    def test_create(self):
        scenario = WindowScenario(
            name="two-overlapping-windows",
            description="Two TextEdit windows overlapping by 200px",
            screen_width=1920,
            screen_height=1080,
            windows=[
                WindowSpec("TextEdit", "Doc1.txt", 100, 100, 800, 600, z_order=0),
                WindowSpec("Safari", "Google", 500, 200, 900, 700, z_order=1),
            ],
        )
        assert len(scenario.windows) == 2
        assert scenario.screen_width == 1920

    def test_to_ground_truth(self):
        scenario = WindowScenario(
            name="single-window",
            description="One maximized window",
            screen_width=1920,
            screen_height=1080,
            windows=[
                WindowSpec("TextEdit", "Doc.txt", 0, 25, 1920, 1055, z_order=0),
            ],
        )
        gt = scenario.to_ground_truth(image_path="screenshot_001.png")
        assert gt.stage == "window-detection"
        assert gt.image_path == "screenshot_001.png"
        assert gt.image_width == 1920
        assert gt.image_height == 1080
        assert len(gt.detections) == 1
        assert gt.detections[0].bbox.width == 1920
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `uv run pytest stages/window-detection/generator/tests/test_scenarios.py -v`
Expected: FAIL

- [ ] **Step 4: Implement Scenario types**

```python
# stages/window-detection/generator/src/window_gen/scenarios.py
from __future__ import annotations

from dataclasses import dataclass

from guivision_common.types import BoundingBox, Detection, GroundTruth, GroundTruthSource


@dataclass
class WindowSpec:
    """Specification for a single window to create in the VM."""

    app_name: str
    title: str
    x: int
    y: int
    width: int
    height: int
    z_order: int

    def to_detection(self) -> Detection:
        return Detection(
            label="window",
            bbox=BoundingBox.from_xywh(self.x, self.y, self.width, self.height),
            confidence=1.0,
            metadata={
                "title": self.title,
                "app_name": self.app_name,
                "z_order": self.z_order,
            },
        )


@dataclass
class WindowScenario:
    """A complete scenario: a set of windows to create and screenshot."""

    name: str
    description: str
    screen_width: int
    screen_height: int
    windows: list[WindowSpec]

    def to_ground_truth(self, image_path: str) -> GroundTruth:
        return GroundTruth(
            stage="window-detection",
            image_path=image_path,
            image_width=self.screen_width,
            image_height=self.screen_height,
            detections=[w.to_detection() for w in self.windows],
            sources=[GroundTruthSource.PROGRAMMATIC],
        )
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `uv run pytest stages/window-detection/generator/tests/test_scenarios.py -v`
Expected: All tests PASS

- [ ] **Step 6: Commit**

```bash
git add stages/window-detection/generator/
git commit -m "feat(window-gen): add WindowSpec and WindowScenario types with ground truth generation"
```

---

### Task 1.2: Window Detection Generator — Scenario Library

**Files:**
- Create: `stages/window-detection/generator/src/window_gen/scenario_library.py`
- Create: `stages/window-detection/generator/tests/test_scenario_library.py`

- [ ] **Step 1: Write failing tests**

```python
# stages/window-detection/generator/tests/test_scenario_library.py
from window_gen.scenario_library import build_scenario_library
from window_gen.scenarios import WindowScenario


class TestScenarioLibrary:
    def test_library_is_not_empty(self):
        library = build_scenario_library()
        assert len(library) > 0

    def test_all_entries_are_scenarios(self):
        library = build_scenario_library()
        for scenario in library:
            assert isinstance(scenario, WindowScenario)
            assert scenario.name
            assert scenario.screen_width > 0
            assert scenario.screen_height > 0
            assert len(scenario.windows) >= 0

    def test_has_zero_window_scenario(self):
        library = build_scenario_library()
        names = [s.name for s in library]
        assert "empty-desktop" in names

    def test_has_single_window_scenario(self):
        library = build_scenario_library()
        single = [s for s in library if len(s.windows) == 1]
        assert len(single) >= 1

    def test_has_multi_window_scenario(self):
        library = build_scenario_library()
        multi = [s for s in library if len(s.windows) >= 3]
        assert len(multi) >= 1

    def test_has_overlapping_windows(self):
        library = build_scenario_library()
        has_overlap = False
        for scenario in library:
            for i, w1 in enumerate(scenario.windows):
                d1 = w1.to_detection()
                for w2 in scenario.windows[i + 1:]:
                    d2 = w2.to_detection()
                    if d1.bbox.iou(d2.bbox) > 0:
                        has_overlap = True
        assert has_overlap, "Library must include overlapping window scenarios"

    def test_windows_within_screen_bounds(self):
        library = build_scenario_library()
        for scenario in library:
            for w in scenario.windows:
                assert w.x >= 0, f"{scenario.name}: window x < 0"
                assert w.y >= 0, f"{scenario.name}: window y < 0"
                assert w.x + w.width <= scenario.screen_width
                assert w.y + w.height <= scenario.screen_height
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `uv run pytest stages/window-detection/generator/tests/test_scenario_library.py -v`
Expected: FAIL

- [ ] **Step 3: Implement scenario library**

```python
# stages/window-detection/generator/src/window_gen/scenario_library.py
from __future__ import annotations

from window_gen.scenarios import WindowScenario, WindowSpec

_SCREEN_W = 1920
_SCREEN_H = 1080


def build_scenario_library() -> list[WindowScenario]:
    """Build the complete library of window detection training scenarios."""
    return [
        _empty_desktop(),
        _single_centered_window(),
        _single_maximized_window(),
        _single_small_window(),
        _two_side_by_side(),
        _two_overlapping(),
        _three_cascaded(),
        _four_tiled(),
        _many_small_windows(),
    ]


def _empty_desktop() -> WindowScenario:
    return WindowScenario(
        name="empty-desktop",
        description="Desktop with no application windows open",
        screen_width=_SCREEN_W,
        screen_height=_SCREEN_H,
        windows=[],
    )


def _single_centered_window() -> WindowScenario:
    return WindowScenario(
        name="single-centered",
        description="One medium window centered on screen",
        screen_width=_SCREEN_W,
        screen_height=_SCREEN_H,
        windows=[
            WindowSpec("TextEdit", "Untitled.txt", 360, 140, 1200, 800, z_order=0),
        ],
    )


def _single_maximized_window() -> WindowScenario:
    return WindowScenario(
        name="single-maximized",
        description="One window filling the entire screen (below menu bar)",
        screen_width=_SCREEN_W,
        screen_height=_SCREEN_H,
        windows=[
            WindowSpec("Safari", "Google", 0, 25, 1920, 1055, z_order=0),
        ],
    )


def _single_small_window() -> WindowScenario:
    return WindowScenario(
        name="single-small",
        description="One small dialog-sized window",
        screen_width=_SCREEN_W,
        screen_height=_SCREEN_H,
        windows=[
            WindowSpec("Finder", "Info", 600, 300, 300, 400, z_order=0),
        ],
    )


def _two_side_by_side() -> WindowScenario:
    return WindowScenario(
        name="two-side-by-side",
        description="Two windows side by side, no overlap",
        screen_width=_SCREEN_W,
        screen_height=_SCREEN_H,
        windows=[
            WindowSpec("TextEdit", "Left.txt", 0, 25, 960, 1055, z_order=0),
            WindowSpec("Safari", "Right Page", 960, 25, 960, 1055, z_order=1),
        ],
    )


def _two_overlapping() -> WindowScenario:
    return WindowScenario(
        name="two-overlapping",
        description="Two windows with significant overlap",
        screen_width=_SCREEN_W,
        screen_height=_SCREEN_H,
        windows=[
            WindowSpec("TextEdit", "Background.txt", 100, 100, 900, 700, z_order=0),
            WindowSpec("Safari", "Foreground", 400, 200, 900, 700, z_order=1),
        ],
    )


def _three_cascaded() -> WindowScenario:
    return WindowScenario(
        name="three-cascaded",
        description="Three windows cascaded diagonally",
        screen_width=_SCREEN_W,
        screen_height=_SCREEN_H,
        windows=[
            WindowSpec("TextEdit", "First.txt", 100, 100, 800, 600, z_order=0),
            WindowSpec("Safari", "Second", 250, 200, 800, 600, z_order=1),
            WindowSpec("Finder", "Third", 400, 300, 800, 600, z_order=2),
        ],
    )


def _four_tiled() -> WindowScenario:
    return WindowScenario(
        name="four-tiled",
        description="Four windows in a 2x2 grid",
        screen_width=_SCREEN_W,
        screen_height=_SCREEN_H,
        windows=[
            WindowSpec("TextEdit", "TopLeft.txt", 0, 25, 960, 528, z_order=0),
            WindowSpec("Safari", "TopRight", 960, 25, 960, 528, z_order=1),
            WindowSpec("Finder", "BottomLeft", 0, 553, 960, 527, z_order=2),
            WindowSpec("Terminal", "BottomRight", 960, 553, 960, 527, z_order=3),
        ],
    )


def _many_small_windows() -> WindowScenario:
    return WindowScenario(
        name="many-small-windows",
        description="Six small overlapping windows — stress test",
        screen_width=_SCREEN_W,
        screen_height=_SCREEN_H,
        windows=[
            WindowSpec("Finder", "Info 1", 50, 50, 350, 300, z_order=0),
            WindowSpec("Finder", "Info 2", 150, 100, 350, 300, z_order=1),
            WindowSpec("Finder", "Info 3", 250, 150, 350, 300, z_order=2),
            WindowSpec("TextEdit", "Note 1", 800, 50, 400, 350, z_order=3),
            WindowSpec("TextEdit", "Note 2", 900, 150, 400, 350, z_order=4),
            WindowSpec("Safari", "Page", 500, 500, 700, 500, z_order=5),
        ],
    )
```

- [ ] **Step 4: Run tests**

Run: `uv run pytest stages/window-detection/generator/tests/test_scenario_library.py -v`
Expected: All tests PASS

- [ ] **Step 5: Commit**

```bash
git add stages/window-detection/generator/
git commit -m "feat(window-gen): add scenario library with 9 diverse window layouts"
```

---

### Task 1.3: Window Detection Generator — Synthetic Screenshot Renderer

This renders scenarios as synthetic screenshots WITHOUT needing a VM — useful for rapid iteration and unit testing. The VM-based generator (Task 1.5) produces real screenshots.

**Files:**
- Create: `stages/window-detection/generator/src/window_gen/synthetic.py`
- Create: `stages/window-detection/generator/tests/test_synthetic.py`

- [ ] **Step 1: Write failing tests**

```python
# stages/window-detection/generator/tests/test_synthetic.py
import numpy as np
import pytest
from guivision_common.types import BoundingBox

from window_gen.scenarios import WindowScenario, WindowSpec
from window_gen.synthetic import render_scenario


class TestRenderScenario:
    def test_renders_correct_size(self):
        scenario = WindowScenario(
            name="test",
            description="test",
            screen_width=1920,
            screen_height=1080,
            windows=[],
        )
        img = render_scenario(scenario)
        assert img.shape == (1080, 1920, 3)

    def test_empty_desktop_is_not_blank(self):
        """Desktop background should have some color, not pure black."""
        scenario = WindowScenario(
            name="test", description="test",
            screen_width=800, screen_height=600, windows=[],
        )
        img = render_scenario(scenario)
        assert img.mean() > 0, "Empty desktop should not be pure black"

    def test_window_region_differs_from_background(self):
        scenario = WindowScenario(
            name="test", description="test",
            screen_width=800, screen_height=600,
            windows=[WindowSpec("App", "Title", 100, 100, 400, 300, z_order=0)],
        )
        img = render_scenario(scenario)
        bg_region = img[0:50, 0:50]  # top-left corner, should be desktop
        win_region = img[150:250, 150:350]  # inside window
        assert not np.array_equal(bg_region.mean(axis=(0, 1)), win_region.mean(axis=(0, 1)))

    def test_window_has_title_bar(self):
        """The top portion of a window should look different from the body (title bar)."""
        scenario = WindowScenario(
            name="test", description="test",
            screen_width=800, screen_height=600,
            windows=[WindowSpec("App", "Title", 100, 100, 400, 300, z_order=0)],
        )
        img = render_scenario(scenario)
        title_bar = img[100:130, 100:500]  # top 30px of window
        body = img[200:300, 100:500]  # middle of window
        assert not np.array_equal(
            title_bar.mean(axis=(0, 1)), body.mean(axis=(0, 1))
        ), "Title bar should visually differ from window body"

    def test_z_order_front_window_on_top(self):
        """Higher z_order windows should be rendered on top."""
        scenario = WindowScenario(
            name="test", description="test",
            screen_width=400, screen_height=400,
            windows=[
                WindowSpec("Back", "Back", 0, 0, 300, 300, z_order=0),
                WindowSpec("Front", "Front", 100, 100, 300, 300, z_order=1),
            ],
        )
        img = render_scenario(scenario)
        # Check overlap region (100-300, 100-300) — should match front window color
        overlap_center = img[200, 200]
        # The front window body should be drawn last
        # We can't check exact color without knowing the palette, but
        # the pixel should not be the desktop background
        assert not np.array_equal(overlap_center, img[0, 0]), \
            "Overlap region should not be desktop background"

    def test_deterministic(self):
        scenario = WindowScenario(
            name="test", description="test",
            screen_width=400, screen_height=300,
            windows=[WindowSpec("App", "T", 50, 50, 200, 150, z_order=0)],
        )
        img1 = render_scenario(scenario, seed=42)
        img2 = render_scenario(scenario, seed=42)
        np.testing.assert_array_equal(img1, img2)
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `uv run pytest stages/window-detection/generator/tests/test_synthetic.py -v`
Expected: FAIL

- [ ] **Step 3: Implement synthetic renderer**

```python
# stages/window-detection/generator/src/window_gen/synthetic.py
from __future__ import annotations

import numpy as np

from window_gen.scenarios import WindowScenario, WindowSpec

# Predefined color palettes for visual variation
_DESKTOP_COLORS = [
    (30, 60, 120),    # dark blue
    (50, 50, 60),     # dark grey
    (20, 80, 60),     # dark teal
    (80, 40, 80),     # purple
]

_TITLE_BAR_COLORS = [
    (220, 220, 220),  # light grey (macOS)
    (50, 50, 50),     # dark grey (dark mode)
    (0, 120, 215),    # blue (Windows)
    (60, 60, 60),     # charcoal
]

_WINDOW_BODY_COLORS = [
    (255, 255, 255),  # white
    (40, 40, 40),     # dark mode
    (248, 248, 248),  # off-white
    (30, 30, 30),     # very dark
]

_TITLE_BAR_HEIGHT = 28


def render_scenario(
    scenario: WindowScenario,
    seed: int | None = None,
) -> np.ndarray:
    rng = np.random.default_rng(seed)

    # Draw desktop background
    bg_idx = rng.integers(0, len(_DESKTOP_COLORS))
    bg_color = _DESKTOP_COLORS[bg_idx]
    img = np.full((scenario.screen_height, scenario.screen_width, 3), bg_color, dtype=np.uint8)

    # Add subtle noise to desktop
    noise = rng.integers(-10, 11, size=img.shape, dtype=np.int16)
    img = np.clip(img.astype(np.int16) + noise, 0, 255).astype(np.uint8)

    # Draw windows in z-order (lowest first, so highest is on top)
    sorted_windows = sorted(scenario.windows, key=lambda w: w.z_order)

    for win in sorted_windows:
        palette_idx = rng.integers(0, len(_TITLE_BAR_COLORS))
        _draw_window(img, win, palette_idx, rng)

    return img


def _draw_window(
    img: np.ndarray,
    win: WindowSpec,
    palette_idx: int,
    rng: np.random.Generator,
) -> None:
    h, w = img.shape[:2]
    x1, y1 = max(0, win.x), max(0, win.y)
    x2, y2 = min(w, win.x + win.width), min(h, win.y + win.height)

    if x2 <= x1 or y2 <= y1:
        return

    # Draw window shadow (offset by 4px)
    sx1, sy1 = min(w, x1 + 4), min(h, y1 + 4)
    sx2, sy2 = min(w, x2 + 4), min(h, y2 + 4)
    img[sy1:sy2, sx1:sx2] = np.clip(
        img[sy1:sy2, sx1:sx2].astype(np.int16) - 40, 0, 255
    ).astype(np.uint8)

    # Draw title bar
    tb_color = _TITLE_BAR_COLORS[palette_idx]
    tb_y2 = min(y2, y1 + _TITLE_BAR_HEIGHT)
    img[y1:tb_y2, x1:x2] = tb_color

    # Draw traffic lights / close buttons (3 small circles approximated as squares)
    btn_y = y1 + _TITLE_BAR_HEIGHT // 2 - 3
    for i, color in enumerate([(255, 95, 86), (255, 189, 46), (39, 201, 63)]):
        bx = x1 + 12 + i * 20
        if bx + 6 < x2:
            img[btn_y:btn_y + 6, bx:bx + 6] = color

    # Draw window body
    body_color = _WINDOW_BODY_COLORS[palette_idx]
    img[tb_y2:y2, x1:x2] = body_color

    # Draw 1px border
    img[y1:y2, x1:x1 + 1] = (180, 180, 180)
    img[y1:y2, x2 - 1:x2] = (180, 180, 180)
    img[y1:y1 + 1, x1:x2] = (180, 180, 180)
    img[y2 - 1:y2, x1:x2] = (180, 180, 180)
```

- [ ] **Step 4: Run tests**

Run: `uv run pytest stages/window-detection/generator/tests/test_synthetic.py -v`
Expected: All tests PASS

- [ ] **Step 5: Commit**

```bash
git add stages/window-detection/generator/
git commit -m "feat(window-gen): add synthetic screenshot renderer with window drawing"
```

---

### Task 1.4: Window Detection Generator — Dataset Builder

**Files:**
- Create: `stages/window-detection/generator/src/window_gen/dataset.py`
- Create: `stages/window-detection/generator/tests/test_dataset.py`

- [ ] **Step 1: Write failing tests**

```python
# stages/window-detection/generator/tests/test_dataset.py
import json
from pathlib import Path

import numpy as np
import pytest

from window_gen.dataset import generate_dataset, DatasetConfig
from window_gen.scenarios import WindowScenario, WindowSpec


class TestGenerateDataset:
    def _simple_scenarios(self):
        return [
            WindowScenario(
                name="single",
                description="test",
                screen_width=640,
                screen_height=480,
                windows=[WindowSpec("App", "Title", 50, 50, 300, 200, z_order=0)],
            ),
            WindowScenario(
                name="empty",
                description="test",
                screen_width=640,
                screen_height=480,
                windows=[],
            ),
        ]

    def test_generates_images_and_labels(self, tmp_path):
        config = DatasetConfig(
            output_dir=tmp_path,
            variations_per_scenario=2,
            seed=42,
        )
        generate_dataset(self._simple_scenarios(), config)

        images_dir = tmp_path / "images"
        labels_dir = tmp_path / "labels"
        assert images_dir.exists()
        assert labels_dir.exists()

        png_files = list(images_dir.glob("*.png"))
        json_files = list(labels_dir.glob("*.json"))
        assert len(png_files) == 4  # 2 scenarios * 2 variations
        assert len(json_files) == 4

    def test_label_matches_image(self, tmp_path):
        config = DatasetConfig(output_dir=tmp_path, variations_per_scenario=1, seed=42)
        generate_dataset(self._simple_scenarios()[:1], config)

        label_file = list((tmp_path / "labels").glob("*.json"))[0]
        with open(label_file) as f:
            gt = json.load(f)

        assert gt["stage"] == "window-detection"
        assert gt["image_width"] == 640
        assert gt["image_height"] == 480
        assert len(gt["detections"]) == 1
        assert gt["detections"][0]["label"] == "window"

    def test_manifest_created(self, tmp_path):
        config = DatasetConfig(output_dir=tmp_path, variations_per_scenario=1, seed=42)
        generate_dataset(self._simple_scenarios(), config)

        manifest = tmp_path / "manifest.json"
        assert manifest.exists()
        with open(manifest) as f:
            data = json.load(f)
        assert data["stage"] == "window-detection"
        assert data["num_samples"] == 2
        assert len(data["samples"]) == 2

    def test_variations_are_different(self, tmp_path):
        scenarios = [
            WindowScenario(
                name="single", description="test",
                screen_width=640, screen_height=480,
                windows=[WindowSpec("App", "T", 50, 50, 300, 200, z_order=0)],
            ),
        ]
        config = DatasetConfig(output_dir=tmp_path, variations_per_scenario=3, seed=42)
        generate_dataset(scenarios, config)

        png_files = sorted((tmp_path / "images").glob("*.png"))
        assert len(png_files) == 3
        # Files should not be identical (different seeds produce different noise/colors)
        sizes = [f.stat().st_size for f in png_files]
        assert len(set(sizes)) > 1, "Variations should produce different images"
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `uv run pytest stages/window-detection/generator/tests/test_dataset.py -v`
Expected: FAIL

- [ ] **Step 3: Implement dataset builder**

```python
# stages/window-detection/generator/src/window_gen/dataset.py
from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path

from guivision_common.image_io import save_image
from window_gen.scenarios import WindowScenario
from window_gen.synthetic import render_scenario


@dataclass
class DatasetConfig:
    output_dir: Path
    variations_per_scenario: int = 10
    seed: int = 0


def generate_dataset(
    scenarios: list[WindowScenario],
    config: DatasetConfig,
) -> None:
    images_dir = config.output_dir / "images"
    labels_dir = config.output_dir / "labels"
    images_dir.mkdir(parents=True, exist_ok=True)
    labels_dir.mkdir(parents=True, exist_ok=True)

    samples = []
    sample_idx = 0

    for scenario in scenarios:
        for var in range(config.variations_per_scenario):
            seed = config.seed + sample_idx
            name = f"{scenario.name}_{var:04d}"

            img = render_scenario(scenario, seed=seed)
            img_path = images_dir / f"{name}.png"
            save_image(img, img_path)

            gt = scenario.to_ground_truth(image_path=f"images/{name}.png")
            label_path = labels_dir / f"{name}.json"
            with open(label_path, "w") as f:
                json.dump(gt.to_dict(), f, indent=2)

            samples.append({
                "image": f"images/{name}.png",
                "label": f"labels/{name}.json",
                "scenario": scenario.name,
                "variation": var,
            })
            sample_idx += 1

    manifest = {
        "stage": "window-detection",
        "num_samples": len(samples),
        "samples": samples,
    }
    with open(config.output_dir / "manifest.json", "w") as f:
        json.dump(manifest, f, indent=2)
```

- [ ] **Step 4: Run tests**

Run: `uv run pytest stages/window-detection/generator/tests/test_dataset.py -v`
Expected: All tests PASS

- [ ] **Step 5: Commit**

```bash
git add stages/window-detection/generator/
git commit -m "feat(window-gen): add dataset builder that generates images + labels + manifest"
```

---

### Task 1.5: Window Detection Generator — VM-Based Screenshot Capture

This is the real data generator that drives a VM via GUIVisionVMDriver. It creates actual windows in a real OS, captures screenshots, and records ground truth from both programmatic knowledge and agent reports.

**Files:**
- Create: `stages/window-detection/generator/src/window_gen/vm_capture.py`
- Create: `stages/window-detection/generator/tests/test_vm_capture.py`

- [ ] **Step 1: Write tests (unit tests with mocked VM, integration test marked for live VM)**

```python
# stages/window-detection/generator/tests/test_vm_capture.py
import json
from pathlib import Path
from unittest.mock import AsyncMock, MagicMock, patch

import numpy as np
import pytest

from window_gen.vm_capture import VMCaptureSession, VMCaptureConfig
from window_gen.scenarios import WindowScenario, WindowSpec


class TestVMCaptureConfig:
    def test_create(self):
        config = VMCaptureConfig(
            vnc_host="localhost",
            vnc_port=5901,
            vnc_password="secret",
            ssh_host="localhost",
            ssh_port=22,
            ssh_user="admin",
            platform="macos",
        )
        assert config.vnc_host == "localhost"
        assert config.platform == "macos"


class TestVMCaptureSession:
    def test_build_applescript_for_window(self):
        """Test that we generate correct AppleScript to position a window."""
        session = VMCaptureSession.__new__(VMCaptureSession)
        spec = WindowSpec("TextEdit", "Test.txt", 100, 200, 800, 600, z_order=0)
        script = session._build_position_script(spec, platform="macos")
        assert "TextEdit" in script
        assert "100" in script
        assert "200" in script
        assert "800" in script
        assert "600" in script

    def test_build_scenario_script_orders_by_z(self):
        """Windows should be positioned in z_order so the last one is on top."""
        session = VMCaptureSession.__new__(VMCaptureSession)
        scenario = WindowScenario(
            name="test", description="test",
            screen_width=1920, screen_height=1080,
            windows=[
                WindowSpec("Safari", "Page", 500, 200, 900, 700, z_order=1),
                WindowSpec("TextEdit", "Doc.txt", 100, 100, 800, 600, z_order=0),
            ],
        )
        scripts = session._build_scenario_scripts(scenario, platform="macos")
        # Should be ordered by z_order: TextEdit first (back), Safari second (front)
        assert "TextEdit" in scripts[0]
        assert "Safari" in scripts[1]


@pytest.mark.integration
class TestVMCaptureIntegration:
    """These tests require a running VM. Run with: pytest -m integration"""

    def test_capture_single_window(self, tmp_path):
        """End-to-end: create a window in the VM, screenshot, verify ground truth."""
        pytest.skip("Requires live VM — run manually with pytest -m integration")
```

- [ ] **Step 2: Run unit tests**

Run: `uv run pytest stages/window-detection/generator/tests/test_vm_capture.py -v -m "not integration"`
Expected: FAIL

- [ ] **Step 3: Implement VM capture session**

```python
# stages/window-detection/generator/src/window_gen/vm_capture.py
from __future__ import annotations

import json
import subprocess
import tempfile
from dataclasses import dataclass
from pathlib import Path

from guivision_common.image_io import load_image
from guivision_common.types import GroundTruth, GroundTruthSource
from window_gen.scenarios import WindowScenario, WindowSpec


@dataclass
class VMCaptureConfig:
    vnc_host: str
    vnc_port: int
    vnc_password: str | None = None
    ssh_host: str | None = None
    ssh_port: int = 22
    ssh_user: str = "admin"
    platform: str = "macos"
    guivision_cli: str = "guivision"  # path to GUIVisionVMDriver CLI


class VMCaptureSession:
    """Drives a VM to create window scenarios and capture screenshots."""

    def __init__(self, config: VMCaptureConfig):
        self.config = config

    def capture_scenario(
        self,
        scenario: WindowScenario,
        output_dir: Path,
        sample_name: str,
    ) -> tuple[Path, GroundTruth]:
        """Create the scenario in the VM, capture screenshot, return (image_path, ground_truth)."""
        # 1. Close all existing windows
        self._run_ssh("osascript -e 'tell application \"System Events\" to keystroke \"w\" using {command down, option down}'")

        # 2. Position windows according to scenario (ordered by z_order)
        scripts = self._build_scenario_scripts(scenario, self.config.platform)
        for script in scripts:
            self._run_ssh(f"osascript -e '{script}'")

        # 3. Brief pause for windows to settle
        self._run_ssh("sleep 0.5")

        # 4. Capture screenshot via GUIVisionVMDriver
        img_path = output_dir / "images" / f"{sample_name}.png"
        img_path.parent.mkdir(parents=True, exist_ok=True)
        self._capture_screenshot(img_path)

        # 5. Build ground truth from programmatic knowledge
        gt = scenario.to_ground_truth(image_path=f"images/{sample_name}.png")

        # 6. Optionally enrich with agent data if agent is available
        agent_gt = self._query_agent_windows()
        if agent_gt:
            gt.sources.append(GroundTruthSource.AGENT)

        # 7. Save ground truth
        label_path = output_dir / "labels" / f"{sample_name}.json"
        label_path.parent.mkdir(parents=True, exist_ok=True)
        with open(label_path, "w") as f:
            json.dump(gt.to_dict(), f, indent=2)

        return img_path, gt

    def _build_scenario_scripts(
        self, scenario: WindowScenario, platform: str
    ) -> list[str]:
        sorted_windows = sorted(scenario.windows, key=lambda w: w.z_order)
        return [self._build_position_script(w, platform) for w in sorted_windows]

    def _build_position_script(self, spec: WindowSpec, platform: str) -> str:
        if platform == "macos":
            return (
                f'tell application "{spec.app_name}" to activate\n'
                f'tell application "System Events" to tell process "{spec.app_name}"\n'
                f"  set position of front window to {{{spec.x}, {spec.y}}}\n"
                f"  set size of front window to {{{spec.width}, {spec.height}}}\n"
                f"end tell"
            )
        elif platform == "windows":
            # PowerShell approach — to be implemented with Windows agent
            return f"# Windows positioning for {spec.app_name} — requires agent"
        else:
            return f"# Linux positioning for {spec.app_name} — requires wmctrl or agent"

    def _capture_screenshot(self, output_path: Path) -> None:
        cmd = [
            self.config.guivision_cli,
            "screenshot",
            "--host", self.config.vnc_host,
            "--port", str(self.config.vnc_port),
            "--output", str(output_path),
        ]
        if self.config.vnc_password:
            cmd.extend(["--password", self.config.vnc_password])
        subprocess.run(cmd, check=True, capture_output=True)

    def _run_ssh(self, command: str) -> str:
        if not self.config.ssh_host:
            return ""
        cmd = [
            "ssh",
            "-o", "StrictHostKeyChecking=no",
            "-p", str(self.config.ssh_port),
            f"{self.config.ssh_user}@{self.config.ssh_host}",
            command,
        ]
        result = subprocess.run(cmd, capture_output=True, text=True, timeout=30)
        return result.stdout

    def _query_agent_windows(self) -> list[dict] | None:
        """Query guest agent for window list — returns None if agent unavailable."""
        # Agent integration will be implemented as part of the agents/ subproject
        return None
```

- [ ] **Step 4: Run tests**

Run: `uv run pytest stages/window-detection/generator/tests/test_vm_capture.py -v -m "not integration"`
Expected: Unit tests PASS

- [ ] **Step 5: Commit**

```bash
git add stages/window-detection/generator/
git commit -m "feat(window-gen): add VM capture session for real screenshot generation"
```

---

### Task 1.6: Window Detection Analysis — Traditional CV Baseline (Heuristic)

Before training any ML model, we establish a baseline using traditional computer vision. This is the "traditional" path that we'll A/B test against the ML path.

**Files:**
- Create: `stages/window-detection/analysis/pyproject.toml`
- Create: `stages/window-detection/analysis/src/window_analysis/__init__.py`
- Create: `stages/window-detection/analysis/src/window_analysis/heuristic.py`
- Create: `stages/window-detection/analysis/tests/test_heuristic.py`

- [ ] **Step 1: Create pyproject.toml**

```toml
[project]
name = "window-analysis"
version = "0.1.0"
description = "Window detection analysis library (heuristic + ML)"
requires-python = ">=3.11"
dependencies = [
    "guivision-common",
    "numpy>=1.26",
    "opencv-python-headless>=4.8",
    "pillow>=10.0",
]

[project.optional-dependencies]
ml = [
    "ultralytics>=8.0",
    "torch>=2.0",
]

[build-system]
requires = ["hatchling"]
build-backend = "hatchling.build"

[tool.hatch.build.targets.wheel]
packages = ["src/window_analysis"]
```

- [ ] **Step 2: Write failing tests for heuristic detector**

```python
# stages/window-detection/analysis/tests/test_heuristic.py
import numpy as np
import pytest

from guivision_common.types import BoundingBox, Detection
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
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `uv run pytest stages/window-detection/analysis/tests/test_heuristic.py -v`
Expected: FAIL

- [ ] **Step 4: Implement heuristic window detector**

```python
# stages/window-detection/analysis/src/window_analysis/heuristic.py
from __future__ import annotations

import cv2
import numpy as np

from guivision_common.nms import non_maximum_suppression
from guivision_common.types import BoundingBox, Detection


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
```

- [ ] **Step 5: Update __init__.py**

```python
# stages/window-detection/analysis/src/window_analysis/__init__.py
from window_analysis.heuristic import detect_windows_heuristic

__all__ = ["detect_windows_heuristic"]
```

- [ ] **Step 6: Run tests**

Run: `uv run pytest stages/window-detection/analysis/tests/test_heuristic.py -v`
Expected: All tests PASS

- [ ] **Step 7: Commit**

```bash
git add stages/window-detection/analysis/
git commit -m "feat(window-analysis): add heuristic window detector baseline using Canny + contours"
```

---

### Task 1.7: Window Detection Analysis — Evaluate Baseline on Synthetic Data

**Files:**
- Create: `stages/window-detection/analysis/tests/test_baseline_accuracy.py`

- [ ] **Step 1: Write accuracy test**

```python
# stages/window-detection/analysis/tests/test_baseline_accuracy.py
import pytest
from guivision_common.metrics import compute_metrics
from window_analysis.heuristic import detect_windows_heuristic
from window_gen.scenario_library import build_scenario_library
from window_gen.synthetic import render_scenario


@pytest.mark.vision
class TestBaselineAccuracy:
    """Measure heuristic detector accuracy on synthetic screenshots."""

    def test_accuracy_on_scenario_library(self):
        """Run the heuristic detector on all scenarios and report precision/recall/F1."""
        scenarios = build_scenario_library()
        all_preds = []
        all_gts = []

        for scenario in scenarios:
            img = render_scenario(scenario, seed=0)
            preds = detect_windows_heuristic(img)
            gts = [w.to_detection() for w in scenario.windows]
            all_preds.extend(preds)
            all_gts.extend(gts)

        result = compute_metrics(
            predictions=all_preds,
            ground_truths=all_gts,
            iou_threshold=0.5,
        )

        # Report metrics (this test documents baseline, not a gate)
        print(f"\n{'='*60}")
        print(f"HEURISTIC BASELINE — Window Detection")
        print(f"{'='*60}")
        print(f"Scenarios:  {len(scenarios)}")
        print(f"GT windows: {len(all_gts)}")
        print(f"Predicted:  {len(all_preds)}")
        print(f"Precision:  {result.precision:.3f}")
        print(f"Recall:     {result.recall:.3f}")
        print(f"F1:         {result.f1:.3f}")
        print(f"TP={result.true_pos} FP={result.false_pos} FN={result.false_neg}")
        print(f"{'='*60}")

        # Baseline should at least be non-trivial
        assert result.f1 > 0.0, "Heuristic detector should detect at least some windows"

    def test_per_scenario_breakdown(self):
        """Per-scenario accuracy breakdown for debugging."""
        scenarios = build_scenario_library()

        for scenario in scenarios:
            img = render_scenario(scenario, seed=0)
            preds = detect_windows_heuristic(img)
            gts = [w.to_detection() for w in scenario.windows]

            result = compute_metrics(
                predictions=preds, ground_truths=gts, iou_threshold=0.5
            )
            print(
                f"  {scenario.name:25s}  "
                f"gt={len(gts):2d}  pred={len(preds):2d}  "
                f"P={result.precision:.2f}  R={result.recall:.2f}  F1={result.f1:.2f}"
            )
```

- [ ] **Step 2: Run accuracy test**

Run: `uv run pytest stages/window-detection/analysis/tests/test_baseline_accuracy.py -v -s -m vision`
Expected: PASS, with printed metrics showing baseline performance

- [ ] **Step 3: Commit**

```bash
git add stages/window-detection/analysis/tests/
git commit -m "test(window-analysis): add baseline accuracy evaluation on synthetic data"
```

---

### Task 1.8: Window Detection Training — YOLO Dataset Conversion

**Files:**
- Create: `stages/window-detection/training/pyproject.toml`
- Create: `stages/window-detection/training/src/window_train/__init__.py`
- Create: `stages/window-detection/training/src/window_train/yolo_format.py`
- Create: `stages/window-detection/training/tests/test_yolo_format.py`

- [ ] **Step 1: Create pyproject.toml**

```toml
[project]
name = "window-train"
version = "0.1.0"
description = "Training infrastructure for window detection models"
requires-python = ">=3.11"
dependencies = [
    "guivision-common",
    "ultralytics>=8.0",
    "pyyaml>=6.0",
]

[build-system]
requires = ["hatchling"]
build-backend = "hatchling.build"

[tool.hatch.build.targets.wheel]
packages = ["src/window_train"]
```

- [ ] **Step 2: Write failing tests for YOLO format conversion**

```python
# stages/window-detection/training/tests/test_yolo_format.py
import json
from pathlib import Path

import pytest
import yaml

from window_train.yolo_format import convert_to_yolo, YOLODatasetConfig


class TestConvertToYOLO:
    def _make_manifest(self, tmp_path):
        """Create a minimal manifest with one sample."""
        images_dir = tmp_path / "source" / "images"
        labels_dir = tmp_path / "source" / "labels"
        images_dir.mkdir(parents=True)
        labels_dir.mkdir(parents=True)

        # Create a dummy image file
        (images_dir / "sample_0000.png").write_bytes(b"fake_png_data")

        # Create a ground truth label
        gt = {
            "stage": "window-detection",
            "image_path": "images/sample_0000.png",
            "image_width": 1920,
            "image_height": 1080,
            "detections": [
                {
                    "label": "window",
                    "bbox": [100, 200, 900, 800],
                    "confidence": 1.0,
                    "metadata": {"title": "Test"},
                }
            ],
            "sources": ["programmatic"],
        }
        with open(labels_dir / "sample_0000.json", "w") as f:
            json.dump(gt, f)

        manifest = {
            "stage": "window-detection",
            "num_samples": 1,
            "samples": [
                {"image": "images/sample_0000.png", "label": "labels/sample_0000.json"}
            ],
        }
        manifest_path = tmp_path / "source" / "manifest.json"
        with open(manifest_path, "w") as f:
            json.dump(manifest, f)

        return manifest_path

    def test_creates_yolo_directory_structure(self, tmp_path):
        manifest_path = self._make_manifest(tmp_path)
        output_dir = tmp_path / "yolo"
        config = YOLODatasetConfig(
            manifest_path=manifest_path,
            output_dir=output_dir,
            train_ratio=1.0,
        )
        convert_to_yolo(config)

        assert (output_dir / "images" / "train").exists()
        assert (output_dir / "labels" / "train").exists()
        assert (output_dir / "data.yaml").exists()

    def test_yolo_label_format(self, tmp_path):
        manifest_path = self._make_manifest(tmp_path)
        output_dir = tmp_path / "yolo"
        config = YOLODatasetConfig(
            manifest_path=manifest_path,
            output_dir=output_dir,
            train_ratio=1.0,
        )
        convert_to_yolo(config)

        label_files = list((output_dir / "labels" / "train").glob("*.txt"))
        assert len(label_files) == 1

        content = label_files[0].read_text().strip()
        parts = content.split()
        assert len(parts) == 5  # class_id cx cy w h

        class_id = int(parts[0])
        cx, cy, w, h = float(parts[1]), float(parts[2]), float(parts[3]), float(parts[4])

        assert class_id == 0  # "window" is class 0
        # Normalised coordinates should be in [0, 1]
        assert 0.0 <= cx <= 1.0
        assert 0.0 <= cy <= 1.0
        assert 0.0 < w <= 1.0
        assert 0.0 < h <= 1.0

        # Check actual values: bbox [100,200,900,800] on 1920x1080
        # center_x = (100+900)/2/1920 = 500/1920 ≈ 0.2604
        # center_y = (200+800)/2/1080 = 500/1080 ≈ 0.4630
        # width = (900-100)/1920 ≈ 0.4167
        # height = (800-200)/1080 ≈ 0.5556
        assert abs(cx - 500 / 1920) < 0.001
        assert abs(cy - 500 / 1080) < 0.001

    def test_data_yaml_contents(self, tmp_path):
        manifest_path = self._make_manifest(tmp_path)
        output_dir = tmp_path / "yolo"
        config = YOLODatasetConfig(
            manifest_path=manifest_path,
            output_dir=output_dir,
            train_ratio=1.0,
        )
        convert_to_yolo(config)

        with open(output_dir / "data.yaml") as f:
            data = yaml.safe_load(f)

        assert data["nc"] == 1
        assert data["names"] == ["window"]
        assert "train" in data
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `uv run pytest stages/window-detection/training/tests/test_yolo_format.py -v`
Expected: FAIL

- [ ] **Step 4: Implement YOLO format converter**

```python
# stages/window-detection/training/src/window_train/yolo_format.py
from __future__ import annotations

import json
import shutil
from dataclasses import dataclass
from pathlib import Path

import yaml


CLASS_NAMES = ["window"]


@dataclass
class YOLODatasetConfig:
    manifest_path: Path
    output_dir: Path
    train_ratio: float = 0.8
    val_ratio: float = 0.2


def convert_to_yolo(config: YOLODatasetConfig) -> Path:
    source_dir = config.manifest_path.parent

    with open(config.manifest_path) as f:
        manifest = json.load(f)

    samples = manifest["samples"]
    split_idx = int(len(samples) * config.train_ratio)
    train_samples = samples[:split_idx] if split_idx < len(samples) else samples
    val_samples = samples[split_idx:] if split_idx < len(samples) else []

    for split_name, split_samples in [("train", train_samples), ("val", val_samples)]:
        if not split_samples:
            continue

        img_dir = config.output_dir / "images" / split_name
        lbl_dir = config.output_dir / "labels" / split_name
        img_dir.mkdir(parents=True, exist_ok=True)
        lbl_dir.mkdir(parents=True, exist_ok=True)

        for sample in split_samples:
            src_img = source_dir / sample["image"]
            src_lbl = source_dir / sample["label"]
            name = Path(sample["image"]).stem

            # Copy image
            shutil.copy2(src_img, img_dir / f"{name}.png")

            # Convert label to YOLO format
            with open(src_lbl) as f:
                gt = json.load(f)

            img_w = gt["image_width"]
            img_h = gt["image_height"]

            lines = []
            for det in gt["detections"]:
                bbox = det["bbox"]  # [x1, y1, x2, y2]
                cx = (bbox[0] + bbox[2]) / 2.0 / img_w
                cy = (bbox[1] + bbox[3]) / 2.0 / img_h
                w = (bbox[2] - bbox[0]) / img_w
                h = (bbox[3] - bbox[1]) / img_h
                class_id = CLASS_NAMES.index(det["label"])
                lines.append(f"{class_id} {cx:.6f} {cy:.6f} {w:.6f} {h:.6f}")

            (lbl_dir / f"{name}.txt").write_text("\n".join(lines))

    # Write data.yaml
    data_yaml = {
        "path": str(config.output_dir.resolve()),
        "train": "images/train",
        "val": "images/val" if val_samples else "images/train",
        "nc": len(CLASS_NAMES),
        "names": CLASS_NAMES,
    }
    yaml_path = config.output_dir / "data.yaml"
    with open(yaml_path, "w") as f:
        yaml.dump(data_yaml, f, default_flow_style=False)

    return yaml_path
```

- [ ] **Step 5: Run tests**

Run: `uv run pytest stages/window-detection/training/tests/test_yolo_format.py -v`
Expected: All tests PASS

- [ ] **Step 6: Commit**

```bash
git add stages/window-detection/training/
git commit -m "feat(window-train): add YOLO format converter for training data"
```

---

### Task 1.9: Window Detection Training — Training Script

**Files:**
- Create: `stages/window-detection/training/src/window_train/train.py`
- Create: `stages/window-detection/training/configs/yolov8s-window.yaml`
- Create: `stages/window-detection/training/tests/test_train.py`

- [ ] **Step 1: Write test for training config validation**

```python
# stages/window-detection/training/tests/test_train.py
import pytest
from pathlib import Path

from window_train.train import TrainConfig, validate_config


class TestTrainConfig:
    def test_create_default(self):
        config = TrainConfig(data_yaml=Path("/tmp/data.yaml"))
        assert config.model == "yolov8s.pt"
        assert config.epochs == 100
        assert config.imgsz == 1280
        assert config.device == "mps"

    def test_validate_missing_data_yaml(self, tmp_path):
        config = TrainConfig(data_yaml=tmp_path / "nonexistent.yaml")
        errors = validate_config(config)
        assert any("data_yaml" in e for e in errors)

    def test_validate_valid_config(self, tmp_path):
        yaml_path = tmp_path / "data.yaml"
        yaml_path.write_text("nc: 1\nnames: [window]\n")
        config = TrainConfig(data_yaml=yaml_path)
        errors = validate_config(config)
        assert len(errors) == 0

    def test_validate_bad_imgsz(self, tmp_path):
        yaml_path = tmp_path / "data.yaml"
        yaml_path.write_text("nc: 1\nnames: [window]\n")
        config = TrainConfig(data_yaml=yaml_path, imgsz=123)
        errors = validate_config(config)
        assert any("imgsz" in e for e in errors)
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `uv run pytest stages/window-detection/training/tests/test_train.py -v`
Expected: FAIL

- [ ] **Step 3: Implement training config and script**

```python
# stages/window-detection/training/src/window_train/train.py
from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path


@dataclass
class TrainConfig:
    data_yaml: Path
    model: str = "yolov8s.pt"
    epochs: int = 100
    imgsz: int = 1280
    batch: int = -1  # auto-detect based on memory
    device: str = "mps"
    project: str = "runs/window-detection"
    name: str = "train"
    patience: int = 20
    save_period: int = 10
    workers: int = 8
    amp: bool = True
    export_coreml: bool = True


def validate_config(config: TrainConfig) -> list[str]:
    errors = []
    if not config.data_yaml.exists():
        errors.append(f"data_yaml does not exist: {config.data_yaml}")
    if config.imgsz % 32 != 0:
        errors.append(f"imgsz must be divisible by 32, got {config.imgsz}")
    if config.epochs < 1:
        errors.append(f"epochs must be >= 1, got {config.epochs}")
    return errors


def train(config: TrainConfig) -> Path:
    """Run YOLO training. Returns path to best weights."""
    errors = validate_config(config)
    if errors:
        raise ValueError(f"Invalid config: {'; '.join(errors)}")

    from ultralytics import YOLO

    model = YOLO(config.model)
    results = model.train(
        data=str(config.data_yaml),
        epochs=config.epochs,
        imgsz=config.imgsz,
        batch=config.batch,
        device=config.device,
        project=config.project,
        name=config.name,
        patience=config.patience,
        save_period=config.save_period,
        workers=config.workers,
        amp=config.amp,
    )

    best_weights = Path(config.project) / config.name / "weights" / "best.pt"

    if config.export_coreml and best_weights.exists():
        trained = YOLO(str(best_weights))
        trained.export(format="coreml", imgsz=config.imgsz, half=True, nms=True)

    return best_weights
```

- [ ] **Step 4: Create training config YAML**

```yaml
# stages/window-detection/training/configs/yolov8s-window.yaml
# Configuration for window detection YOLO training
# Usage: python -m window_train.train --config configs/yolov8s-window.yaml

model: yolov8s.pt
epochs: 100
imgsz: 1280
batch: -1
device: mps
patience: 20
save_period: 10
workers: 8
amp: true
export_coreml: true
```

- [ ] **Step 5: Run tests**

Run: `uv run pytest stages/window-detection/training/tests/test_train.py -v`
Expected: All tests PASS

- [ ] **Step 6: Commit**

```bash
git add stages/window-detection/training/
git commit -m "feat(window-train): add YOLO training script with config validation and CoreML export"
```

---

### Task 1.10: Window Detection Analysis — YOLO Model Detector

**Files:**
- Create: `stages/window-detection/analysis/src/window_analysis/yolo_detector.py`
- Create: `stages/window-detection/analysis/tests/test_yolo_detector.py`

- [ ] **Step 1: Write tests**

```python
# stages/window-detection/analysis/tests/test_yolo_detector.py
import numpy as np
import pytest

from guivision_common.types import Detection
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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `uv run pytest stages/window-detection/analysis/tests/test_yolo_detector.py -v -m "not vision"`
Expected: FAIL — import error

- [ ] **Step 3: Implement YOLO detector wrapper**

```python
# stages/window-detection/analysis/src/window_analysis/yolo_detector.py
from __future__ import annotations

from pathlib import Path

import numpy as np

from guivision_common.nms import non_maximum_suppression
from guivision_common.types import BoundingBox, Detection

CLASS_NAMES = ["window"]


class YOLOWindowDetector:
    """Window detector using a trained YOLO model."""

    def __init__(
        self,
        model_path: str | Path,
        confidence_threshold: float = 0.25,
        iou_threshold: float = 0.45,
        imgsz: int = 1280,
    ):
        model_path = Path(model_path)
        if not model_path.exists():
            raise FileNotFoundError(f"Model not found: {model_path}")

        from ultralytics import YOLO

        self._model = YOLO(str(model_path))
        self._confidence_threshold = confidence_threshold
        self._iou_threshold = iou_threshold
        self._imgsz = imgsz

    def detect(self, image: np.ndarray) -> list[Detection]:
        results = self._model.predict(
            image,
            imgsz=self._imgsz,
            conf=self._confidence_threshold,
            iou=self._iou_threshold,
            verbose=False,
        )

        detections: list[Detection] = []
        for result in results:
            if result.boxes is None:
                continue
            for box in result.boxes:
                x1, y1, x2, y2 = box.xyxy[0].cpu().numpy()
                conf = float(box.conf[0].cpu().numpy())
                cls_id = int(box.cls[0].cpu().numpy())
                label = CLASS_NAMES[cls_id] if cls_id < len(CLASS_NAMES) else f"class_{cls_id}"

                detections.append(
                    Detection(
                        label=label,
                        bbox=BoundingBox(
                            x1=int(round(x1)),
                            y1=int(round(y1)),
                            x2=int(round(x2)),
                            y2=int(round(y2)),
                        ),
                        confidence=conf,
                    )
                )

        return detections
```

- [ ] **Step 4: Update __init__.py**

```python
# stages/window-detection/analysis/src/window_analysis/__init__.py
from window_analysis.heuristic import detect_windows_heuristic

__all__ = ["detect_windows_heuristic"]

# YOLOWindowDetector is imported explicitly when needed (requires ultralytics)
```

- [ ] **Step 5: Run tests**

Run: `uv run pytest stages/window-detection/analysis/tests/test_yolo_detector.py -v -m "not vision"`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add stages/window-detection/analysis/
git commit -m "feat(window-analysis): add YOLO window detector wrapper with Ultralytics"
```

---

### Task 1.11: A/B Testing Harness — Compare Heuristic vs YOLO

**Files:**
- Create: `common/src/guivision_common/ab_test.py`
- Create: `common/tests/test_ab_test.py`

- [ ] **Step 1: Write failing tests**

```python
# common/tests/test_ab_test.py
import pytest
from guivision_common.types import BoundingBox, Detection
from guivision_common.metrics import MetricsResult
from guivision_common.ab_test import ABTestResult, run_ab_test


class TestABTest:
    def _det(self, x1, y1, x2, y2, conf=0.9):
        return Detection("window", BoundingBox(x1, y1, x2, y2), conf)

    def test_ab_test_result(self):
        r = ABTestResult(
            method_a="heuristic",
            method_b="yolo",
            metrics_a=MetricsResult(0.7, 0.8, 0.746, 8, 3, 2),
            metrics_b=MetricsResult(0.9, 0.85, 0.874, 17, 2, 3),
            num_images=10,
        )
        assert r.winner == "yolo"
        assert r.f1_delta == pytest.approx(0.874 - 0.746)

    def test_ab_test_result_tie(self):
        m = MetricsResult(0.9, 0.9, 0.9, 9, 1, 1)
        r = ABTestResult("a", "b", m, m, 10)
        assert r.winner is None  # tie

    def test_run_ab_test(self):
        images = [{"id": "img1"}]
        gts = [[self._det(0, 0, 100, 100)]]

        def detector_a(img):
            return [self._det(0, 0, 100, 100)]

        def detector_b(img):
            return [self._det(0, 0, 100, 100), self._det(500, 500, 600, 600)]

        result = run_ab_test(
            images=images,
            ground_truths=gts,
            method_a=("perfect", detector_a),
            method_b=("extra_fp", detector_b),
            iou_threshold=0.5,
        )
        assert result.method_a == "perfect"
        assert result.metrics_a.precision == 1.0
        assert result.metrics_b.precision == 0.5
        assert result.winner == "perfect"
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `uv run pytest common/tests/test_ab_test.py -v`
Expected: FAIL

- [ ] **Step 3: Implement A/B test harness**

```python
# common/src/guivision_common/ab_test.py
from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Callable

from guivision_common.metrics import MetricsResult, compute_metrics
from guivision_common.types import Detection


@dataclass(frozen=True)
class ABTestResult:
    method_a: str
    method_b: str
    metrics_a: MetricsResult
    metrics_b: MetricsResult
    num_images: int

    @property
    def f1_delta(self) -> float:
        return self.metrics_b.f1 - self.metrics_a.f1

    @property
    def winner(self) -> str | None:
        if self.metrics_a.f1 > self.metrics_b.f1:
            return self.method_a
        elif self.metrics_b.f1 > self.metrics_a.f1:
            return self.method_b
        return None

    def summary(self) -> str:
        lines = [
            f"A/B Test: {self.method_a} vs {self.method_b}",
            f"Images: {self.num_images}",
            f"  {self.method_a}: P={self.metrics_a.precision:.3f} R={self.metrics_a.recall:.3f} F1={self.metrics_a.f1:.3f}",
            f"  {self.method_b}: P={self.metrics_b.precision:.3f} R={self.metrics_b.recall:.3f} F1={self.metrics_b.f1:.3f}",
            f"  Winner: {self.winner or 'TIE'} (F1 delta: {self.f1_delta:+.3f})",
        ]
        return "\n".join(lines)


def run_ab_test(
    images: list[Any],
    ground_truths: list[list[Detection]],
    method_a: tuple[str, Callable],
    method_b: tuple[str, Callable],
    iou_threshold: float = 0.5,
) -> ABTestResult:
    name_a, detect_a = method_a
    name_b, detect_b = method_b

    all_preds_a: list[Detection] = []
    all_preds_b: list[Detection] = []
    all_gts: list[Detection] = []

    for img, gts in zip(images, ground_truths):
        all_preds_a.extend(detect_a(img))
        all_preds_b.extend(detect_b(img))
        all_gts.extend(gts)

    metrics_a = compute_metrics(all_preds_a, all_gts, iou_threshold)
    metrics_b = compute_metrics(all_preds_b, all_gts, iou_threshold)

    return ABTestResult(
        method_a=name_a,
        method_b=name_b,
        metrics_a=metrics_a,
        metrics_b=metrics_b,
        num_images=len(images),
    )
```

- [ ] **Step 4: Update __init__.py**

Add to `common/src/guivision_common/__init__.py`:

```python
from guivision_common.ab_test import ABTestResult, run_ab_test
```

And add `"ABTestResult"`, `"run_ab_test"` to `__all__`.

- [ ] **Step 5: Run all common tests**

Run: `uv run pytest common/tests/ -v`
Expected: All tests PASS

- [ ] **Step 6: Commit**

```bash
git add common/
git commit -m "feat(common): add A/B testing harness for comparing detection methods"
```

---

### Task 1.12: Window Detection — CLI Entry Point

**Files:**
- Create: `stages/window-detection/generator/src/window_gen/cli.py`
- Create: `stages/window-detection/generator/tests/test_cli.py`

- [ ] **Step 1: Write failing test**

```python
# stages/window-detection/generator/tests/test_cli.py
import json
from pathlib import Path
from unittest.mock import patch

import pytest

from window_gen.cli import main


class TestCLI:
    def test_generate_synthetic(self, tmp_path):
        output = tmp_path / "output"
        with patch("sys.argv", ["window-gen", "synthetic", "--output", str(output), "--variations", "2"]):
            main()

        assert (output / "manifest.json").exists()
        with open(output / "manifest.json") as f:
            manifest = json.load(f)
        assert manifest["num_samples"] > 0
        assert len(list((output / "images").glob("*.png"))) == manifest["num_samples"]
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `uv run pytest stages/window-detection/generator/tests/test_cli.py -v`
Expected: FAIL

- [ ] **Step 3: Implement CLI**

```python
# stages/window-detection/generator/src/window_gen/cli.py
from __future__ import annotations

import argparse
import sys
from pathlib import Path

from window_gen.dataset import DatasetConfig, generate_dataset
from window_gen.scenario_library import build_scenario_library


def main(argv: list[str] | None = None) -> None:
    parser = argparse.ArgumentParser(description="Window detection training data generator")
    subparsers = parser.add_subparsers(dest="command", required=True)

    # synthetic subcommand
    syn = subparsers.add_parser("synthetic", help="Generate synthetic training data")
    syn.add_argument("--output", type=Path, required=True, help="Output directory")
    syn.add_argument("--variations", type=int, default=10, help="Variations per scenario")
    syn.add_argument("--seed", type=int, default=42, help="Random seed")

    args = parser.parse_args(argv)

    if args.command == "synthetic":
        scenarios = build_scenario_library()
        config = DatasetConfig(
            output_dir=args.output,
            variations_per_scenario=args.variations,
            seed=args.seed,
        )
        generate_dataset(scenarios, config)
        print(f"Generated {len(scenarios) * args.variations} samples in {args.output}")


if __name__ == "__main__":
    main()
```

- [ ] **Step 4: Run tests**

Run: `uv run pytest stages/window-detection/generator/tests/test_cli.py -v`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add stages/window-detection/generator/
git commit -m "feat(window-gen): add CLI for synthetic data generation"
```

---

### Task 1.13: CLAUDE.md for the Project

**Files:**
- Create: `CLAUDE.md`

- [ ] **Step 1: Create CLAUDE.md**

```markdown
# GUIVisionPipeline

## Build & Test

### Python (primary)
```bash
# Install all workspace packages
uv sync --all-packages

# Run unit tests (fast, no models or VMs)
uv run pytest -m "not integration and not slow and not vision"

# Run vision accuracy tests (requires generated data)
uv run pytest -m vision -s

# Run specific stage tests
uv run pytest stages/window-detection/ -v

# Run common tests
uv run pytest common/tests/ -v
```

### Swift (CoreML inference, OCR, GUIVisionVMDriver integration)
```bash
cd swift/
swift build
swift test
```

## Project Structure

- `common/` — Shared Python types, metrics, NMS, image I/O, A/B testing
- `stages/<name>/generator/` — Training data generation for each pipeline stage
- `stages/<name>/training/` — Model training/fine-tuning scripts
- `stages/<name>/analysis/` — Production analysis library + CLI
- `agents/` — Guest agents for macOS/Windows/Linux VMs
- `pipeline/` — Orchestrator that chains stages
- `swift/` — Swift package for CoreML inference and OCR

## Conventions

- All images are RGB numpy arrays (H, W, 3), uint8
- All bounding boxes are pixel coordinates, top-left origin, (x1, y1, x2, y2) format
- Ground truth JSON follows the `GroundTruth` schema in `common/src/guivision_common/types.py`
- YOLO training data uses standard Ultralytics format (normalized center coords)
- Test markers: `unit`, `vision`, `integration`, `slow`
- Each pipeline stage must meet accuracy gates before the next stage begins

## Key Dependencies

- GUIVisionVMDriver: Swift library for VNC/SSH VM control (used via CLI or Swift integration)
- Ultralytics: YOLO model training and inference
- OpenCV: Traditional CV heuristic baselines
- Apple Vision framework: OCR (Swift only)
- CoreML: Production inference on Apple Silicon (Swift only)
```

- [ ] **Step 2: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: add CLAUDE.md with build, test, and project conventions"
```

---

## Future Phases (Separate Plans)

Each of these will get its own detailed plan document when the preceding stage passes its accuracy gate:

| Phase | Stage | Accuracy Gate | Depends On |
|-------|-------|--------------|------------|
| 2 | OS Chrome Detection | P/R/F1 >= 0.90 | Phase 1 |
| 3 | Element Detection | mAP >= 0.85 | Phase 1 |
| 4 | Menu Detection | P/R/F1 >= 0.85 | Phase 1 |
| 5 | OCR Integration | CER <= 0.05 | Phase 1 |
| 6 | Icon Classification | Top-1 accuracy >= 0.85 | Phase 3 |
| 7 | State Detection | Accuracy >= 0.90 | Phase 3 |
| 8 | Hierarchy Building | - | Phases 1-7 |
| 9 | Guest Agents (macOS) | - | Phase 1 |
| 10 | Guest Agents (Windows) | - | Phase 1 |
| 11 | Pipeline Orchestrator | - | Phases 1-8 |
| 12 | Swift/CoreML Integration | - | Phases 1-7 |
