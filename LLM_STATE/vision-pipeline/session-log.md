# Session Log

### Session 1 (2026-04-08) — Project scaffold + common library
- Direct port from `guivision_common` to `pipeline_common` was clean — the original types are well-designed
- 49 tests across 7 test files, all passing
- `PipelineStepResult` kept intentionally simple: flat dataclass with `Any` output payload since each step produces different data structures (list of OCR detections, region tree, widget classification, etc.)
- `StepConfig` is a minimal frozen dataclass (step_name + version) — step-specific configs will extend it in their own packages
- uv workspace pattern works well: `uv sync --all-packages` needed to install workspace member dependencies (not just `uv sync`)
- The hatchling `packages = ["src/pipeline_common"]` pattern from the original project carries over unchanged

### Session 2 (2026-04-08) — Code review of Session 1
- Port is correct and complete — all logic, algorithms, and behavior are identical to originals (modulo namespace rename)
- 3 fixes applied: (1) restored `GroundTruthSource` inline comments documenting design intent for each enum member, (2) made `PipelineStepResult` frozen for immutability consistency with `StepConfig`, (3) added `StepConfig` round-trip JSON test
- 50 tests now (was 49), all passing
- `__init__.py` exports all 15 public symbols correctly
- Reviewer flagged missing `Path` import in test_image_io.py — false positive, original had dead imports (`tempfile`, `Path`), port correctly removed them
- Reviewer flagged `output: Any` serialization concern — intentional design choice, tests already document the JSON-native contract via `test_output_can_be_any_json_serializable_type`
- Design note for Session 26 (orchestrator): `PipelineStepResult` may need `status`/`error` fields for failure propagation, and upstream provenance tracking for step composition chains

### Session 3 (2026-04-08) — OCR analyzer — Swift CLI extraction
- Swift CLI extracted cleanly — `recognizeText()` from `FindTextCommand.swift` was self-contained, only needed Vision + CoreGraphics frameworks
- Binary naming convention established: `guivision-` prefix for all pipeline binaries (e.g., `guivision-ocr`). Designed for eventual homebrew bottle installation on PATH
- Binary lookup: PATH first (installed), then relative `.build/debug/` fallback (development)
- `PipelineStepResult.output` stores JSON-native dicts (not typed Detection objects) per the established convention. `parse_ocr_output()` provides typed access when needed
- Swift output format `{text, bounds: {x, y, width, height}}` converts to Detection format `{label, bbox: [x1, y1, x2, y2]}` — the Python wrapper handles this mapping
- 11 unit tests (mocked subprocess) + 8 integration tests (real Swift CLI on Pillow-generated images), all passing
- Apple Vision OCR works well on programmatically rendered text at 36px+ — good confidence scores (>0.9) on clean white/black images
- `python -m ocr_analyzer image.png` CLI works for standalone invocation
- Directory structure: `pipeline/ocr/swift/` for Swift CLI, `pipeline/ocr/src/ocr_analyzer/` for Python wrapper — slightly different from plan's `analyzer/swift/` but cleaner as a Python package

### Session 4 (2026-04-09) — Code review of Session 3
- Swift CLI is truly standalone: `Package.swift` depends only on `swift-argument-parser`, builds cleanly in isolation, no reference to main guivision package
- Coordinate conversion matches original `FindTextCommand.swift` exactly: `(1.0 - box.origin.y - box.height) * imageHeight`
- 4 fixes applied: (1) `int()` → `round()` for pixel coordinate conversion — avoids systematic truncation bias in downstream IoU calculations, (2) added 30s subprocess timeout to prevent hangs on pathological images, (3) extracted duplicated `_mock_subprocess_success` to module-level function, (4) fixed fragile unit test that relied on dev binary existing on disk — now uses `patch.object(Path, "exists", return_value=True)`
- 61 unit tests pass (50 common + 11 OCR), 10 integration tests pass separately
- Python CLI (`python -m ocr_analyzer`) verified working with correct `--help` output
- Output schema verified: Swift `{text, bounds: {x,y,w,h}, confidence}` → Python `{label, bbox: [x1,y1,x2,y2], confidence}` matches `Detection.to_dict()` format
- Design note: `OCRConfig` is a standalone frozen dataclass, not extending `StepConfig`. This is fine — `StepConfig` is for pipeline metadata, not step-specific configuration. Future steps should follow this pattern

### Session 5 (2026-04-09) — OCR generator — training data creation
- Generator lives as `ocr_generator` package alongside `ocr_analyzer` in the same `pipeline/ocr/` workspace member — hatchling `packages` list extended to `["src/ocr_analyzer", "src/ocr_generator"]`
- **Note**: The programmatic generator (Pillow-rendered text on solid backgrounds) was removed in Session 17 bug fixes — it tested a fundamentally different problem than real-world OCR and provided no signal about VM performance. Only VM-based generation remains
- CLI: `python -m ocr_generator --output-dir DIR --connect-json connect.json`
- Ground truth `image_path` uses relative paths (`samples/ocr_00042.png`) so output directories are portable

### Session 6 (2026-04-09) — OCR trainer/evaluator — accuracy benchmarking
- Evaluator lives as `ocr_evaluator` package alongside `ocr_analyzer` and `ocr_generator` — hatchling `packages` list now `["src/ocr_analyzer", "src/ocr_generator", "src/ocr_evaluator"]`
- Named `ocr_evaluator` not `ocr_trainer` since there's no model training for Apple Vision OCR — the "trainer" role is purely evaluation/benchmarking
- Evaluator consumes **pre-computed predictions** (`predictions/*.json`) rather than running the analyzer directly — this separates inference from evaluation, enables caching, and doesn't require the Swift CLI to be built
- Generator updated to populate `Detection.metadata` with rendering params (`font_size`, `font_name`, `font_color`, `background_color`) — needed for per-category breakdown
- Text metrics use `difflib.SequenceMatcher.ratio()` (LCS-based) rather than Levenshtein edit distance — no external dependency needed, and the ratio is more intuitive (proportion of matching characters vs total)
- Word accuracy counts ground truth words found in prediction (order-independent) — handles both extra and missing words gracefully
- Per-category breakdown currently by font size bucket only; contrast and font family bucketing deferred to Session 7 review (requires deciding bucket boundaries for contrast ratios)
- Confidence calibration uses 4 bins (0–0.5, 0.5–0.7, 0.7–0.9, 0.9–1.0) — empty bins are omitted from output
- Threshold gating: `ThresholdConfig` defaults to 0.0 for all thresholds (always passes unless configured), `EvaluationReport.passed` drives CLI exit code (1 on failure)
- 28 evaluator tests + 20 generator tests (including new metadata test) + 11 analyzer tests + 50 common tests = 109 total, all passing
- CLI: `python -m ocr_evaluator evaluate --data-dir DIR --output results.json [--min-char-accuracy N --min-word-accuracy N --min-detection-f1 N]`

### Session 7 (2026-04-09) — Code review of Sessions 5-6 + end-to-end OCR validation
- 3 bugs fixed: (1) `compute_text_metrics` word accuracy over-counted duplicates — `"Hello"` matched both GT words in `"Hello Hello"`, reporting 100% instead of 50%. Fixed by consuming matched prediction words (list.remove), (2) per-font-size bucketing appended the same `SampleResult` N times when a sample had N detections in the same bucket, inflating `sample_count`. Fixed by collecting unique buckets per sample, (3) `_match_predictions_to_ground_truth` in evaluator duplicated the greedy IoU matching logic from `compute_metrics` — extracted `match_detections()` to `pipeline_common.metrics` and removed the evaluator's copy
- `match_detections()` now exported from `pipeline_common` — returns `list[tuple[Detection, Detection]]` matched pairs. `compute_metrics` refactored to call it internally. 7 new tests for `match_detections`, 2 new tests for the bugs. 118 total tests, all passing
- End-to-end pipeline validated with VM screenshots: generate → analyze (0.5-0.7s/sample) → evaluate → report. VM results: 9.1% char accuracy due to granularity mismatch between line-level OCR and element-level accessibility ground truth
- **Note**: Historical programmatic results (80-100% char accuracy) came from the removed Pillow-based generator. These measured a different problem (rendered text on solid backgrounds) and don't reflect real-world performance
- Sub-project pattern is clean and replicable: `{step}/src/{step}_{role}/` with `__init__.py`, `__main__.py`, `{role}.py`. Single `pyproject.toml` per step with multiple hatchling packages. Phase 2 can follow this template directly
- Design note: evaluator consumes pre-computed predictions (inference/evaluation separation). The orchestrator (Session 26) will need a convenience wrapper to chain generate→analyze→evaluate without manual steps

### Session 8 (2026-04-09) — Geometric region decomposition
- Region decomposition follows the established sub-project pattern: `pipeline/region-decomposition/` with `src/region_analyzer/` package, registered as workspace member in root `pyproject.toml`
- Architecture: 4 focused modules — `region_types.py` (Region tree + config), `edge_detection.py` (Canny + contour extraction), `line_detection.py` (HoughLinesP divider detection), `containment_hierarchy.py` (tree building from flat rectangles), `analyzer.py` (orchestration)
- `Region` is a separate type from `Detection` because it needs a tree structure (children). Provides `to_detection()` and `flatten_to_detections()` for compatibility with pipeline_common metrics
- Containment hierarchy algorithm: sort rectangles by area descending, assign each to smallest containing parent. O(n²) is fine for UI regions (typically <100). Partial overlaps become siblings, not parent-child
- OpenCV returns numpy int32 from `boundingRect()` and `HoughLinesP()` — these are NOT JSON serializable. Must cast to native Python `int()` at the boundary where CV values enter our data structures
- Line detection picks up rectangle borders as divider lines, which splits the full image into many small edge regions. Session 9 review should evaluate: should line splits only be applied within detected contour regions instead of the full image?
- Deduplication needed at two levels: (1) `_deduplicate_rectangles` merges near-identical contour bounding boxes (within 5px), (2) `_deduplicate_lines` merges near-identical divider lines (within 5px position)
- Synthetic test images (via `tests/synthetic_images.py` helpers) provide reliable, deterministic test fixtures — colored rectangles on gray backgrounds detected consistently by Canny
- 56 region-decomposition tests + 118 existing = 174 total tests, all passing
- CLI: `python -m region_analyzer image.png [--output regions.json] [--min-width N --min-height N --canny-low N --canny-high N]`

### Session 9 (2026-04-09) — Code review of Session 8
- 4 fixes applied: (1) moved `from pipeline_common import BoundingBox` from inside `analyze_image()` to module-level import, (2) added `GaussianBlur` before Canny in `line_detection.py` to match `edge_detection.py` — reduces noise-induced false positive lines, (3) exposed `--min-line-length-ratio` and `--merge-distance` in CLI for full `RegionConfig` tunability, (4) replaced `NamedTemporaryFile(delete=False)` with pytest `tmp_path` fixture in `test_region_analyzer.py` — eliminates temp file leaks
- 174 unit tests pass (56 region-decomposition + 118 existing), 10 OCR integration tests deselected (require Swift CLI)
- Architecture is clean: 4 focused modules (edge_detection, line_detection, containment_hierarchy, region_types) + orchestrator (analyzer). Each module has a clear responsibility and clean public API
- Containment hierarchy correctly handles: nesting, sibling non-overlapping regions, partial overlaps (treated as siblings), smallest-parent assignment, order-independent input
- Design consideration deferred: line splits currently applied to full image bounds, causing rectangle borders to be detected as divider lines. This produces spurious line-split rectangles that overlap with contour-detected ones. The hierarchy builder handles this overlap gracefully, but applying splits only within detected contour regions would be more precise. Worth revisiting when evaluating on real screenshots in future sessions
- Real screenshot evaluation deferred to Session 10 — synthetic test images validate the algorithm correctness, but real UI screenshots (Xcode, VS Code, terminals) needed to assess practical accuracy and tune thresholds
- Performance not benchmarked on 4K — synthetic tests run in 0.37s total but real 4K screenshots will stress the Canny + HoughLinesP pipeline. Defer to when we have real test images

### Session 10 (2026-04-09) — Region decomposition generator + semantic classifier training
- Generator and evaluator follow the established pattern: `region_generator` and `region_evaluator` packages alongside `region_analyzer` in the same `pipeline/region-decomposition/` workspace member. Hatchling `packages` now `["src/region_analyzer", "src/region_generator", "src/region_evaluator"]`
- Pillow added as dependency for the generator (image rendering)
- 6 layout templates defined: `single_panel`, `sidebar_content`, `header_content_footer`, `ide_layout`, `tabbed_layout`, `dialog_overlay`. Each produces an RGB numpy array (via OpenCV drawing) + GroundTruth with semantic labels
- Semantic label taxonomy: `editor_pane`, `sidebar`, `tab_bar`, `status_bar`, `toolbar`, `content_area`, `dialog`, `panel`, `header`, `footer`. Labels embedded in ground truth Detection objects with `metadata={"layout_type": template.value}`
- VM-based generation deferred — requires guivision CLI + VM infrastructure. Programmatic generation covers algorithm correctness; real screenshots needed for practical accuracy assessment
- YOLO semantic classifier training deferred — current analyzer outputs `label="region"` for all detections (no semantic classification). Training requires labeled real screenshot data. The evaluator infrastructure is ready to measure classification accuracy when a classifier is added
- Evaluator consumes pre-computed predictions (same pattern as OCR evaluator). Flattens hierarchical region trees into flat Detection lists, filters out >95% image area root nodes, matches against ground truth using `match_detections()` from pipeline_common
- Metrics: per-sample mean IoU, detection precision/recall/F1, per-layout-type breakdown. ThresholdConfig gates pass/fail
- 36 new tests (20 generator + 16 evaluator) + 174 existing = 210 total tests, all passing
- CLIs: `python -m region_generator --output-dir DIR --count N [--seed S]`, `python -m region_evaluator evaluate --data-dir DIR --output results.json [--min-mean-iou N --min-detection-f1 N]`

### Session 11 (2026-04-09) — Code review of Session 10 + region decomposition end-to-end
- 2 review fixes applied: (1) corrected misleading BGR color comments in generator (e.g., "light blue" was actually light tan), (2) eliminated unnecessary Detection object re-creation — metadata now set directly in `_draw_region` instead of being patched afterward in `render_layout`
- End-to-end pipeline validated: generate 30 samples → analyze with region_analyzer → evaluate with region_evaluator → report
- **Key metrics**: mean IoU 0.964 (excellent spatial accuracy), recall 1.0 (no regions missed), precision 0.14 (many false positives), F1 0.245
- Low precision is expected and understood: the geometric analyzer finds ALL rectangular edges (contour boundaries + divider lines). For a 2-panel layout, it produces ~23 detections: 3 contour rectangles (correct) + 20 line-split rectangles (noise from rectangle borders being detected as divider lines)
- Disabling line splits (min_line_length_ratio=0.95) improved precision to 0.20 and F1 to 0.33, but contour detection still over-counts because Canny finds edges on both sides of each panel border
- Design conclusion: the geometric base layer correctly finds ALL edges with excellent spatial accuracy. Precision improvement requires semantic classification (YOLO) to distinguish meaningful UI regions from geometric noise — this is the purpose of the deferred classifier
- Training data diversity: 6 layout templates × 5 seeded variations = 30 samples. All semantic labels (sidebar, content_area, toolbar, status_bar, editor_pane, tab_bar, dialog) are represented. Generator determinism verified via seeded random
- Per-layout IoU: sidebar_content 0.98, single_panel 0.99, ide_layout 0.97, dialog_overlay 0.95, tabbed_layout 0.93, header_content_footer 0.92. Thin regions (tab_bar 35px, toolbar 40px, status_bar 30px) have lower IoU due to border thickness being a larger proportion of total region area
- Cross-platform testing deferred — no VM infrastructure in this session; synthetic images validate algorithm correctness
- Recursive refinement not tested — would require analyzing sub-regions of detected contour regions. The line-split noise issue suggests this could help: apply line detection within contour regions rather than the full image. Deferred to future refinement
- 210 total tests, all passing

### Session 12 (2026-04-09) — Widget detection baseline
- Widget detection sub-project created at `pipeline/widget-detection/` with `src/widget_analyzer/` package, registered as workspace member
- Redraw source code not available in this repo — heuristic classifier built from scratch using geometric and color features
- Widget type taxonomy: 19 types defined in `WidgetType` enum (button, text_field, checkbox, radio, toggle, slider, dropdown, tab, list_item, tree_item, menu_item, toolbar, scroll_bar, progress_bar, label, link, image, separator, unknown)
- State properties: 12 states in `WidgetState` enum (enabled/disabled, focused/unfocused, checked/unchecked, selected/unselected, expanded/collapsed, pressed/normal)
- Heuristic classifier architecture: extract visual features once (`_Features` class), then run priority-ordered scorer functions for each widget type. Each scorer returns a confidence and optional states. Highest confidence wins
- Features extracted: edge density, mean/std color, white ratio, colored ratio (HSV saturation), green ratio (for checkmarks/toggles), border strength (edge density in outer 20%), interior white ratio, circle presence (HoughCircles)
- Successfully classifies 8 widget types from synthetic images: button, checkbox (checked/unchecked), text_field (focused/unfocused), slider, separator, progress_bar, label, toggle (on/off)
- Two heuristic tuning issues resolved: (1) label vs text_field — added border_strength > 0.05 requirement for text_field (labels have no border), (2) toggle (on) vs button — small widgets with circles excluded from button scoring
- Synthetic widget images via `tests/synthetic_widgets.py`: OpenCV-drawn widgets with realistic proportions. Serves as ground truth for heuristic calibration
- `WidgetClassification` is a frozen dataclass with `to_dict()`/`from_dict()` round-trip serialization, matching the `Detection` pattern
- 32 new tests (19 analyzer + 13 types) + 210 existing = 242 total tests, all passing
- CLI: `python -m widget_analyzer image.png [--output result.json] [--min-confidence N]`
- Limitation: only 8 of 19 types tested — radio, dropdown, tab, list_item, tree_item, menu_item, toolbar, scroll_bar, image not yet implemented (would need more synthetic images or real screenshot data). The `UNKNOWN` fallback handles these gracefully

### Session 13 (2026-04-09) — Code review of Session 12
- 6 fixes from code review: (1) added `cv2.imread` null check — returns `None` for unreadable files, now raises `ValueError` with path, (2) added input shape guard — non-3-channel images (grayscale, RGBA) return `UNKNOWN` instead of crashing, (3) raised FOCUSED state threshold from 0.02 to 0.06 — was triggering on anti-aliasing noise, now requires visible colored border, (4) reduced progress_bar max height from 20 to 16 — eliminates overlap with slider at the h=20 boundary, (5) replaced mutable `list`/`dict` in frozen `WidgetClassification` with immutable `tuple` types — prevents mutation of ostensibly frozen dataclass and enables hashing, (6) removed unused `ocr_text` parameter from `classify_widget` and `analyze_widget` — dead API surface that misled callers
- Review also noted: border_strength computation uses 20% margins which gives a 55%+ border area for small widgets (24×24 checkbox), making interior_white_ratio unreliable for small images. This is a known limitation of the fixed-percentage approach — a future refinement could use absolute pixel margins (2-3px) for small widgets
- HSV hue range for green detection (35-85 in OpenCV half-scale) documented as covering yellow-green through cyan — potential false positives for teal/cyan accent themes in future real screenshot testing
- 242 total tests, all passing

### Session 13 (cont.) — Region decomposition algorithmic fix
- Two-part fix to region analyzer's line-split logic, addressing the precision problem flagged in Sessions 8/9/11:
  - (1) Line splits now applied within each contour rectangle, not the full image — eliminates spurious grid from rectangle borders being detected as divider lines
  - (2) Divider lines that coincide (within merge_distance) with any contour rectangle edge are filtered out — they are borders, not true dividers
- `_filter_border_lines()` added to analyzer.py: for each line, check if its position matches any rectangle's corresponding edge (horizontal line near y1/y2, vertical near x1/x2)
- End-to-end results on 30 synthetic samples: precision 0.14→0.86, recall 1.0→0.87, F1 0.25→0.86, IoU 0.96→0.97
- Per-layout: single_panel/sidebar_content/ide_layout now at F1=1.0; dialog_overlay F1=0.80 (dialog detected, content_area sometimes slightly off); tabbed_layout F1=0.67 and header_content_footer F1=0.80 (thin regions at 30-40px near min_region_height=30 threshold)
- Remaining recall gaps are in thin regions (tab_bar 35px, toolbar 40px, status_bar 30px) — these are at the edge of the min_region_height filter and would benefit from semantic classification
- 244 total tests (2 new analyzer tests), all passing

### Session 14 (2026-04-09) — Widget detection generator + model training
- Generator and evaluator follow the established pattern: `widget_generator` and `widget_evaluator` packages alongside `widget_analyzer` in the same `pipeline/widget-detection/` workspace member. Hatchling `packages` now `["src/widget_analyzer", "src/widget_generator", "src/widget_evaluator"]`
- Generator produces **individual widget crop images** (one widget per image), unlike OCR/region generators which produce full screenshots — this maps directly to how the analyzer works (classifies a single crop)
- 8 widget types rendered with parametric variation: button, checkbox, text_field, slider, separator, progress_bar, label, toggle. Each has size ranges (±50% around defaults), 5 background colors (including dark mode), and 7 accent colors
- State-pair generation: checkbox (checked/unchecked), toggle (checked/unchecked), text_field (focused/unfocused). Each state variant is a separate sample with visually distinct rendering — tests verified images differ between state pairs
- Weighted type distribution reflects real UI frequency: label 23%, button 20%, text_field 15%, checkbox 12%, toggle 12%, separator 8%, slider 5%, progress_bar 5%
- Ground truth uses Detection with `label=widget_type` and `metadata={"widget_type", "states", "width", "height", "bg_color", "accent_color"}` — enables per-category evaluation breakdown
- Evaluator measures **type accuracy** (classification correctness) and **state accuracy** (state detection correctness, only for state-applicable widgets). State accuracy excludes stateless widgets (separator, slider, label) from the denominator
- Confusion matrix tracks gt_type → pred_type counts — identifies systematic misclassification patterns (e.g., button↔label confusion)
- Confidence calibration uses same 4-bin structure as OCR evaluator — compares mean predicted confidence to actual accuracy per bin
- VM-based cross-platform generation and YOLO model training not completed in this session — only synthetic programmatic generation was implemented. These are scheduled for Session 21 which adds `--mode vm` support and YOLO training using the VM harness built in Session 15. The guivision CLI and agent infrastructure for VM-based generation already exists (screenshot, agent snapshot/press/set-value/focus, exec, input commands)
- 45 new tests (25 generator + 18 evaluator + 2 existing updated) = 287 total tests across all pipeline steps, all passing
- CLIs: `python -m widget_generator --output-dir DIR --count N [--seed S]`, `python -m widget_evaluator evaluate --data-dir DIR --output results.json [--min-type-accuracy N --min-state-accuracy N]`

### Session 15 (2026-04-09) — VM-based data generation harness
- VM harness implemented as 4 modules in `pipeline/common/src/pipeline_common/`: `vm_connection.py`, `vm_capture.py`, `accessibility_ground_truth.py`, `role_mapping.py`
- Added `--json` flag to `guivision agent snapshot` and `guivision agent inspect` CLI commands — outputs raw JSON (via JSONEncoder with prettyPrinted+sortedKeys) instead of AgentFormatter's human-readable text. Required because the formatted text drops positionX/Y coordinates needed for BoundingBox creation
- `VMConnection` wraps all major guivision CLI commands: screenshot, screen-size, agent snapshot/inspect/press/set-value/focus/health/wait, exec, input (click/type/key), find-text. Uses `--connect` flag with ConnectionSpec JSON file for all commands
- `VMCaptureSession` manages paired capture: screenshot + accessibility snapshot + screen_size in one operation. Writes 3 files per capture: image (samples/), ground truth JSON (ground_truth/), raw snapshot JSON (snapshots/). Auto-incrementing names with 5-digit zero-padding
- `AccessibilityGroundTruth` converts ElementInfo tree to Detection objects with stage-specific label mapping: "ocr" uses element label/value as text, "widget-detection" maps role→widget type, "region-decomposition" maps role→region semantic label. Unknown stages use raw role as label (forward-compatible)
- Role mapping covers 33 widget type mappings and 24 region label mappings from the 152 UnifiedRole values. Roles are organized into categories: `ROLE_TO_WIDGET_TYPE`, `ROLE_TO_REGION_LABEL`, `TEXT_CONTENT_ROLES` (for OCR), `STRUCTURAL_ROLES` (filtered out)
- Widget state extraction from accessibility properties: enabled/disabled, focused/unfocused from boolean fields; checked/unchecked for checkbox/switch/toggle-button, selected/unselected for radio, expanded/collapsed for disclosure-triangle/tree-item — derived from value field
- Float-to-int coordinate conversion uses `round()` (not `int()`) per the Session 4 fix — avoids systematic truncation bias in downstream IoU calculations
- Structural roles (none, presentation, generic, unknown, line-break, word-break, inline-text-box, text-run, ruby-annotation, list-marker) are filtered out from all ground truth — they are invisible/decorative elements
- 136 new tests (26 role mapping + 41 accessibility ground truth + 38 VM connection + 19 VM capture + 12 integration scaffolding) = 423 total tests across all pipeline steps, all passing
- All new modules exported from `pipeline_common.__init__` — 31 public symbols total (was 15)
- Integration test scaffolding at `test_vm_capture.py::TestIntegrationScaffolding` is marked `@pytest.mark.integration` and auto-skips when no `connect.json` or unhealthy agent. Ready for Sessions 17/19/21

### Session 16 (2026-04-09) — Code review of Session 15
- Role mapping coverage analysis: of 132 UnifiedRole values, 77 were mapped across 4 categories, 55 were unmapped. 26 important gaps found, ~27 correctly unmapped (structural containers, media elements, inline formatting, deprecated roles)
- 3 new widget type mappings added: `input-time`→`text_field`, `list-box`→`dropdown`, `menu-list-option`→`menu_item` — these are interactive elements analogous to already-mapped peers
- 13 new region label mappings added: `menu`→`panel`, `menu-list-popup`→`panel`, `search`→`content_area`, `radio-group`→`panel`, `list`→`panel`, `table`→`panel`, `tree`→`panel`, `tree-grid`→`panel`, `grid`→`panel`, `popover`→`panel`, `notification`→`dialog`, `toast`→`dialog`, `log`→`content_area` — critical gap was container roles whose children were already mapped (list, tree, table, menu)
- 10 new TEXT_CONTENT_ROLES entries: `menu-list-option`, `grid-cell`, `description-list-term`, `description-list-detail`, `note`, `definition`, `term`, `mark`, `time`, `timer`
- `VMConnection.inspect()` refactored to use `_query_args()` — was duplicating the same 5-condition arg-building logic already extracted as a static method
- Swift `--json` flag on snapshot/inspect commands: clean implementation, no issues. `prettyPrinted + sortedKeys` formatting ensures deterministic output for testing
- Harness API is sufficient for all three generator types (OCR, region, widget): VMConnection wraps all needed CLI commands, VMCaptureSession handles paired capture, AccessibilityGroundTruth handles stage-specific conversion. No API changes needed
- Error handling is adequate: VMConnectionError wraps binary-not-found, timeout, and non-zero exit. JSON parse errors from snapshot/inspect propagate as json.JSONDecodeError (intentional — distinct from CLI errors). Tests cover all error paths
- 441 total tests (was 423), all passing. 18 new tests: 16 role mapping tests + 3 accessibility ground truth tests for new mappings, minus 1 deselected integration test

### Session 17 (2026-04-10) — VM-based OCR generator + real screenshot evaluation
- VM-based OCR generator implemented as `ocr_generator.vm_generator` module — the only generation mode (programmatic generator removed)
- `AppScenario` dataclass defines what apps to launch: `app_name`, `launch_command`, `description`, `captures_per_scenario`, `wait_after_launch`, `app_ready_timeout`, `interactions`, `window_filter`. Four default macOS scenarios: TextEdit, Terminal, Safari, Finder
- `VMGeneratorConfig` takes `connect_json` path — total sample count determined by sum of `captures_per_scenario` across scenarios
- Generation workflow: create `VMConnection` → health check → for each scenario: `exec_command` (launch) → `_wait_for_app_ready` (poll until AX tree has elements) → capture + interact loop
- App launch failures (non-zero exit code) skip the scenario and continue — graceful degradation rather than aborting the entire run
- Sample names include app slug: `ocr_vm_textedit_00000` — readable and sortable
- Optional font metadata enrichment via `VMConnection.inspect()`: queries accessibility for `fontFamily`, `fontSize`, `fontWeight` per text element. Opt-in because it adds round-trips and may fail on some elements
- Font metadata enrichment handles `VMConnectionError` gracefully — detection is preserved without font metadata rather than failing the sample
- CLI: `python -m ocr_generator --output-dir DIR --connect-json connect.json [--enrich-font-metadata] [--guivision-binary PATH]`
- Evaluator extended with two deferred category breakdowns from Session 6:
  - **Contrast level bucketing**: uses WCAG 2.1 relative luminance formula (`relative_luminance()`) to compute contrast ratio between `font_color` and `background_color` from detection metadata. Three buckets: "low (<3:1)", "medium (3-7:1)", "high (>7:1)" aligned with WCAG AA/AAA thresholds
  - **Font family bucketing**: groups by `font_name` from detection metadata, provided via `--enrich-font-metadata`
- Both new evaluator categories follow the same pattern as `by_font_size`: collect unique buckets per sample, aggregate using `_aggregate_results()`. Detections without the required metadata fields are simply ungrouped (empty dict for that category)
- `EvaluationReport` extended with `by_contrast` and `by_font_family` fields, both serialize/deserialize via `to_dict()`
- 474 total tests (was 441), all passing. 33 new tests: 18 VM generator unit tests + 2 integration tests (deselected) + 17 evaluator tests (3 luminance + 5 contrast + 4 contrast breakdown + 5 font family breakdown). 13 total deselected integration tests
- **BUG FOUND during real VM testing**: VM-based generation produced 10 samples but ALL ground truth files contained only 3 Notification Center widget detections — no TextEdit/Terminal/Safari/System Settings content
- Root cause is NOT TCC — `AXIsProcessTrusted()` returns true, `/windows` endpoint correctly returns all 7 app windows, and `--mode all` snapshot shows full element trees. The actual problems:
  - **(1) Accessibility window registration lag**: apps appear visually within seconds but their AX windows don't register for 30-60+ seconds after `open -a`. The 3-second `time.sleep()` is grossly insufficient
  - **(2) No app-readiness verification**: `wait_for_agent()` only checks agent HTTP health, not whether the target app's windows are visible in the AX tree
  - **(3) No content diversity**: multiple captures of the same app produce identical screenshots because nothing changes between captures — need app interactions (type text, run commands, navigate)
  - **(4) Default snapshot mode="interact" filters out static-text elements**: OCR needs ALL text including non-interactive `static-text` labels, but the default `filterInteractive` removes them. Need `--mode all` or at minimum include text roles
- OCR Swift CLI (`guivision-ocr`) confirmed working on real screenshots: correctly detects menu bar text (TextEdit, File, Edit, Format, View, Window, Help), toolbar labels (Helvetica, Regular, 12), window title (Untitled) with confidence=1.0
- Fix plan written to `LLM_STATE/plan-fix-vm-ocr-generation.md` — covers readiness polling, interactions, depth/mode, and real end-to-end validation

### Session 17 Bug Fixes (2026-04-09)
- **All 4 root causes fixed** and validated with real VM generation:
  1. Replaced `time.sleep()` + `wait_for_agent()` with active readiness polling (`_wait_for_app_ready()`): polls `snapshot(window=app_name)` every 1s until elements appear, with configurable timeout (default 30s)
  2. Added `interactions` field to `AppScenario`: `type:` prefix for VNC text input, plain strings for SSH `exec_command`. Runs between captures for content diversity
  3. Now passes `depth=10, mode="all"` for all captures — catches deeply nested text and non-interactive `static-text` elements
  4. Captures now filter snapshot by `window=app_name` — ground truth is per-app, not all-windows-merged
- **Replaced System Settings** (doesn't appear in AX tree — SwiftUI app) with **Finder** in default scenarios
- `osascript` commands hang when executed via SSH (no desktop session) — use VNC `type_text()` or `open` commands instead
- Screenshot timeout increased from 30s to 60s (VNC handshake latency)
- **Real VM results**: 10 samples across 4 apps (TextEdit 23 dets, Terminal 5, Safari 25→172, Finder 56→131), 468 total ground truth detections
- **OCR evaluation on VM screenshots**: 9.1% char accuracy, 7.7% word accuracy, 6.6% detection F1
  - Dramatic degradation from programmatic baseline (80-100% char accuracy)
  - Root cause: **granularity mismatch** — Apple Vision OCR detects text lines ("TextEdit File Edit"), accessibility ground truth has individual elements ("File", "Edit" separately). IoU@0.5 rarely matches
  - Also: OCR reads visual text not in AX tree (rendered web content, icon labels), and AX tree has elements not visible to OCR (hidden/offscreen)
  - **Future improvement needed**: text-content-based matching instead of pure IoU for VM evaluation
- **Removed programmatic OCR generator** (`generator.py`, `test_ocr_generator.py`): rendered Pillow text on solid backgrounds — tested a fundamentally different problem than real-world OCR. VM mode is the only generation mode. Pillow moved from OCR runtime dependency to workspace dev dependency (still needed by integration tests)
- 470 total tests, all passing (was 474; -20 programmatic generator tests, +16 readiness/interaction tests)

### Session 17 End-to-End Validation (2026-04-10)
- AX visibility blocker resolved — all 4 default macOS app scenarios (TextEdit, Terminal, Safari, Finder) now produce ground truth
- **Text-content matching** implemented as a hybrid evaluation strategy:
  - `match_by_text_content()` does whole-word containment: GT "File" matches prediction "TextEdit File Edit" but "Edit" does NOT match "TextEdit" (split on whitespace, match whole words)
  - One prediction can match multiple GT elements (one-to-many) — handles OCR line-grouping naturally
  - IoU tiebreaker when multiple predictions contain the same GT text — picks best spatial overlap
  - Matched pairs score 1.0 char/word accuracy (containment verified); unmatched GT scores 0.0
  - `MatchingStrategy` enum (`IOU`, `TEXT_CONTENT`) on `EvaluatorConfig` and `evaluate_sample`, CLI `--matching-strategy text_content`
- **Subprocess pipe inheritance bug fixed** in `VMConnection._run`:
  - `subprocess.run(capture_output=True)` uses `communicate()` which waits for pipe EOF. The guivision `_server` daemon (300s idle timeout) inherits pipe FDs from its parent process and keeps them open → hangs forever
  - Fix: temp files + `Popen.wait()` instead of `communicate()`. `wait()` waits for process exit (not pipe EOF), temp files are seekable and don't block on children
  - Root cause is in guivision `_server` not closing inherited FDs when daemonizing — temp file workaround is reliable regardless
- **OCR analyzer invocation**: Python wrapper `python -m ocr_analyzer image.png` outputs `PipelineStepResult` envelope (dict with `output` key), not raw Detection list. Evaluator expects raw `list[Detection]` — must unwrap the `output` field. Future: evaluator should accept both formats, or orchestrator handles this
- **Real VM accuracy results** (10 samples across 4 apps, 459 GT detections, 292 OCR predictions):
  - IoU matching (baseline): 7.4% char, 5.9% word, 3.7% F1 — nearly useless due to granularity mismatch
  - Text-content matching: 17.1% char, 17.1% word, 20.9% F1 — 5.6× improvement in F1
  - Per-app: Finder best (41-49% F1 — file names word-match well), Terminal worst (5% F1 — only 5 AX elements, 32 OCR lines from terminal output)
  - Low recall explained: AX labels include non-visual elements (icon descriptions like "Arrow Down Circle", role names like "sidebar") that OCR never sees; OCR reads visual text not in AX tree (rendered web content, typed text)
  - The ~17% text-content recall represents the genuine overlap between "what OCR reads" and "what the AX tree labels"
- 488 total tests, all passing (was 470; +18 text-content matching tests in evaluator, updated 38 VM connection tests for Popen mock)
