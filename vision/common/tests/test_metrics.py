import pytest
from testanyware_common.types import BoundingBox, Detection
from testanyware_common.metrics import compute_metrics, MetricsResult


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
