from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Callable

from testanyware_common.metrics import MetricsResult, compute_metrics
from testanyware_common.types import Detection


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
