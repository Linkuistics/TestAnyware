# vision/ — Vision pipeline (host-side)

Python `uv` workspace that decomposes VM screenshots into structured
UI data. Each stage is a workspace member with its own
`pyproject.toml`; the pipeline orchestrator composes them
top-down.

## Working on this component

```bash
cd vision
uv sync                          # install workspace dependencies
uv run pytest                    # unit + vision tests (skips integration/slow)
uv run pytest -m integration     # needs a live VM
uv run ruff check                # lint
```

Pytest is invoked with `--import-mode=importlib` (baked into the
workspace config) — several workspace members share top-level package
names and the default `prepend` import mode causes duplicate-module
collisions.

## Notes

- The icon classifier has **no trained model yet**. All icon
  classification currently uses the shape-heuristic fallback at
  `stages/icon-classification/src/icon_classification/shape_analysis.py`,
  which handles about eight geometric icons.
- The pipeline orchestrator at `pipeline/` is a stub — composition is
  currently driven by callers importing stages directly.
- Stage architecture is documented at
  [`docs/architecture/vision-pipeline.md`](../docs/architecture/vision-pipeline.md).

See [`docs/components/vision.md`](../docs/components/vision.md) for
module layout, key files, and common pitfalls.
