import pytest
from window_gen.scenarios import WindowScenario, WindowSpec


class TestWindowSpec:
    def test_create(self):
        spec = WindowSpec(
            app_name="TextEdit",
            title="Untitled",
            x=100,
            y=100,
            width=800,
            height=600,
            z_order=0,
        )
        assert spec.app_name == "TextEdit"
        assert spec.width == 800

    def test_to_ground_truth_detection(self):
        spec = WindowSpec(
            app_name="TextEdit",
            title="Untitled",
            x=100, y=100, width=800, height=600,
            z_order=0,
        )
        det = spec.to_detection()
        assert det.label == "window"
        assert det.bbox.x1 == 100
        assert det.bbox.x2 == 900
        assert det.confidence == 1.0
        assert det.metadata["title"] == "Untitled"
        assert det.metadata["app_name"] == "TextEdit"
        assert det.metadata["z_order"] == 0


class TestWindowScenario:
    def test_create(self):
        scenario = WindowScenario(
            name="two-overlapping-windows",
            description="Two TextEdit windows overlapping by 200px",
            screen_width=1920,
            screen_height=1080,
            windows=[
                WindowSpec("TextEdit", "Doc1.txt", 100, 100, 800, 600, z_order=0),
                WindowSpec("Safari", "Google", 500, 200, 900, 700, z_order=1),
            ],
        )
        assert len(scenario.windows) == 2
        assert scenario.screen_width == 1920

    def test_to_ground_truth(self):
        scenario = WindowScenario(
            name="single-window",
            description="One maximized window",
            screen_width=1920,
            screen_height=1080,
            windows=[
                WindowSpec("TextEdit", "Doc.txt", 0, 25, 1920, 1055, z_order=0),
            ],
        )
        gt = scenario.to_ground_truth(image_path="screenshot_001.png")
        assert gt.stage == "window-detection"
        assert gt.image_path == "screenshot_001.png"
        assert gt.image_width == 1920
        assert gt.image_height == 1080
        assert len(gt.detections) == 1
        assert gt.detections[0].bbox.width == 1920
