from __future__ import annotations

from dataclasses import dataclass

from testanyware_common.types import BoundingBox, Detection, GroundTruth, GroundTruthSource


@dataclass
class WindowSpec:
    """Specification for a single window to create in the VM."""

    app_name: str
    title: str
    x: int
    y: int
    width: int
    height: int
    z_order: int

    def to_detection(self) -> Detection:
        return Detection(
            label="window",
            bbox=BoundingBox.from_xywh(self.x, self.y, self.width, self.height),
            confidence=1.0,
            metadata={
                "title": self.title,
                "app_name": self.app_name,
                "z_order": self.z_order,
            },
        )


@dataclass
class WindowScenario:
    """A complete scenario: a set of windows to create and screenshot."""

    name: str
    description: str
    screen_width: int
    screen_height: int
    windows: list[WindowSpec]

    def to_ground_truth(self, image_path: str) -> GroundTruth:
        return GroundTruth(
            stage="window-detection",
            image_path=image_path,
            image_width=self.screen_width,
            image_height=self.screen_height,
            detections=[w.to_detection() for w in self.windows],
            sources=[GroundTruthSource.PROGRAMMATIC],
        )
