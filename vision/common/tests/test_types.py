import pytest
from testanyware_common.types import BoundingBox


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
