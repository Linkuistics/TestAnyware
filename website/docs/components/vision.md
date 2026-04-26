---
title: Vision
---

# Component: `vision/` — Vision pipeline

Python `uv` workspace on the host that decomposes VM screenshots into
structured UI data. See
[`docs/architecture/vision-pipeline.md`](../architecture/vision-pipeline.md)
for the staging model and contract; this document focuses on the
module layout and maintainer workflow.

## Layout

```
vision/
├── pyproject.toml                    # workspace root; declares members
├── common/
│   ├── pyproject.toml                # testanyware-common — shared types
│   ├── src/testanyware_common/
│   └── tests/
├── pipeline/                         # orchestrator (currently stub)
│   ├── src/
│   └── tests/
├── stages/
│   ├── window-detection/
│   │   ├── generator/                # synthetic training-data generator
│   │   ├── training/                 # detector training + configs
│   │   └── analysis/                 # runtime detector
│   ├── drawing-primitives/           # geometric primitives (absorbed Redraw)
│   └── icon-classification/
│       ├── src/icon_classification/
│       ├── training/
│       │   ├── README.md             # end-to-end training workflow
│       │   └── collect-training-data.sh
│       └── data/                     # model artefacts (post-training)
├── swift/                            # Swift helpers (CoreML loader, etc.)
└── docs/
```

## Key files

| File | Role |
|------|------|
| `vision/pyproject.toml` | Workspace root; declares members and pytest markers (`unit | vision | integration | slow`). |
| `vision/common/src/testanyware_common/` | Shared dataclasses and utilities imported by all stages. |
| `vision/stages/<stage>/pyproject.toml` | Per-stage package metadata. |
| `vision/stages/icon-classification/src/icon_classification/shape_analysis.py` | Heuristic fallback active until the learned model ships. |
| `vision/stages/icon-classification/training/README.md` | Collect → label → Create ML → bundle walkthrough. |

## Build / test

```bash
cd vision

# Install dependencies for the whole workspace
uv sync

# Run tests (default marker filter skips integration and slow)
uv run pytest

# Run a specific marker
uv run pytest -m unit
uv run pytest -m vision
uv run pytest -m integration      # requires a live VM

# Lint / format
uv run ruff check
uv run ruff format
```

**Pytest is invoked with `--import-mode=importlib`** (baked into the
workspace config). This is required because several workspace members
share top-level package names and the default `prepend` import mode
causes duplicate-module collisions.

## Common pitfalls

- **Icon classifier has no model yet.** All icon classification goes
  through `shape_analysis.py`, which handles ~8 geometric icons and
  returns `"unknown"` for everything else. Treat icon labels as
  best-effort until a model lands at `data/icon_classifier.onnx` or
  `.mlmodelc`.
- **Stages are independent packages.** Running `pytest` from a stage
  subdirectory without `uv sync` at the workspace root will fail with
  import errors for `testanyware_common`. Always sync from `vision/`.
- **Match the pytest marker in your test.** Un-marked tests run by
  default; if you want a test to run only under `-m vision` or
  `-m integration`, mark it explicitly — otherwise it runs every
  time.
- **The pipeline orchestrator is a stub.** `pipeline/` exists to
  declare the import target but doesn't yet compose stages. Driving
  end-to-end composition currently means importing each stage
  explicitly in your script.
