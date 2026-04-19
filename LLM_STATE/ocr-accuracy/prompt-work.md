Commands:
- Kill any stale `testanyware _server` processes before VM operations.
- Start VM: `source scripts/macos/vm-start.sh` (append `--platform macos|linux|windows` to target a platform).
- Update `pipeline/connect.json` with VNC port, password, and agent IP from the startup output.
- Generate samples:
  `cd pipeline && uv run python -m ocr_generator --connect-json connect.json --output-dir data/ocr-vm --testanyware-binary ../cli/.build/debug/testanyware`
- Analyze a sample: `uv run python -m ocr_analyzer <image>`
- Evaluate against baseline with BOTH strategies:
  - `uv run python -m ocr_evaluator evaluate --data-dir data/ocr-vm --matching-strategy text_content --output data/ocr-vm/results-text.json`
  - `uv run python -m ocr_evaluator evaluate --data-dir data/ocr-vm --matching-strategy iou --output data/ocr-vm/results-iou.json`
- Stop VM: `source scripts/macos/vm-stop.sh`
- Tests: `uv run pytest --ignore=ocr/swift -x` (run before and after changes; expect 540+ pipeline tests to pass).

Constraints:
- TDD: write tests before implementation.
- OCR analyzer outputs `PipelineStepResult` envelopes — unwrap the `output` field.
- Always evaluate and report both IoU and text-content matching; compare per-app and aggregate against baseline.
- Evaluate on all three platforms; the generator auto-detects platform from `connect.json`'s `platform` field.
