"""Icon classification vocabulary.

`VOCABULARY` is the 52-label list from the v1 `models.json` (icon_classifier.classes),
used verbatim so the eventual trained model's class indices align with the names here.

`SHAPE_ANALYSIS_LABELS` is the subset that the shape-analysis heuristic can emit.
"""

from __future__ import annotations

VOCABULARY: list[str] = [
    "arrow-down", "arrow-left", "arrow-right", "arrow-up",
    "battery", "bell", "bluetooth", "calendar", "camera",
    "checkmark", "chevron-down", "chevron-left", "chevron-right", "chevron-up",
    "close-x", "cloud", "document", "download", "edit-pencil",
    "ellipsis", "external-link", "eye", "eye-slash",
    "folder", "gear", "hamburger-menu", "heart", "home",
    "info-circle", "link", "lock", "magnifying-glass", "microphone",
    "minus", "pause", "person", "play", "plus",
    "question-circle", "refresh", "share", "skip-back", "skip-forward",
    "star", "stop-media", "trash", "unlock", "upload",
    "volume-off", "volume-up", "warning-triangle", "wifi",
]

SHAPE_ANALYSIS_LABELS: set[str] = {
    "plus",
    "minus",
    "close-x",
    "checkmark",
    "chevron-left",
    "chevron-right",
    "chevron-up",
    "chevron-down",
}
