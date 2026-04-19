"""Icon classification stage: classify cropped UI-button icons into a fixed vocabulary.

Uses an optional CoreML/ONNX model when available; falls back to a shape-analysis
heuristic for a small set of obvious geometric icons (plus, minus, close-x,
checkmark, chevrons).
"""
