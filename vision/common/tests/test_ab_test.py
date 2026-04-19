import pytest
from testanyware_common.types import BoundingBox, Detection
from testanyware_common.metrics import MetricsResult
from testanyware_common.ab_test import ABTestResult, run_ab_test


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
