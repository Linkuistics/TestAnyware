# icon-classification

Per-button icon classification stage for the TestAnyware vision pipeline. Given a screenshot plus a list of element detections (from an upstream stage), classifies any button-like detection into a fixed 52-label icon vocabulary (gear, checkmark, close-x, chevrons, etc.). Non-button detections pass through with `icon_label=None`.

**Status:** pre-model-trained. The eventual CoreML/ONNX model has not been produced yet, so classification falls back to a shape-analysis heuristic (see `src/icon_classification/shape_analysis.py`) that handles ~8 obvious geometric icons; anything else is reported as `"unknown"`. Once a trained model lands at `data/icon_classifier.onnx` (or `.mlmodelc`), the classifier will use it automatically — see `training/README.md` for the end-to-end model-creation workflow.

This stage is designed to be composed into the top-level pipeline orchestrator in `../../pipeline/`, which is currently a stub (`testanyware_pipeline/__init__.py` only). When the orchestrator is filled in, it will construct an `IconClassificationStage` and call `stage.run(image, detections)` after the element-detection stage.
