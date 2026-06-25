# Vision on a downsampled-2× HiDPI frame — primary measurements

**Leaf:** `verify-vision-on-downsampled-2x-k7` (grove `hidpi-vision`).
**Date:** 2026-06-25. **Gate:** ADR-0016 D4 — the deferred on-distribution claim.

## Question

Does the macOS vision pipeline stay **on-distribution** when fed a logical
1920×1080 frame produced by 2:1-downsampling a real 2× (Retina) render, versus
the native-1× baseline? A downsampled-2× frame carries @2x assets, retina font
hinting, and heavier antialiasing the native-1× synthetic training set
(`vision/stages/window-detection/generator/src/window_gen/scenario_library.py:5`,
`_SCREEN_W=1920, _SCREEN_H=1080`) never saw.

## What is actually measurable (the finding that reshaped the leaf)

The leaf brief assumed OCR **and** window-detection were runnable on real frames.
Codebase reality:

| Consumer | Distribution-sensitive? | Live-wired into the CLI framebuffer path? | Runnable here? |
|---|---|---|---|
| **OCR** (Apple Vision on macOS, `engine=vision`) | No — general third-party pretrained model | ✅ `screen find-text` | ✅ live (no offline-file path on macOS) |
| **Window-detection** (YOLOv8) | **Yes** — trained on synthetic 1920×1080 1× | ❌ offline pipeline stage | ❌ **no checkpoint in repo** |
| **Icon classifier** (the ADR-0016 D4 "prime suspect") | **Yes** | ❌ offline pipeline stage | ❌ **no checkpoint in repo** |

- `find . \( -name '*.pt' -o -name '*.pth' -o -name '*.onnx' -o -name '*.safetensors' \)` and `*.mlpackage` → **empty**. The trained detectors exist only as
  training/analysis infrastructure (plus a distribution-agnostic Canny heuristic);
  there is no weight artifact to run on any frame.
- `grep -rn` over `cli-rs/` for window-detection / icon-classifier invocation → no
  hits beyond the downsample code itself. The **only** live framebuffer→vision
  consumer reachable through `vm start` is OCR.

So the consumer that is *measurable here* (OCR) is the one *least* at distribution
risk; the consumers *most* at risk (the trained detectors) cannot be measured —
they are not built, and are not live consumers of the HiDPI framebuffer.

## Method

Focused live run on a Retina-mode host (the host-display approach k4 used). The dev
host is a 1× 5120×2160 ultrawide; its panel offers `2560×1080 pt | 5120×2160 px |
2.00×`. The host main display was temporarily switched into that 2× mode (a fresh
process then reads `NSScreen.main.backingScaleFactor == 2.0` — what tart's `pt`
path consults at VM construction) and **restored to 1× afterward**.

Two fresh `testanyware-golden-macos-tahoe` clones, same ground-truth scene (an RTF
with a 28/18/14-pt heading, body pangrams, UI-label and alphanumeric-data lines,
and two deliberately adversarial small-font lines), opened in TextEdit at default
window size:

- **native-1×:** host at 1×, `vm start --platform macos` (ADR-0014 1× switch →
  guest 1920×1080 LoDPI).
- **downsampled-2×:** host at 2×, `vm start --platform macos --display 1920x1080@2x`.

Per run: `screen size`, `screen capture` (logical) + `screen capture --physical`,
`screen find-text --json` (Apple Vision). Binary: `cli-rs/target/release/testanyware`.

## Results

### A. Positive @2× path — confirmed end-to-end live (the k4-pinned sequence)

| Check | Result |
|---|---|
| `vm start --display 1920x1080@2x` guest switch | `switched main display to 1920x1080 pt @ 2x (3840x2160 px, modeID 23)` |
| Host-scale warning | **absent** (host genuinely 2× → request honored, not degraded) |
| `screen size` | **1920×1080** (logical) |
| `screen capture` (default) | **1920×1080** (downsampled) |
| `screen capture --physical` | **3840×2160** (raw Retina) |
| Downsample fidelity (logical vs 2:1 box-average of physical) | **≤1 LSB/channel, 100% within 1 LSB** (desktop meanabs 0.0042; text-scene meanabs 0.0077) |
| Logical click `(239,15)` on "Format" menu | menu opened (`Font`, `Make Plain Text`, …) → **×2 pointer mapping lands** |

The scale-aware `RfbConnection` (k5) presents a faithful logical surface live; k6's
opt-in wiring drives the guest-side Retina switch and suppresses the 1× switch.

### B. OCR (Apple Vision, `engine=vision`) — native-1× vs downsampled-2×

| Metric | native-1× | downsampled-2× | Δ |
|---|---|---|---|
| Exact ground-truth lines recovered | 7/9 | 7/9 | **0** |
| Token recall (63 tokens) | 59 (93.7%) | 57 (90.5%) | **−2 (−3.2 pts)** |

The 7 representative (non-adversarial) lines — heading, body pangrams, UI labels,
and the `Balance: $1,234.56 Ratio: 78.9% ID: AB-7700` alphanumeric line — were
recovered **perfectly and identically at both scales**. The entire −2-token delta
falls on the **single adversarial small-font line**
(`illegible? mInImUm xX oO lI 1l !@#$ legible`): at 2× the leading `i` of
"illegible" was dropped and "mInImUm" read as "minlmUm" — both 1-character `i/l/I/1`
confusions, the ambiguous-glyph noise floor of OCR, not a systematic downsample
effect.

**Caveat:** N=1 scene, two independent (not pixel-identical) VM runs; the
adversarial-line delta is within plausible run-to-run OCR noise.

## Pass bar and verdict

**Pass bar:** OCR text recall on the downsampled-2× frame within ≤5 points of the
native-1× baseline **and** representative (non-adversarial) text at parity (no
systematic, non-noise degradation).

**Result: PASS for the live vision path.** −3.2 pts, entirely on one adversarial
`i/l` line, representative text at 100% parity, on a frame that is a ≤1-LSB-faithful
2:1 downsample of the real Retina render.

**Disposition (amends ADR-0016 D4):**

1. **Bless vision-on-HiDPI for the current live pipeline (OCR).** HiDPI is *not*
   realism/viewer-only — the shipping live vision consumer operates at parity on the
   downsampled-2× frame. Apple Vision is a general pretrained model, so this is
   expected and robust.
2. **The distribution-sensitive trained detectors (window-detection, icon
   classifier) are carried forward to the existing training workstream, not failed.**
   They have no checkpoints and are not live framebuffer consumers, so there is
   nothing to degrade here. When they are built they will be trained on the team's
   target distribution anyway (ADR-0013's named "separate workstream"); a model
   trained only on synthetic 1× renders would by construction treat a downsampled-2×
   frame as off-distribution, with the **@2x-asset icon classifier the prime
   suspect** to validate first. This is a note on that workstream, not a HiDPI
   blocker.

This completes the grove on the minimal-opt-in scope: HiDPI ships as a working
opt-in with the live vision path empirically validated.
