# Vision Pipeline

Build a composable vision pipeline in `pipeline/` within TestAnyware. The pipeline
extracts structured, machine-precise visual data from GUI screenshots — serving as an
evaluation function for LLM-driven GUI development. Each pipeline step is an independent
sub-project with generator/trainer/analyzer CLIs communicating via JSON. Python primary,
Swift only for Apple Vision OCR.

## Task Backlog

### Code review of VM-based OCR generation

**Category:** `ocr`
**Status:** `not_started`
**Dependencies:** none

**Description:**

**Dependency context (all upstream work done; this task is fully unblocked):**
ocr-accuracy plan's multi-word GT matching (done, Session 28), center-distance
spatial matching (done, Session 29), Linux AT-SPI coordinate fix (done,
Session 30), A/B evaluation snapshots (done, Session 31), menu-item AX
filter (done, Session 32), button/textfield AX filter (done, Session 33),
macOS command-output GT exclusion (done, Session 34), `by_app` evaluator
bucket (done, Session 35), cross-platform command-output GT exclusion (done,
Session 37), single-char non-alphanumeric matcher fix (done, Session 38),
GTK4 per-element AT-SPI diagnosis (done, Session 39), **alternative-OCR-engine
survey (done, Session 40)**, **EasyOCR adopted as default OCR engine and
cached baselines refreshed (done, Session 41, 2026-04-13)**. The
previously-blocking engine survey landed with a clear verdict: EasyOCR
wins aggregate text-F1 on every platform by +9 to +20pp over Apple
Vision, the dense-monospace gap is Apple-Vision-specific (not
OCR-task-bound), and `OCRConfig` now dispatches among
`apple_vision`/`tesseract`/`easyocr` via schema 0.2.0. Canonical
`data/ocr-vm-{platform}/predictions/` dirs now hold EasyOCR output, the prior
Apple Vision predictions are preserved under `predictions-apple-vision/` for
any per-engine A/B, and the "Current Baseline" table in the ocr-accuracy plan
is the Session 41 EasyOCR baseline. Session 41 also discovered that EasyOCR
warm inference is **much faster than expected** on small frames
(0.79s/sample Windows, 1.44s/sample Linux, 3.92s/sample macOS Retina) —
the offline-vs-interactive runtime framing in memory applies mostly to
macOS Retina-2x captures, not to Linux/Windows 800×600 captures. Soft
dependency on ocr-accuracy "Drop GTK4 spatial GT (denominator-correctness
stop-gap)" — Linux IoU numbers are noisier than they should be until that
ships, since GTK4 elements currently sit in the IoU denominator with
`(0,0)` per-element positions that no engine can match.

Review VM-generated OCR data quality across all three platforms.
Compare synthetic vs VM accuracy results. Text-content matching (multi-word adjacent,
fuzzy) and center-distance spatial matching (prediction center inside GT box) are
implemented; Linux spatial metrics are no longer broken after the AT-SPI coordinate
fix (Session 30: Linux IoU F1 1.9% → 9.8%, with a known GTK4 empty-title limitation
tracked in the ocr-accuracy plan). A/B snapshot infrastructure (`pipeline_common/
data_snapshot`) is available for honest cross-version evaluation — use a frozen
snapshot for this review rather than a fresh VM run so the results are reproducible.
Current spatial F1 as of Session 30: macOS ~25%, Windows ~34% (with content-variation
caveats), Linux 9.8%. Text-content F1 is still on the stale Session 23 baseline and
will be revised by the ocr-accuracy plan's baseline refresh task. Session 35's
`by_app` bucket exposed a 60pp per-app text-F1 spread (Linux terminal 1.42% →
Linux Nautilus 61.68%) hidden by aggregate numbers; any VM-OCR review must
read per-app, not aggregate. Session 40's engine survey produced a
concrete EasyOCR-default per-platform aggregate (macOS 38.68%, Linux-fix
61.57%, Windows 24.73%) and headline per-app numbers in the ocr-accuracy
memory; use those as the reference baseline for this review rather than
the Apple Vision Session 23 numbers. Specific data-generation questions
the review should answer in the EasyOCR world: (a) does EasyOCR's
improved recognition expose new GT pipeline gaps that Apple Vision's
weaker recognition was masking (i.e. samples where EasyOCR reads text
the GT pipeline doesn't have)? (b) which content classes still
bottleneck on GT coordinate problems vs engine quality (memory's
"Engine choice dominates text-content F1; GT coordinate problems
dominate IoU F1" entry says this is a clean separation — confirm)?
(c) is the Windows windowsterminal focus bug the same shape under
EasyOCR, or does the engine swap reveal a different failure mode?

**Results:** _pending_

---

### VM-based region generator + YOLO semantic classifier

**Category:** `region-decomposition`
**Status:** `not_started`
**Dependencies:** none

**Description:**

**Cross-plan heads-up from ocr-accuracy Session 39:** GTK4 CSD apps (Nautilus,
GNOME Text Editor, any GTK4 app on Linux) return `(0, 0, w, h)` from AT-SPI
`getExtents` for *every descendant element* under both `DESKTOP_COORDS` and
`WINDOW_COORDS`. Per-element widths/heights ARE populated, but positions are
uniformly zero. The window-level offset fix shipped in Session 39
(`agents/linux/guivision_agent/accessibility.py`) gives the top-level frame a
correct screen position, but every child stacks at the window origin.
Implication for this task: **any VM-based Linux GT that uses AT-SPI as
spatial ground truth will produce degenerate labels for GTK4 apps.** Either
(a) filter GTK4 apps out of the initial Linux scenario list and rely on
GTK3/Qt/Electron apps (gnome-terminal, Firefox via AXWebArea, VS Code via
AXWebArea) for Linux coverage, or (b) await the ocr-accuracy plan's "GTK4
per-element position recovery" task verdict (OCR-driven registration vs.
drop-GTK4-spatial-GT vs. per-element grab-focus). Option (a) unblocks this
task today.

Extend region generator with VM support: launch multi-panel apps
(Xcode, VS Code, terminals with splits), capture screenshots, use accessibility
layout snapshots for ground truth. Map accessibility roles to semantic labels
(window→content_area, toolbar→toolbar, etc.). Train YOLO semantic classifier.
Evaluate on real screenshots across all three platforms. Test recursive refinement
(does re-running on sub-regions help?).

**Results:** _pending_

---

### Code review of region generator + semantic classifier

**Category:** `region-decomposition`
**Status:** `not_started`
**Dependencies:** VM-based region generator + YOLO semantic classifier

**Description:**

Review YOLO model accuracy. Compare per-layout performance: synthetic vs real.
Cross-platform accuracy comparison.

**Results:** _pending_

---

### VM-based widget generator + remaining widget types + YOLO classifier

**Category:** `widget-detection`
**Status:** `not_started`
**Dependencies:** none

**Description:**

**Cross-plan heads-up (same GTK4 per-element AT-SPI caveat as the region
generator task above):** GTK4 CSD apps produce degenerate per-element
positions through AT-SPI on Linux (Session 39 ocr-accuracy diagnosis). Any
widget GT that uses AT-SPI element bounding boxes as supervision on Linux
must either filter GTK4 apps out of the scenario list or await the
ocr-accuracy GTK4 per-element recovery verdict. GTK3 (gnome-terminal), Qt,
and Electron/WebKit apps (Firefox, Chromium, VS Code) are unaffected.

Extend widget generator with VM support. Implement remaining 11 widget types
in heuristic analyzer (radio, dropdown, tab, list_item, tree_item, menu_item,
toolbar, scroll_bar, link, image). Fix widget analyzer refinements (absolute
pixel margins, narrow HSV green range). Train per-platform YOLO models.
Research: single model vs per-platform models.

**Results:** _pending_

---

### Code review of widget generator + end-to-end

**Category:** `widget-detection`
**Status:** `not_started`
**Dependencies:** VM-based widget generator + remaining widget types + YOLO classifier

**Description:**

Cross-platform widget classification accuracy comparison. Heuristic vs YOLO
accuracy comparison on real screenshots. State detection accuracy review.

**Results:** _pending_

---

### Visual properties — port from Redraw

**Category:** `visual-properties`
**Status:** `not_started`
**Dependencies:** none

**Description:**

Create `pipeline/visual-properties/` sub-project. Port Redraw's Tier 3 code:
color extraction (`extract_fill_color`, `detect_gradient`), border detection
(`detect_border`, `detect_border_radius`), shadow detection
(`detect_shadow`). Adapt from PIL Image + list bounds to pipeline's types.
Benchmark accuracy on real UI element crops from VMs.

**Results:** _pending_

---

### Font detection

**Category:** `visual-properties`
**Status:** `not_started`
**Dependencies:** none

**Description:**

Create `pipeline/font-detection/` sub-project. Port Redraw's `font_matcher.py`
(SSIM-based font matching). Build font reference database for system fonts per
platform (macOS: SF Pro/Mono, Menlo; Windows: Segoe UI, Consolas; Linux:
Cantarell, Ubuntu, Noto Sans). Generator renders known text in known fonts
using platform-native rendering in-VM. Analyzer outputs font family, weight,
size, style. Use `testanyware agent inspect` font metadata as ground truth.

**Results:** _pending_

---

### Code review of visual properties + font detection

**Category:** `visual-properties`
**Status:** `not_started`
**Dependencies:** Visual properties — port from Redraw, Font detection

**Description:**

Validate visual property extraction and font detection accuracy on real UI
elements from VMs across platforms. Document minimum reliable font size for
family identification.

**Results:** _pending_

---

### Icon classification

**Category:** `icon-classification`
**Status:** `not_started`
**Dependencies:** none

**Description:**

Create `pipeline/icon-classification/` sub-project. Define icon taxonomy
(close, minimize, maximize, add, remove, settings, search, menu, chevron,
etc.). Generator: programmatic SVG rendering + VM-based real icon capture.
Train CNN or YOLO-cls classifier. Add color-state detection (icon color →
semantic state).

**Results:** _pending_

---

### Layout analysis

**Category:** `layout-analysis`
**Status:** `not_started`
**Dependencies:** none

**Description:**

Create `pipeline/layout-analysis/` sub-project. Primarily algorithmic: spacing
measurement, alignment detection, grid conformance testing, distribution
analysis, element grouping by proximity. Generator: programmatic known
layouts + VM-based web pages with known CSS. Test with synthetic element
arrangements.

**Results:** _pending_

---

### WebView connector

**Category:** `integration`
**Status:** `not_started`
**Dependencies:** none

**Description:**

Create `pipeline/webview-connector/` sub-project (does NOT follow
generator/trainer/analyzer pattern — it's a discovery/connector tool). CDP
discovery for Electron/CEF apps. Accessibility-based WebView detection
(`AXWebArea` on macOS, UIA WebView pattern on Windows). App-profile-based
WebView location.

**Results:** _pending_

---

### Deterministic VM evaluation scenarios

**Category:** `infrastructure`
**Status:** `not_started`
**Dependencies:** none

**Description:**

Cross-plan learning from ocr-accuracy: VM evaluation data is not deterministic
across runs. Windows is particularly unreliable — Explorer content varies
between runs, making cross-session metric deltas meaningless without
controlled content. macOS and Linux are more stable because scenario scripts
type specific text and open specific folders, but sidebar content still
varies. All VM-based pipeline evaluation tasks (region generator, widget
generator, etc.) will face this same problem. Design controlled scenarios
that produce deterministic content: fixed folder contents, specific text
typed into editors, known window layouts. This enables reliable cross-session
metric comparison for all pipeline steps.

**Results:** _pending_

---

### Pipeline orchestrator

**Category:** `integration`
**Status:** `not_started`
**Dependencies:** none

**Description:**

Build orchestrator that composes steps: sequential composition, selective
composition, recursive/iterative flows. CLI:
`python -m pipeline_orchestrator image.png --steps ocr,regions,widgets,visual-props`.
Full integration tests on screenshots from all three platforms. Benchmark
suite: timing per step, end-to-end latency, accuracy per step.

**Cross-plan note from ocr-accuracy Sessions 40 + 41:** the alternative-engine
survey landed with a concrete verdict — EasyOCR **is now the default**
(Session 41 shipped `OCRConfig().engine == "easyocr"`) after winning +9 to
+20pp aggregate text-F1 over Apple Vision on every platform. Apple Vision
remains available as an explicit `OCRConfig(engine="apple_vision")` choice;
Tesseract is the runtime-cheap middle path on dense monospace. The
orchestrator's OCR step should: (a) default to EasyOCR (matching the analyzer
default, no override needed for offline/batch runs), (b) accept an `engine`
override for cases where runtime cost matters (Session 41 warm timing data:
macOS Retina samples are ~4× Linux/Windows cost, so runtime pressure is
macOS-specific), (c) emit `Detection` objects annotated with the engine that
produced them so downstream consumers can A/B engine choices, and (d) be
agnostic to whether the OCR step is a single-engine wrapper or the eventual
multi-engine router (ocr-accuracy's router task is **demoted to low
post-Session-41** — the runtime argument for routing collapsed on
Linux/Windows; the router now survives only as a quality-motivated
per-content-class preprocessing dispatcher, not a cross-engine
runtime-balancer). Existing cached datasets must remain A/B-able under both
single-engine and router configurations, using the `predictions-{engine}/`
backup pattern Session 41 established.

**Cross-plan update from ocr-accuracy Session 43:** upscale-4x is universally
dead as a preprocessing lever — no cell hit >=5pp F1 lift on any engine, with
14-21x runtime. The orchestrator should never offer upscale-4x as a default
and should treat it as deprecated. The only SHIP-grade preprocess cell is
Linux terminal EasyOCR upscale-2x (+7.25pp F1 at 16x runtime ~24.5s/sample).
Per-content-class routing does NOT require a pixel-level content detector —
the `by_app` filename convention (`extract_app_from_sample_name()`) suffices
for dispatching the single viable preprocess override. Safari
"high-resolution rescore" hypothesis was falsified (Apple Vision Safari
recall/char bit-flat across all cutoffs and preprocess modes). The
recognition-bound hypothesis is reinforced — remaining dense-monospace gap is
not closable by any single-axis lever tested (engine, preprocess, line
extraction).

**Cross-plan update from ocr-accuracy Session 44:** EasyOCR subprocess
cold-start is 4.8–5.5s per call (PyTorch import + Reader construction),
making per-call subprocess dispatch non-viable for interactive use. The
orchestrator's interactive/CLI mode must use a long-lived OCR analyzer daemon
(keeping `_easyocr_reader_cache` warm) to deliver 0.79–3.92s warm inference
instead of 4.8–5.5s cold-start per call. Offline/batch mode is unaffected
(amortizes Reader init across samples).

**Results:** _pending_

---
