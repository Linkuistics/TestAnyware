from PIL import Image

from testanyware_common.types import BoundingBox, Detection

from drawing_primitives.stage import DrawingPrimitivesStage


def test_extract_primitives_from_single_element():
    img = Image.new("RGB", (200, 100), (240, 240, 240))
    stage = DrawingPrimitivesStage()
    detections = [
        Detection(
            bbox=BoundingBox.from_xywh(x=10, y=10, w=100, h=50),
            label="button",
            confidence=0.9,
        )
    ]
    result = stage.run(image=img, detections=detections)
    assert len(result) == 1
    primitives = result[0]
    assert primitives.element_id == 0
    assert primitives.dominant_color is not None
    assert isinstance(primitives.dominant_color, tuple)
    assert len(primitives.dominant_color) == 3
    assert primitives.border is not None
    assert primitives.shadow is not None
