---
title: Vision Pipeline
---

Python uv workspace at `vision/` that decomposes VM screenshots into
structured UI data. Each stage is a workspace member with its own
`pyproject.toml` and test suite, composed top-down by the pipeline
orchestrator.

## Stage contract

Every stage exposes a class with a `run()` method. The input is a
`PIL.Image` plus any upstream outputs; the output is a list of typed
detections defined in `testanyware_common.types`. Stages must be pure
functions of their inputs so the orchestrator can cache and parallelize.

```python
class Stage:
    def run(self, image: Image, upstream: Outputs) -> Outputs: ...
```

`Outputs` is a stage-specific dataclass (e.g. `WindowDetections`,
`ElementDetections`) imported from `testanyware_common.types`. No stage
imports another stage's implementation — only its output type.

## Workspace layout

Declared in `vision/pyproject.toml`:

```
vision/
├── common/                          # testanyware-common — shared types, utilities
├── pipeline/                        # orchestrator (currently a stub)
├── stages/
│   ├── window-detection/
│   │   ├── generator/               # synthetic-data generator
│   │   ├── training/                # detector training script + configs
│   │   └── analysis/                # runtime window-boundary detector
│   ├── drawing-primitives/          # low-level line/box/shape primitives
│   │                                # (absorbed from the Redraw project)
│   └── icon-classification/         # per-button icon classifier
│       ├── src/icon_classification/ # classifier + shape-heuristic fallback
│       ├── training/                # training workflow (Create ML)
│       └── data/                    # model artefacts (post-training)
```

## Stages

### `window-detection`

Three sub-packages that together own the "where are the windows" step.

- **generator** — produces synthetic training images and ground-truth
  labels (window rectangles + chrome regions). Used to bootstrap the
  detector without needing thousands of real screenshots.
- **training** — configs + scripts to train the detector from the
  generator's output or real labelled data.
- **analysis** — the runtime detector. Input: a screenshot. Output:
  window bounding boxes and chrome regions that downstream stages
  consume for layout context.

### `drawing-primitives`

Low-level geometric primitives (line, box, shape grouping) used by the
element and chrome stages. Absorbed from the standalone Redraw project.
Pure geometry — no model, no ML.

### `icon-classification`

Per-button icon classification against a fixed 52-label vocabulary
(gear, checkmark, close-x, chevrons, plus, minus, etc.). Given a
screenshot and a list of button-like detections from an upstream
stage, returns the best label (or `"unknown"`) for each.

**Status:** model not yet trained. The eventual CoreML/ONNX model has
not been produced, so classification currently falls back to the
shape-analysis heuristic at
`src/icon_classification/shape_analysis.py`. The heuristic handles ~8
obvious geometric icons (plus, minus, close-x, checkmark, four
chevrons); everything else comes back as `"unknown"`. Once a trained
model lands at `data/icon_classifier.onnx` or
`data/icon_classifier.mlmodelc`, the classifier will use it
automatically. See `vision/stages/icon-classification/training/README.md`
for the end-to-end training workflow (collect → label → Create ML →
bundle).

## Test organisation

Marker-driven (pytest markers declared in `vision/pyproject.toml`):

| Marker | Meaning |
|--------|---------|
| `unit` | Pure logic, no models or VMs |
| `vision` | Detector accuracy against golden datasets |
| `integration` | End-to-end against live VMs |
| `slow` | Takes more than 10 seconds |

Default invocation skips integration and slow tests:
```
cd vision && uv sync && uv run pytest
```

**Required flag:** pytest is invoked with `--import-mode=importlib`
(baked into the uv workspace config). This is necessary because several
workspace members share top-level package names and the default `prepend`
import mode causes duplicate-module collisions.

## How this composes

The pipeline orchestrator at `vision/pipeline/` (currently a stub)
will wire stages in topological order:

```
screenshot
  │
  ▼
window-detection/analysis ── chrome regions
  │
  ▼
(element detection stage — future)
  │
  ▼
icon-classification  ← uses button-like detections
  │
  ▼
drawing-primitives (geometric hints for all above)
```

Stages produce additive annotations on a shared `Detections` object;
downstream stages read earlier detections via dataclass fields, not
by re-running upstream work. This is the same composition model used
on the host-CLI side: each stage runs alone from the command line and
the pipeline is assembled at the outermost boundary.
