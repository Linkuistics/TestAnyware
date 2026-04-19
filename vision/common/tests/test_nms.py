from testanyware_common.types import BoundingBox, Detection
from testanyware_common.nms import non_maximum_suppression


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
