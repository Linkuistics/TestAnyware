from __future__ import annotations

from testanyware_common.types import Detection


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
