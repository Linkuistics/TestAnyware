# 060-windows-ocr-band

**Kind:** planning (decide the Windows OCR engine, then likely spawn one work
leaf to build it)

## Origin

The **deferred Windows OCR band** from `040-windows-harness` (Windows aarch64 ran
2/3 bands GREEN; the OCR band was walled because **EasyOCR is uninstallable on
win-arm64** — `opencv-python-headless` ships no `win_arm64` wheel, no in-guest
MSVC to source-build it). That wall is what hoisted and motivated the
`215-docker-host-unification` spike. **`215` reported (2026-06-07, reject** —
`docs/research/240-docker-host-unification.md`): do **not** containerize the whole
host binary, but it surfaced the correct *narrow* fix for OCR specifically. This
leaf decides that fix.

## The two candidates (from the `215` findings)

Both sit at the **ADR-0002 per-platform `OcrEngine` seam** — adding a Windows arm,
not changing the host architecture. Neither touches the host-side-framebuffer
invariant (OCR is host-side compute *downstream* of RFB capture, no hypervisor
dependency).

1. **Containerized Linux EasyOCR** (`engine = easyocr_container`). The native
   Windows host CLI captures the framebuffer (pure-Rust RFB client, already
   `screen capture`-GREEN on win-arm64 in `040`) and ships the PNG to a **Linux
   EasyOCR container** over a local socket/HTTP. Docker Desktop on Windows runs
   Linux containers natively — **no nested virt** (there's no VM-under-test here,
   just compute), so the fully-wheeled Linux EasyOCR stack runs regardless of
   Windows host arch. Reuses the existing `OcrChildBridge` daemon shape ([[CONTEXT.md]]
   *OCR daemon*), but the child is a container, not an in-host venv.
   - *Cost/risk:* adds a Docker dependency on the Windows user's machine; image
     build/ship/version; daemon-over-socket wiring; first-run latency.
2. **Native `Windows.Media.Ocr`** (`engine = windows_media_ocr`). The Windows
   built-in OCR API via WinRT FFI — no Python, no container, no extra runtime.
   - *Cost/risk:* a Windows-only FFI engine to write + maintain (objc2-equivalent
     for WinRT); accuracy/feature parity vs EasyOCR unknown (language packs,
     bounding-box quality on UI text); the ADR-0003 FFI-strategy question recurs
     for WinRT.

A third honest option: **accept the OCR gap on Windows for now** (ship the
2/3-green surface, document `screen find-text` as unsupported on win-arm64) and
defer until a user needs it.

## Grilling seeds

- Which engine — and is the decision arch-specific (x86_64-windows *can* install
  EasyOCR natively, so the container/native question may only bind on **arm64**)?
- Does the containerized path's Docker dependency violate [[minimal-images]] /
  user-friction expectations more than a native FFI engine's maintenance cost?
- Accuracy bar: does `Windows.Media.Ocr` meet the vision-pipeline's needs, or is
  EasyOCR parity required (cross-ref the ocr-accuracy grove)?
- Sequencing vs `050`: `050` ships the **OCR-less** 2/3-green Windows binary now
  (this leaf does **not** block it); whichever engine wins is an additive band.

## Done when

- A decision recorded (likely an **ADR**, or an extension of ADR-0002) on the
  Windows OCR engine: containerized EasyOCR / native `Windows.Media.Ocr` /
  accept-the-gap.
- If build work is implied, a work leaf is added for it (the `OcrEngine` Windows
  arm + the Windows harness OCR band finally run green, or the gap is documented
  as accepted).

## Notes

- Durable rationale for *why not containerize the whole host*:
  `docs/research/240-docker-host-unification.md` + ADR-0010.
- Acceptance gate for any resulting command behavior: **CLI design contract**.
