"""Resolve an element query against a flat or nested element list.

Mirrors the query resolution logic from macOS (QueryResolver.swift) and
Windows (UiaQueryResolver.cs): filter by role, label, id, then disambiguate
by index.
"""

from __future__ import annotations

from testanyware_agent.models import ElementInfo


def resolve(
    elements: list[ElementInfo],
    role: str | None = None,
    label: str | None = None,
    element_id: str | None = None,
    index: int | None = None,
) -> tuple[str, ElementInfo | None, list[ElementInfo] | None]:
    """Resolve a query to a single element.

    Returns a tuple of (result_type, element, matches) where:
      - ("found", element, None) — single match
      - ("not_found", None, None) — no matches
      - ("multiple", None, matches) — ambiguous matches
    """
    flat = _flatten(elements)

    # Filter by role
    if role:
        flat = [e for e in flat if e.role == role]

    # Filter by label (substring, case-insensitive)
    if label:
        label_lower = label.lower()
        flat = [e for e in flat if e.label and label_lower in e.label.lower()]

    # Filter by id (exact match)
    if element_id:
        flat = [e for e in flat if e.id == element_id]

    if not flat:
        return ("not_found", None, None)

    # Disambiguate by index
    if index is not None:
        if 0 <= index < len(flat):
            return ("found", flat[index], None)
        return ("not_found", None, None)

    if len(flat) == 1:
        return ("found", flat[0], None)

    return ("multiple", None, flat[:10])


def _flatten(elements: list[ElementInfo]) -> list[ElementInfo]:
    """Flatten a nested element tree into a flat list."""
    result: list[ElementInfo] = []
    for element in elements:
        result.append(element)
        if element.children:
            result.extend(_flatten(element.children))
    return result
