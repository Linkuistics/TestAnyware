# 11. Windows OCR via native Windows.Media.Ocr, not a Linux EasyOCR container

Date: 2026-06-08

## Status

Accepted

## Context

ADR-0002 adopted per-platform OCR engine selection at the `OcrEngine` seam
(`testanyware-ocr-client/src/engine.rs`): macOS uses in-process Apple Vision,
Linux/Windows use the EasyOCR Python daemon (`OcrChildBridge`). That mapping
broke on **Windows aarch64**: EasyOCR is **uninstallable on win-arm64** —
`opencv-python-headless` (a hard dependency) ships no `win_arm64` wheel on PyPI,
conda-forge, or cgohlke's win-arm64 set, and cannot be source-built in a minimal
golden (no MSVC toolchain; the project's minimal-images discipline). This was the
low-regret kill signal recorded in the `220/040-windows-harness` disposition; the
Windows verification harness ran **2/3 bands green** (endpoint-free +
endpoint-driven) with the OCR band deferred.

That wall hoisted the `215-docker-host-unification` spike, whose findings
(`docs/research/240-docker-host-unification.md`) **rejected** containerizing the
whole host binary — it fails the host-side-framebuffer invariant (ADR-0010) on
exactly the two platforms it would help (macOS, Windows) — but surfaced one sound,
narrow carve-out: **OCR is host-side compute on a captured PNG, downstream of
framebuffer capture, with no hypervisor dependency**, so the OCR *engine* alone
could run in a Linux container regardless of host arch. That left a genuine
three-way decision for the deferred Windows OCR band, owned by leaf
`220/060-windows-ocr-band`:

1. **Containerized Linux EasyOCR** (`engine = easyocr_container`) — the native
   Windows host captures the framebuffer and ships the PNG to a Linux EasyOCR
   container over a local socket. Full EasyOCR parity; gate-irrelevant. *Cost:* a
   **Docker Desktop dependency on every Windows end-user's machine**, plus an
   image to build/ship/version, socket wiring, and first-run pull latency.
2. **Native `Windows.Media.Ocr`** (`engine = windows_media_ocr`) — the Windows
   built-in WinRT OCR API. No Python, no container, no extra runtime. *Cost:* a
   Windows-only FFI engine to write and maintain; accuracy/bounding-box parity vs
   EasyOCR unverified.
3. **Accept the gap** — ship the 2/3-green surface (`220/050` already does) and
   document `screen find-text` as unsupported on win-arm64, deferring until a user
   needs it.

The distribution context is decisive: `220/050` ships Windows as a **`.zip` of
`testanyware.exe` + co-located ffmpeg DLLs**, no Homebrew, no installer. The
relevant cost axis for option 1 is therefore **end-user runtime friction**, the
same axis the minimal-images discipline weighs — and a Docker daemon on the user's
host is a heavy ask layered onto a zip.

A secondary, precedent-setting question rides along: *how* to call WinRT from Rust
— the same FFI strategy choice ADR-0003 settled for macOS Vision, now recurring
for WinRT.

## Decision

Adopt **native `Windows.Media.Ocr`** as the Windows arm of the `OcrEngine` seam,
bound via the **pure-Rust `windows` crate**.

- **Engine.** A `#[cfg(windows)]` `OcrEngine::WindowsMediaOcr` variant, reporting
  the `engine` token `"windows_media_ocr"` in the `screen-find-text` JSON schema
  (alongside `"vision"` and `"easyocr_daemon"`). `recognize()` converts the PNG to
  a WinRT `SoftwareBitmap` and runs `Windows.Media.Ocr.OcrEngine` to produce lines
  and per-word bounding boxes mapped to `OcrDetection`.
- **Selection.** `#[cfg(windows)]` `detect()` returns `WindowsMediaOcr`
  **unconditionally** — there is **no Windows `TESTANYWARE_OCR_FALLBACK`**. The
  daemon fallback is dead on the only runtime-verified arch (EasyOCR uninstallable
  on win-arm64) and x86_64-windows is build-only, so a fallback env would be
  speculative surface for an unverified target. An EasyOCR path can be added
  lazily if a user ever needs it. (The harness's experimental
  `TESTANYWARE_WINDOWS_TRY_OCR=1` knob — which attempted in-guest EasyOCR
  provisioning while the band was deferred — is **retired in `070`** now that a
  real engine exists; the Windows OCR band runs unconditionally.)
- **FFI strategy.** The Microsoft-official **`windows` crate** (WinRT bindings;
  features `Media_Ocr`, `Graphics_Imaging`, `Globalization`), declared under
  `[target.'cfg(windows)'.dependencies]`. This is the direct analogue of
  ADR-0003's `objc2`-over-Swift-shim choice for macOS: a C#/WinRT shim would drag
  the .NET toolchain into a build that is today pure `cargo-zigbuild` **and cannot
  cross-compile from the Mac**, whereas `windows` is a plain Cargo dependency that
  cross-builds for `aarch64-pc-windows-gnullvm`.

Delivery is staged (grove lazy decomposition): this ADR records the decision; the
implementation + the live harness OCR band land in work leaf
`220/070-windows-media-ocr-engine`.

## Consequences

- **Zero end-user runtime dependency.** WinRT OCR ships in Windows 10+; the
  `220/050` zip stays a self-contained `.exe`+DLLs with nothing to install, pull,
  or keep running. The Docker-Desktop-on-the-user's-machine cost of the container
  option is avoided.
- **Uniform across both Windows arches.** `WindowsMediaOcr` works identically on
  aarch64 and x86_64, so there is no arm64/x86_64 OCR divergence — important since
  x86_64-windows is build/link-verified only (no native guest here).
- **Right tool for the workload.** The OCR target is rendered UI text (menu bars,
  labels — high-contrast, crisp), where classical OCR excels; EasyOCR's
  deep-learning edge is natural-scene text TestAnyware never captures. Accuracy is
  verified against the workload by the harness OCR band, not assumed.
- **`docs/research/240-docker-host-unification.md` is the durable rationale** for
  why the container carve-out, though architecturally sound and gate-irrelevant
  (ADR-0010), was *not* chosen here — the cost lands on the end user's host, not
  the build. This ADR does not reopen the host-side-framebuffer invariant; it
  leans on the spike's finding that OCR is gate-irrelevant and decides the engine
  on friction grounds.
- **The `windows` crate becomes a load-bearing dependency** for the Windows side
  of the port, per-target-gated so non-Windows builds never compile it — the same
  containment ADR-0003 applies to `objc2`. A fail-fast link check for
  `aarch64-pc-windows-gnullvm` via `cargo-zigbuild` precedes the engine build in
  `070` (the crate bundles import libraries for all Windows arches, so it should
  link, but the cross-from-Mac link is proven before building on top).
- **Windows reaches OCR parity (3/3 bands) with Linux and macOS** once `070` runs
  the band green on aarch64-windows, closing the gap the `040` disposition logged.
  x86_64-windows OCR remains build-verified only, consistent with ADR-0009's
  no-silent-caps treatment of the unverified arch.
- **Detection granularity is per *word*** (`070`). `Windows.Media.Ocr` returns
  lines, each holding words, but only `OcrWord` carries a `BoundingRect` (a line
  exposes text without a box). Emitting one `OcrDetection` per word also serves
  `find-text` consumers best — a tight, clickable box around the matched token
  (the "File" menu) rather than a line box spanning the whole menu bar whose
  centre misses the target — and matches the CLI's "substring match within a
  single detection" contract and the word/phrase granularity of the EasyOCR and
  Vision arms.
- **Confidence is synthesized as `1.0`** (`070`). `Windows.Media.Ocr` exposes no
  per-word confidence, so every Windows detection reports `1.0` — unlike Vision
  (which carries a real per-observation score and drops anything below 0.5) and
  EasyOCR (real per-detection score). Consumers that sort or threshold on
  `confidence` see all Windows detections as fully trusted; the engine token
  `windows_media_ocr` in the envelope signals which semantics apply.
