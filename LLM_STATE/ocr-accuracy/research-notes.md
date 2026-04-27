# OCR Accuracy Research — Notes

Supporting context for the task backlog (`backlog.md`): current baselines,
per-app F1 tables, and a running list of remaining structural gaps. Updated as
new sessions refresh the numbers.

## Current Baseline

Established 2026-04-13 (Session 41) on the cached canonical datasets
`data/ocr-vm-{macos,linux-fix,windows}/`. **Default engine: EasyOCR**
(`OCRConfig().engine == "easyocr"`, `preprocess="none"`, `min_confidence=0.5`).
Same-evaluator-code A/B against the backed-up Apple Vision predictions
(`predictions-apple-vision/`) included for delta context. Replaces the stale
Session 23 Apple-Vision-only table.

### Aggregate (cached A/B, EasyOCR vs pre-flip Apple Vision)

| Platform | Strategy | AV F1 | EasyOCR F1 | Δ F1 | EO Precision | EO Recall | EO Char |
|---|---|---|---|---|---|---|---|
| macOS (10 samples) | text_content | 19.00% | 38.68% | **+19.68pp** | 31.71% | 49.58% | 56.07% |
| macOS | iou | 25.89% | 47.32% | **+21.42pp** | 39.17% | 59.75% | 51.05% |
| Linux (10 samples) | text_content | 45.52% | 61.57% | **+16.05pp** | 63.56% | 59.70% | 41.19% |
| Linux | iou | 8.78% | 9.80% | +1.02pp | 13.79% | 7.60% | 13.07% |
| Windows (8 samples) | text_content | 15.24% | 24.73% | **+9.48pp** | 33.76% | 19.51% | 22.73% |
| Windows | iou | 28.30% | 38.73% | **+10.43pp** | 59.79% | 28.64% | 24.81% |

text-F1 deltas land squarely in the predicted +9 to +20pp band on every platform.

### Per-app text-F1 (cached A/B)

| Platform | App | AV F1 | EasyOCR F1 | Δ F1 | Note |
|---|---|---|---|---|---|
| macOS | finder | 42.23% | 53.54% | +11.30pp | |
| macOS | safari | 19.42% | 40.63% | **+21.21pp** | proportional content lift |
| macOS | terminal | 7.38% | 19.69% | **+12.30pp** | dense monospace; +7pp more available with upscale-2x |
| macOS | textedit | 27.55% | 58.43% | **+30.88pp** | biggest macOS lift; AV ceiling broken |
| Linux | firefox | 8.85% | 15.09% | +6.24pp | low-absolute; sample inspection follow-up |
| Linux | nautilus | 68.78% | 73.63% | +4.85pp | text-only; IoU still GTK4-bound |
| Linux | terminal | 8.22% | 26.09% | **+17.87pp** | upscale-2x reaches 33.33% |
| Linux | texteditor | 20.56% | 35.29% | +14.73pp | |
| Windows | explorer | 23.14% | 27.91% | +4.77pp | |
| Windows | notepad | 14.55% | 42.48% | **+27.93pp** | proportional editor; matches macOS textedit pattern |
| Windows | windowsterminal | 1.50% | 1.44% | **−0.06pp** | bit-flat — generator focus bug caps this bucket; cached predictions show Notepad content for both engines |

## Key Remaining Gaps

- **Linux IoU stays pinned at ~9.8%** for every engine — confirms Session 40's
  forecast that engine choice cannot move spatial F1 off the GTK4 per-element
  zero-coordinate noise floor. Closing this needs the GTK4 stop-gap or
  per-element position recovery. Independent of the engine swap.
- **Windows windowsterminal bit-flat** — generator focus bug.
  Engine-orthogonal: cached `windowsterminal_*` predictions contain Notepad
  text for both engines because the screenshot capture happens before Windows
  Terminal is raised. Until the focus bug closes, the windowsterminal bucket is
  uninterpretable.
- **char-accuracy regressions on three buckets** with simultaneous F1 gains
  (macOS textedit −2.78pp, Linux firefox −5.56pp, Linux texteditor −9.52pp):
  EasyOCR's word segmentation splits differently than Apple Vision so per-pair
  string-similarity drops on shared matches even though more pairs match
  overall. F1 is the right headline; reporting char in isolation would have
  called this a regression. **Verdict-signature rule**: F1 + precision + recall
  is the canonical text-content verdict shape; char/word are content-class
  diagnostics, not verdict drivers.
- **Surprise: Windows IoU +10.43pp** broke memory's "engine choice moves IoU
  by <5pp" rule. Apple Vision was scoring near-zero char on its Windows IoU
  matches (5.78% char) because predictions were near-empty/garbled strings
  whose centres happened to land near GT bbox centres; EasyOCR's Windows IoU
  char jumped to 24.81%. The "GT coordinate problems dominate IoU" rule still
  holds for Linux (GTK4-bound, +1.02pp); Windows is a counter-example because
  the UIA oversized-bbox bug is *less* of a center-distance contaminator than
  expected.
- **macOS TextEdit Apple Vision ceiling broken**: char_accuracy 82.41% was the
  Session 35 "Apple Vision sparse-proportional ceiling" — not actually a
  content-class ceiling, just an Apple Vision recognition ceiling. EasyOCR
  matches it on char (79.63%) but **more than doubles F1** (27.55% → 58.43%).
  The framing in memory's "textedit char 0.8241" entry is now retired.
- **GTK4 per-element coordinate gap** (Session 39 diagnosis) and the
  **Windows windowsterminal focus bug** (Session 36) remain the only two
  structural blockers that no engine swap can fix. Both have dedicated tasks.
  Everything else is now downstream of the engine flip.
