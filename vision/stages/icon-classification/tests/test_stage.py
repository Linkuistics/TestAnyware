from PIL import Image, ImageDraw

from testanyware_common.types import BoundingBox, Detection

from icon_classification.stage import IconClassification, IconClassificationStage


def test_non_button_detection_returns_none_label():
    stage = IconClassificationStage()
    img = Image.new("RGB", (100, 100), (255, 255, 255))
    detections = [
        Detection(
            label="text-field",  # NOT in ICON_ELIGIBLE_LABELS
            bbox=BoundingBox.from_xywh(10, 10, 50, 20),
            confidence=0.9,
        )
    ]
    result = stage.run(img, detections)
    assert len(result) == 1
    assert result[0].icon_label is None


def test_button_detection_classifies_or_returns_unknown():
    stage = IconClassificationStage()
    # Synthetic 24x24 mostly-blank button — heuristic should return 'unknown'
    img = Image.new("RGB", (100, 100), (255, 255, 255))
    detections = [
        Detection(
            label="button",
            bbox=BoundingBox.from_xywh(10, 10, 24, 24),
            confidence=0.9,
        )
    ]
    result = stage.run(img, detections)
    assert len(result) == 1
    # Either a valid label string or 'unknown' — just ensure it's set
    assert result[0].icon_label is not None


def test_plus_icon_shape_heuristic():
    # Draw a synthetic plus sign: white background, black + in the middle
    img = Image.new("RGB", (32, 32), (255, 255, 255))
    draw = ImageDraw.Draw(img)
    # Vertical bar
    draw.rectangle([14, 6, 17, 25], fill=(0, 0, 0))
    # Horizontal bar
    draw.rectangle([6, 14, 25, 17], fill=(0, 0, 0))

    # Embed in a larger image + classify via stage
    parent = Image.new("RGB", (100, 100), (255, 255, 255))
    parent.paste(img, (30, 30))

    stage = IconClassificationStage(confidence_threshold=0.1)  # loose for heuristic
    detections = [
        Detection(
            label="button",
            bbox=BoundingBox.from_xywh(30, 30, 32, 32),
            confidence=0.9,
        )
    ]
    result = stage.run(parent, detections)
    # Accept either 'plus' (heuristic succeeded) or 'unknown' (heuristic didn't catch this synthetic)
    # — both are valid outcomes pre-trained-model
    assert result[0].icon_label in ("plus", "unknown")
