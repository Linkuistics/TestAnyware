# text-ocr — EasyOCR daemon (`ocr_analyzer`)

The OCR engine behind the host CLI's `testanyware screen find-text` on
**Linux and Windows**. macOS uses in-process Apple Vision and never launches
this (ADR-0002). The Rust bridge
(`cli-rs/crates/testanyware-ocr-client/src/bridge.rs`) spawns
`python -m ocr_analyzer --daemon` and talks to it over stdin/stdout.

## Wire protocol (owned by the Rust bridge)

One JSON object per line, both directions:

| Direction | Line |
|-----------|------|
| daemon → host, once at startup | `{"ready": true}` |
| host → daemon, per request | `{"image_path": "/abs/frame.png"}` |
| daemon → host, per request | `{"detections": [{"text": "File", "bbox": [x0,y0,x1,y1], "confidence": 0.97}], "engine": "easyocr"}` |
| daemon → host, terminal failure | `{"error": "..."}` (bridge latches "permanently unavailable") |

`bbox` is axis-aligned `[x_min, y_min, x_max, y_max]`; the bridge derives
width/height by subtraction. The user-facing engine token (`easyocr_daemon`)
is set Rust-side — this module's `engine` field is ignored.

The daemon emits `{"ready": true}` *immediately* and builds the EasyOCR
`Reader` on a background thread, so the heavy torch import + model load
overlaps the host's RFB framebuffer capture and the first `readtext` lands
inside the bridge's 15 s first-call deadline.

## Provisioning recipe (Linux aarch64 HUT)

This is what `cli-rs/.../tests/linux-host-harness.rs` does at run time. The
HUT is the Linux golden `testanyware-golden-linux-24.04`, which the harness
starts through the TestAnyware app (`vm start --platform linux`). The harness
runs each step through the In-VM agent of the HUT, as `admin`, who has
passwordless `sudo`. The venv is never baked into an image. `<run>` is the
staging dir (e.g. `/home/admin/taw`):

```sh
# 1. venv tooling (stock Ubuntu 24.04 ships python3 but not the venv module)
sudo apt-get update -qq && sudo apt-get install -y -qq python3-venv

# 2. build the venv and install EasyOCR (pulls torch — large, slow on first run;
#    aarch64-linux CPU wheels exist on PyPI)
python3 -m venv <run>/venv
<run>/venv/bin/pip install --upgrade pip
<run>/venv/bin/pip install easyocr

# 3. drop the module beside the venv and pre-download the EasyOCR models
#    (Reader() fetches CRAFT detector + recognizer on first construction)
#    ocr_analyzer/ → <run>/ocr_analyzer/
<run>/venv/bin/python -c "import easyocr; easyocr.Reader(['en'], gpu=False)"

# 4. point the host CLI at the venv + module
TESTANYWARE_OCR_PYTHON=<run>/venv/bin/python \
PYTHONPATH=<run> \
  testanyware screen find-text File --vnc <gw>:<port> --json
```

The harness builds the venv again on each run and keeps no cache of it. The
Windows harness uses the same recipe and module through the same kind of
channel, the In-VM agent of its HUT.

`TESTANYWARE_OCR_PYTHON` is the documented interpreter override
(`docs/reference/env-vars.md`); without it the CLI's `resolve_interpreter()`
looks for an install-layout `<prefix>/libexec/venv/bin/python` or a dev-layout
`pipeline/.venv/bin/python` (`engine.rs`).

## Develop

```sh
cd vision
uv sync
uv run pytest stages/text-ocr            # pure helpers + protocol, no torch needed
```

The pure helpers and `serve` loop are torch-free and unit-tested with a fake
reader (`tests/test_daemon.py`); only `build_easyocr_reader` imports easyocr.
