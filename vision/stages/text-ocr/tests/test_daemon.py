"""Offline unit tests for the daemon's pure helpers and line protocol.

These exercise the bridge wire contract with a fake reader and in-memory
streams, so they run in a plain `uv run pytest` with no torch/easyocr present.
"""

from __future__ import annotations

import io
import json

import pytest
from ocr_analyzer.daemon import (
    detections_from_readtext,
    serve,
    to_axis_aligned_bbox,
)


def test_to_axis_aligned_bbox_collapses_quad_to_minmax():
    # EasyOCR quad: four corner points, here a tilted box.
    quad = [[10, 20], [42, 18], [44, 50], [12, 52]]
    assert to_axis_aligned_bbox(quad) == [10.0, 18.0, 44.0, 52.0]


def test_to_axis_aligned_bbox_casts_numpy_like_scalars_to_float():
    # numpy returns its own scalar types; the helper must yield plain floats so
    # json.dumps does not choke. Emulate with a tiny float subclass.
    class NpFloat(float):
        pass

    quad = [[NpFloat(1), NpFloat(2)], [NpFloat(9), NpFloat(2)],
            [NpFloat(9), NpFloat(8)], [NpFloat(1), NpFloat(8)]]
    box = to_axis_aligned_bbox(quad)
    assert box == [1.0, 2.0, 9.0, 8.0]
    assert all(type(v) is float for v in box)
    json.dumps(box)  # must not raise


def test_detections_from_readtext_maps_triples():
    results = [
        ([[0, 0], [30, 0], [30, 12], [0, 12]], "File", 0.97),
        ([[40, 0], [70, 0], [70, 12], [40, 12]], "Edit", 0.81),
    ]
    dets = detections_from_readtext(results)
    assert dets == [
        {"text": "File", "bbox": [0.0, 0.0, 30.0, 12.0], "confidence": 0.97},
        {"text": "Edit", "bbox": [40.0, 0.0, 70.0, 12.0], "confidence": 0.81},
    ]


class _FakeReader:
    def __init__(self, results):
        self._results = results
        self.calls: list[str] = []

    def readtext(self, image_path):
        self.calls.append(image_path)
        return self._results


def _run_serve(reader, lines):
    stdin = io.StringIO("".join(f"{line}\n" for line in lines))
    stdout = io.StringIO()
    serve(lambda: reader, stdin=stdin, stdout=stdout)
    return [json.loads(line) for line in stdout.getvalue().splitlines() if line]


def test_serve_handshakes_then_answers_a_request():
    reader = _FakeReader([([[0, 0], [30, 0], [30, 12], [0, 12]], "File", 0.9)])
    out = _run_serve(reader, [json.dumps({"image_path": "/tmp/a.png"})])
    assert out[0] == {"ready": True}
    assert out[1]["engine"] == "easyocr"
    assert out[1]["detections"][0]["text"] == "File"
    assert reader.calls == ["/tmp/a.png"]


def test_serve_skips_blank_lines():
    reader = _FakeReader([])
    out = _run_serve(reader, ["", "   ", json.dumps({"image_path": "/tmp/a.png"})])
    # Ready + exactly one detections line (blank lines produced nothing).
    assert out[0] == {"ready": True}
    assert len(out) == 2
    assert out[1]["detections"] == []


def test_serve_reports_malformed_request_as_terminal_error():
    reader = _FakeReader([])
    out = _run_serve(reader, ["not-json", json.dumps({"image_path": "/never.png"})])
    assert out[0] == {"ready": True}
    assert "error" in out[1]
    # The loop returns after the error, so the second (valid) line is ignored.
    assert len(out) == 2
    assert reader.calls == []


def test_serve_reports_missing_image_path_as_error():
    reader = _FakeReader([])
    out = _run_serve(reader, [json.dumps({"wrong_key": "x"})])
    assert "error" in out[1]


def test_serve_surfaces_reader_build_failure_as_error():
    def boom():
        raise ImportError("No module named 'easyocr'")

    stdin = io.StringIO(json.dumps({"image_path": "/tmp/a.png"}) + "\n")
    stdout = io.StringIO()
    serve(boom, stdin=stdin, stdout=stdout)
    out = [json.loads(line) for line in stdout.getvalue().splitlines() if line]
    assert out[0] == {"ready": True}  # handshake precedes the lazy build
    assert "error" in out[1]
    assert "easyocr unavailable" in out[1]["error"]


def test_serve_surfaces_readtext_failure_as_error():
    class _Exploding:
        def readtext(self, image_path):
            raise RuntimeError("bad frame")

    out = _run_serve(_Exploding(), [json.dumps({"image_path": "/tmp/a.png"})])
    assert "error" in out[1]
    assert "recognize failed" in out[1]["error"]


if __name__ == "__main__":
    raise SystemExit(pytest.main([__file__, "-q"]))
