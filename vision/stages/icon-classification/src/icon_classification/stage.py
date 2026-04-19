"""Pipeline stage wrapper around the icon classifier.

Given a screenshot and the list of detections from an upstream element-detection
stage, produces one `IconClassification` per input detection. Non-button
detections get `icon_label=None`; eligible detections are cropped and classified.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Optional

from PIL import Image

from testanyware_common.types import Detection

from .classifier import IconClassifier

# Detection labels that should be classified as icons.
ICON_ELIGIBLE_LABELS = {"button", "toolbar-button", "toggle-button", "AXButton", "AXImage"}


@dataclass(frozen=True)
class IconClassification:
    element_id: int
    icon_label: Optional[str]  # None for non-button elements
    confidence: float


class IconClassificationStage:
    def __init__(self, model_path=None, confidence_threshold: float = 0.5):
        self.classifier = IconClassifier(model_path, confidence_threshold)

    def run(
        self, image: Image.Image, detections: list[Detection]
    ) -> list[IconClassification]:
        results: list[IconClassification] = []
        for idx, det in enumerate(detections):
            if det.label not in ICON_ELIGIBLE_LABELS:
                results.append(
                    IconClassification(element_id=idx, icon_label=None, confidence=0.0)
                )
                continue
            bb = det.bbox
            crop = image.crop((bb.x1, bb.y1, bb.x2, bb.y2))
            label, conf = self.classifier.classify(crop)
            results.append(
                IconClassification(element_id=idx, icon_label=label, confidence=conf)
            )
        return results
