# verify-vision-on-downsampled-2x-k7

**Kind:** work (verify / measurement)

## Goal

Measure whether vision stays **on-distribution** when fed a logical 1920×1080 frame
produced by 2:1-downsampling a real 2× macOS render — versus the native-1× baseline.
This is the gate ADR-0016 (D4) deferred: the scale-aware surface gives vision the
right *dimensions*, but a downsampled-2× frame carries @2x assets, retina font
hinting, and heavier AA the native-1× synthetic training set never saw. Pass →
bless vision-on-HiDPI; material fail → HiDPI is realism/viewer-only + a retraining
workstream.

## Context

Read first: **ADR-0016** (D4 — the disposition + this gate), **ADR-0013** (the 1×
distribution + its "retraining is a separate workstream" note), `CONTEXT.md`
`[[HiDPI logical framebuffer]]` + `[[Framebuffer-pixel contract]]`. Depends on
**k4/k5/k6** (a working HiDPI session producing real downsampled-2× frames).

- Vision entry: `screen find-text` (`commands/screen.rs`) → OCR (EasyOCR daemon on
  Linux/Windows; Apple Vision on macOS — `testanyware-ocr-client`); window-detection
  + any icon classifier under `vision/`. Training distribution fixed at
  `vision/stages/window-detection/generator/src/window_gen/scenario_library.py:5`
  (`_SCREEN_W=1920, _SCREEN_H=1080`).
- The two frames to compare for the **same** guest scenes: (a) **native 1×** —
  `vm start` default (logical==physical 1920×1080); (b) **downsampled 2×** —
  `vm start --display 1920x1080@2x` (physical 3840×2160 → logical 1920×1080 via k5).
- **Retina-host dependency:** producing real 2× frames needs a Retina host (k4). If
  unavailable, fall back to a synthetic check (render a known scene at 2×, downsample,
  compare to the 1× render) and flag the lower-fidelity caveat — do not skip silently.

## Done when

- A representative scene/app set is run through `find-text` (OCR) + window-detection
  on both the native-1× and downsampled-2× frames; accuracy/parity deltas are
  measured against the 1× baseline (text recall/precision; detection IoU/box counts).
- A **pass bar** is stated and evaluated (vision within tolerance of the 1×
  baseline). Verdict recorded in ADR-0016's Verification (or a `docs/research/`
  note with primary measurements).
- On **pass:** vision-on-HiDPI is blessed (note it in ADR-0016). On **material
  fail:** ADR-0016 is amended to scope HiDPI as realism/viewer-only and a retraining
  workstream is named (ADR-0013's "separate workstream"); the icon classifier (@2x
  assets) is the prime suspect to call out.

## Notes

Measurement, not a model change. This is the last gate of the grove on the
minimal-opt-in scope — its verdict decides whether HiDPI is "full" (vision too) or
"realism-only" for now. Either verdict completes the grove; retraining, if needed,
is out of scope here.
