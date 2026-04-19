from __future__ import annotations

from dataclasses import dataclass

from PIL import Image

from testanyware_common.types import Detection

from .border import detect_border
from .color import extract_dominant_color
from .shadow import detect_shadow


@dataclass(frozen=True)
class ElementPrimitives:
    element_id: int
    dominant_color: tuple[int, int, int]
    border: dict
    shadow: dict


class DrawingPrimitivesStage:
    """Extract per-element drawing primitives from a screenshot + detections."""

    def run(
        self, image: Image.Image, detections: list[Detection]
    ) -> list[ElementPrimitives]:
        results: list[ElementPrimitives] = []
        for idx, det in enumerate(detections):
            bb = det.bbox
            crop = image.crop((bb.x1, bb.y1, bb.x2, bb.y2))
            results.append(
                ElementPrimitives(
                    element_id=idx,
                    dominant_color=extract_dominant_color(crop),
                    border=detect_border(crop),
                    shadow=detect_shadow(image, bb),
                )
            )
        return results
