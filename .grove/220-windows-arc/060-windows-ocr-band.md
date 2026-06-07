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

## Decisions (running log)

- **Engine = native `Windows.Media.Ocr`** (2026-06-08). A `#[cfg(windows)]`
  `OcrEngine::WindowsMediaOcr` variant at the ADR-0002 seam, reported token
  `"windows_media_ocr"`. Chosen over containerized Linux EasyOCR and
  accept-the-gap because it is the only option with **zero end-user runtime
  dependency** (WinRT OCR ships in Win10+; the distribution is a zip of
  `.exe`+DLLs, `220/050`), works **uniformly on both Windows arches**
  (no arm64/x86_64 OCR divergence — relevant since x86_64-windows is
  build-only), and is the **right tool for the workload** (rendered UI text,
  where classical OCR excels; EasyOCR's deep-learning edge is natural-scene
  text TestAnyware never sees). The container option's Docker-Desktop-on-the-
  user's-machine cost fails the same friction bar [[minimal-images]] encodes,
  just on the host rather than the image. Accept-the-gap was the honest
  fallback (Windows OCR is Tier-2, beyond-parity, additive — the grove's core
  goal of deleting Swift `cli/` is already met), declined because the native
  engine's cost is low enough to close the arc and keep Windows symmetric with
  Linux/macOS (both 3/3).

- **FFI strategy = pure-Rust `windows` crate** (2026-06-08). Microsoft's
  official WinRT bindings, a plain Cargo dependency (`Media_Ocr`,
  `Graphics_Imaging`, `Globalization` features). Direct analogue of ADR-0003's
  `objc2`-over-Swift-shim choice for macOS Vision; the reasoning transfers
  verbatim — a C#/WinRT shim would drag the .NET toolchain into a build that is
  today pure `cargo-zigbuild` **and cannot cross-compile from the Mac**, whereas
  the `windows` crate cross-builds cleanly for `aarch64-pc-windows-gnullvm`.

- **`detect()` is WinRT-only on Windows, no fallback env** (2026-06-08).
  `#[cfg(windows)]` `detect()` returns `WindowsMediaOcr` unconditionally — no
  Windows `TESTANYWARE_OCR_FALLBACK`. The daemon fallback is *dead on the only
  runtime-verified arch* (EasyOCR uninstallable on win-arm64) and x86_64-windows
  is build-only, so a fallback would be speculative surface for an unverified
  target (constraint 4 — add EasyOCR lazily if a user ever needs it). The
  harness's `TESTANYWARE_WINDOWS_TRY_OCR=1` stays a harness-local experimental
  knob, separate from product engine selection.

- **Scope = decide + decompose** (2026-06-08). This planning leaf records
  **ADR-0011** and spawns one work leaf **`070-windows-media-ocr-engine`** to
  build the variant + run the harness OCR band green; the build is its own
  focused session (the brief's stated intent). Fail-fast risk for `070`: prove
  the `windows`-crate **link** for `aarch64-pc-windows-gnullvm` via
  `cargo-zigbuild` early (the `160`-style check) — the crate bundles import
  libraries for all Windows arches so it *should* link, but verify before
  building the engine on top.

## Notes

- Durable rationale for *why not containerize the whole host*:
  `docs/research/240-docker-host-unification.md` + ADR-0010.
- Acceptance gate for any resulting command behavior: **CLI design contract**.
