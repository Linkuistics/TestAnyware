from __future__ import annotations

from dataclasses import dataclass

from testanyware_common.types import Detection


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
