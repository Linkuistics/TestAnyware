Key commands:
- pytest — run all tests (TDD: write tests first)
- Python primary, Swift only for Apple Vision OCR
- Start VMs: source scripts/macos/vm-start.sh
- Stop VMs: source scripts/macos/vm-stop.sh
- uv sync --all-packages — install all workspace dependencies

Constraints:
- TDD: write tests before implementation
- Each pipeline step is a sub-project with generator/trainer/analyzer
- JSON input/output between steps
- Python primary, Swift only for OCR
- Actually generate data, train models, and evaluate against real VMs — do not proceed with mocks only
- OCR accuracy research continues separately in LLM_STATE/ocr-accuracy/
