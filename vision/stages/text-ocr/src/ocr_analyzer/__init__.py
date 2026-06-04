"""EasyOCR daemon for the host CLI's `screen find-text` on Linux/Windows.

The Rust host CLI (`testanyware-ocr-client`, ADR-0002) routes OCR through
this module on non-macOS hosts: it launches `python -m ocr_analyzer --daemon`
and speaks a one-JSON-object-per-line protocol over the child's stdin/stdout.
macOS uses in-process Apple Vision instead and never spawns this daemon.

The wire protocol is defined by the Rust bridge
(`cli-rs/crates/testanyware-ocr-client/src/bridge.rs`):

  * On startup the daemon writes ``{"ready": true}`` once.
  * For each request line ``{"image_path": "/abs/path.png"}`` it writes one
    response line ``{"detections": [{"text", "bbox": [x0,y0,x1,y1],
    "confidence"}], "engine": "easyocr"}``.
  * A ``{"error": "..."}`` response latches the bridge "permanently
    unavailable" (terminal); the daemon then exits.

The reported engine token surfaced to users (``easyocr_daemon``) is set on the
Rust side (`engine.rs::engine_name`), not here — the bridge ignores this
module's ``engine`` field. See ``README.md`` for the provisioning recipe.
"""

from .daemon import detections_from_readtext, serve, to_axis_aligned_bbox

__all__ = ["serve", "detections_from_readtext", "to_axis_aligned_bbox"]
