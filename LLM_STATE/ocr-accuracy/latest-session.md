### Session 47 (2026-04-16T12:38:57Z) — OCR daemon implementation: plan tasks 1–5, 8–11

- **What was attempted**: Execute the first 8 of 15 tasks from the Session 46 implementation plan for the long-lived OCR analyzer daemon. The plan targets tasks 1–5 (core types, Vision engine lift, Python daemon mode, fake harness, OCRChildBridge) and tasks 8–11 (OCRStatusFile, server /ocr route, ServerClient.ocr(), FindTextCommand rewrite).

- **What worked**:
  - **Task 3 — Python `--daemon` mode** (`pipeline/ocr/src/ocr_analyzer/__main__.py`, +54 lines): long-lived stdin/stdout JSON loop with eager EasyOCR reader warmup on startup, `{"ready": true}` signal, error recovery on malformed JSON and bad image paths, clean EOF exit.
  - **6 daemon tests** added to `pipeline/ocr/tests/test_ocr_analyzer.py` under `TestDaemonMode` (marked `@pytest.mark.requires_easyocr`): startup ready signal, single request/response, bad image path recovery, malformed JSON recovery, EOF clean exit, two sequential requests.
  - **Tasks 1, 2, 5, 8** (new Swift OCR types): `OCRDetection` + `OCRResponse` wire types, `VisionOCREngine.recognize(pngData:)` lifting Vision.framework OCR out of `FindTextCommand` into the library, `OCRChildBridge` actor for lazy-spawn/temp-file-PNG/JSON-line protocol, `OCRStatusFile` for persistent degraded-mode tracking — all land as new untracked files under `cli/macos/Sources/GUIVisionVMDriver/OCR/` and `cli/macos/Tests/GUIVisionVMDriverTests/OCR/`.
  - **Task 4 — `fake-ocr-daemon.sh`** harness under `cli/macos/Tests/Resources/` with 8 behavior modes for hermetic bridge testing.
  - **Task 9 — server `/ocr` route** (`GUIVisionServer.swift`, +119 lines): `handleOCRRequest` body collection, `handleOCR` platform dispatch (macOS/nil → `VisionOCREngine` in-process; Linux/Windows → `OCRChildBridge`), `GUIVISION_OCR_FALLBACK=1` opt-in, interpreter resolution chain (`$GUIVISION_OCR_PYTHON` → Cellar-relative Homebrew → dev `.venv` → `/usr/bin/python3`), `OCRStatusFile.write` on permanent failure.
  - **Task 10 — `ServerClient.ocr(pngData:)`** (+9 lines): posts PNG body to `/ocr`, decodes `OCRResponse`.
  - **Task 11 — `FindTextCommand` rewrite** (`FindTextCommand.swift`, −49 lines net): removed all inline Vision OCR; now dispatches through `client.ocr()` and surfaces `response.warning` to stderr. CLI is now fully engine-agnostic.
  - **3 new server tests** in `GUIVisionServerTests.swift`: macOS spec uses Vision engine, nil-platform spec uses Vision engine, Linux spec uses bridge (via fake harness). **1 new wire protocol test** in `ServerClientTests.swift`: OCR request serialization.
  - **All tests pass**: Swift 117 tests, Pipeline 836 tests (19 deselected). Pre-existing `UnifiedRole` failure in a separate target is unrelated.

- **What didn't work / not yet done**: Tasks 6 (failure classification), 7 (interpreter resolution + deadlines), 12 (DoctorCommand), 13 (startup warning), 14 (Package.swift cleanup), 15 (Tier 5 integration + acceptance criteria verification) are all pending. These are primarily error-path hardening and the final integration tier; the happy path is fully functional.

- **What this suggests trying next**: Execute remaining plan tasks 6, 7, 12–15. Task 6 (failure classification refinement for `OCRChildBridge`) and Task 7 (interpreter resolution + process deadline enforcement) are the structural gaps that determine how robust the daemon is in production. Task 15 (end-to-end acceptance test against a live Linux/Windows VM) is the final validation gate.

- **Key learnings**:
  - The OCR dispatch boundary lives cleanly in `handleOCR` on the server side — the Swift CLI (`FindTextCommand`) is now genuinely engine-agnostic and shrank by 49 lines.
  - The fake-shell-script harness pattern (Task 4) enables hermetic OCR bridge tests without needing Python in CI; the 8 behavior modes (`ready_then_echo`, `no_ready`, `immediate_exit`, etc.) cover every failure class the bridge must handle.
  - The `GUIVISION_OCR_FALLBACK=1` env-var opt-in satisfies the spec's aggressive failure-surfacing policy (A3 class): hard-fail by default, opt-in fallback for operators who prefer degraded-but-functional over a hard error.
