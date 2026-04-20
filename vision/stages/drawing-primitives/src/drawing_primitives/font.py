"""Font matching stub.

Real font matching (SSIM against a rendered reference catalogue) requires
the font-reference data from Redraw's ``training/fonts/`` along with the
SSIM pipeline in ``redraw/tier3/font_matcher.py``. Porting that is
deferred to a later milestone.

TODO(task-2f.2): port the SSIM font matcher from the original Redraw repo
(``redraw/python/redraw/tier3/font_matcher.py`` in pre-unification git
history) and vendor the Redraw font reference db under ``training/fonts/``.
"""

from __future__ import annotations

from PIL import Image


def match_font(img: Image.Image) -> dict:
    """Return a placeholder font descriptor.

    Args:
        img: Element image crop (unused until the real matcher lands).

    Returns:
        Dict with ``family`` (``"unknown"``) and ``size`` (``0``).
    """
    del img  # unused stub
    return {"family": "unknown", "size": 0}
