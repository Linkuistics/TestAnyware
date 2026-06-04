"""The daemon loop and its pure helpers.

`serve` is written against an injected ``reader_factory`` and explicit
stdin/stdout streams so the protocol can be unit-tested with a fake reader,
with no torch/easyocr import. `__main__` wires in the real
``build_easyocr_reader``.
"""

from __future__ import annotations

import json
import threading
from collections.abc import Callable, Iterable
from typing import Any, Protocol, TextIO


class Reader(Protocol):
    """The slice of `easyocr.Reader` this daemon uses."""

    def readtext(self, image_path: str) -> list[tuple[Any, Any, Any]]: ...


# EasyOCR's `readtext` returns triples of (quad, text, confidence), where the
# quad is four [x, y] corner points (ints or numpy scalars).
ReadtextResult = Iterable[tuple[Any, Any, Any]]


def to_axis_aligned_bbox(points: Any) -> list[float]:
    """Collapse EasyOCR's four-corner quad to ``[x_min, y_min, x_max, y_max]``.

    The Rust bridge's four-number ``bbox`` form is exactly this axis-aligned
    box; it derives width/height by subtraction (`bridge.rs::parse_response`).
    Casts to `float` so numpy scalar coordinates serialise as plain JSON.
    """
    xs = [float(p[0]) for p in points]
    ys = [float(p[1]) for p in points]
    return [min(xs), min(ys), max(xs), max(ys)]


def detections_from_readtext(results: ReadtextResult) -> list[dict[str, Any]]:
    """Map EasyOCR ``readtext`` triples to the bridge's detection dicts."""
    detections: list[dict[str, Any]] = []
    for quad, text, confidence in results:
        detections.append(
            {
                "text": str(text),
                "bbox": to_axis_aligned_bbox(quad),
                "confidence": float(confidence),
            }
        )
    return detections


def _emit(stream: TextIO, obj: dict[str, Any]) -> None:
    """Write one compact JSON line and flush — the unit of the line protocol."""
    stream.write(json.dumps(obj))
    stream.write("\n")
    stream.flush()


class _ReaderHolder:
    """Builds the reader on a background thread so the heavy torch/easyocr
    import + model load overlaps the host's RFB framebuffer capture, keeping
    the first ``readtext`` inside the bridge's first-call deadline."""

    def __init__(self, factory: Callable[[], Reader]) -> None:
        self._factory = factory
        self._reader: Reader | None = None
        self._error: BaseException | None = None
        self._done = threading.Event()

    def start(self) -> None:
        threading.Thread(target=self._build, name="ocr-reader-warmup", daemon=True).start()

    def _build(self) -> None:
        try:
            self._reader = self._factory()
        except BaseException as exc:  # import/model failure is terminal
            self._error = exc
        finally:
            self._done.set()

    def get(self) -> Reader:
        """Block until the reader is built (or its build failed). The bridge's
        first-call deadline bounds this wait externally; no inner timeout."""
        self._done.wait()
        if self._error is not None:
            raise self._error
        assert self._reader is not None  # _done set with no error ⇒ built
        return self._reader


def serve(
    reader_factory: Callable[[], Reader],
    stdin: TextIO,
    stdout: TextIO,
) -> None:
    """Run the line protocol until stdin closes or a terminal error.

    Emits the ready handshake immediately (the reader warms in the
    background), then for each ``{"image_path": ...}`` request emits a
    detections line. A malformed request or an unavailable/failed reader
    emits a single ``{"error": ...}`` line and ends the loop — which the
    bridge latches as permanently unavailable.
    """
    holder = _ReaderHolder(reader_factory)
    holder.start()
    _emit(stdout, {"ready": True})

    for raw in stdin:
        line = raw.strip()
        if not line:
            continue
        try:
            request = json.loads(line)
            image_path = request["image_path"]
        except (json.JSONDecodeError, KeyError, TypeError) as exc:
            _emit(stdout, {"error": f"malformed request: {exc!r}"})
            return
        try:
            reader = holder.get()
        except BaseException as exc:  # noqa: BLE001 — surface any build failure
            _emit(stdout, {"error": f"easyocr unavailable: {exc!r}"})
            return
        try:
            results = reader.readtext(image_path)
        except Exception as exc:  # noqa: BLE001 — one bad frame is terminal here
            _emit(stdout, {"error": f"recognize failed: {exc!r}"})
            return
        _emit(stdout, {"detections": detections_from_readtext(results), "engine": "easyocr"})


def build_easyocr_reader() -> Reader:
    """Construct the real EasyOCR English reader, CPU-only (no GPU in the HUT).

    Imported lazily so `serve` and the pure helpers stay importable — and
    unit-testable — without torch/easyocr present.
    """
    import easyocr

    return easyocr.Reader(["en"], gpu=False, verbose=False)
