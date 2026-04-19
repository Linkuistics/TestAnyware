from __future__ import annotations

from pathlib import Path

import numpy as np

from testanyware_common.nms import non_maximum_suppression
from testanyware_common.types import BoundingBox, Detection

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
