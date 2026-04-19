"""IconClassifier: combines the optional trained model with the shape heuristic."""

from __future__ import annotations

from pathlib import Path
from typing import Optional

from PIL import Image

from .shape_analysis import classify_by_shape


class IconClassifier:
    """Classify a cropped icon image.

    On construction, attempts to load an ONNX model from `model_path` (or the default
    bundled location `<stage>/data/icon_classifier.onnx`). If the file or the
    `onnxruntime` optional dependency is missing, the classifier falls back to the
    pure-Python shape-analysis heuristic.
    """

    def __init__(
        self,
        model_path: Optional[Path] = None,
        confidence_threshold: float = 0.5,
    ):
        # Default model location; file may or may not exist (model not trained yet).
        default = Path(__file__).parent.parent.parent / "data" / "icon_classifier.onnx"
        self.model_path = Path(model_path) if model_path else default
        self.confidence_threshold = confidence_threshold
        self._session = None
        if self.model_path.exists():
            try:
                import onnxruntime as ort

                self._session = ort.InferenceSession(str(self.model_path))
            except ImportError:
                # onnxruntime optional dep not installed — stay None, fall back to shape analysis
                pass

    def classify(self, image: Image.Image) -> tuple[str, float]:
        # 1) Try model (when available)
        if self._session is not None:
            # Placeholder for real inference; needs model input/output spec to fill in.
            # For now: coerce to 'unknown' since no model is trained.
            return ("unknown", 0.0)

        # 2) Fall back to shape-analysis heuristic
        label, conf = classify_by_shape(image)
        if label is not None and conf >= self.confidence_threshold:
            return (label, conf)

        return ("unknown", 0.0)
