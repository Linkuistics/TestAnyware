from __future__ import annotations

from dataclasses import dataclass, field


@dataclass
class ElementInfo:
    role: str
    enabled: bool
    focused: bool
    child_count: int
    showing: bool = True
    actions: list[str] = field(default_factory=list)
    label: str | None = None
    value: str | None = None
    description: str | None = None
    id: str | None = None
    position_x: float | None = None
    position_y: float | None = None
    size_width: float | None = None
    size_height: float | None = None
    platform_role: str | None = None
    children: list[ElementInfo] | None = None

    def to_dict(self) -> dict:
        d: dict = {"role": self.role, "enabled": self.enabled, "focused": self.focused,
                   "showing": self.showing}
        if self.label is not None:
            d["label"] = self.label
        if self.value is not None:
            d["value"] = self.value
        if self.description is not None:
            d["description"] = self.description
        if self.id is not None:
            d["id"] = self.id
        if self.position_x is not None:
            d["positionX"] = self.position_x
        if self.position_y is not None:
            d["positionY"] = self.position_y
        if self.size_width is not None:
            d["sizeWidth"] = self.size_width
        if self.size_height is not None:
            d["sizeHeight"] = self.size_height
        d["childCount"] = self.child_count
        d["actions"] = self.actions
        if self.platform_role is not None:
            d["platformRole"] = self.platform_role
        if self.children is not None:
            d["children"] = [c.to_dict() for c in self.children]
        return d


@dataclass
class WindowInfo:
    title: str | None
    window_type: str
    size_width: float
    size_height: float
    position_x: float
    position_y: float
    app_name: str
    focused: bool
    elements: list[ElementInfo] | None = None

    def to_dict(self) -> dict:
        d: dict = {}
        if self.title is not None:
            d["title"] = self.title
        d["windowType"] = self.window_type
        d["sizeWidth"] = self.size_width
        d["sizeHeight"] = self.size_height
        d["positionX"] = self.position_x
        d["positionY"] = self.position_y
        d["appName"] = self.app_name
        d["focused"] = self.focused
        if self.elements is not None:
            d["elements"] = [e.to_dict() for e in self.elements]
        return d
