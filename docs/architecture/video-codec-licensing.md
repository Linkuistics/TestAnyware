# Video codec licensing for the Rust CLI

**Status:** decided (needs project-owner ratification before the
recording task lands).
**Audience:** anyone implementing
`port-testanyware-record-to-embedded-libav-ffmpeg-next-instead-of-subprocess`
or maintaining the brew formula.

## Constraints

The recording task carries three hard constraints from the design
conversation:

1. **Embedded library, not subprocess.** No shell-out to system
   `ffmpeg`. The encoder ships inside the `testanyware` binary.
2. **Self-contained binary.** Static linking is preferred so the brew
   bottle / Windows zip works without runtime codec downloads.
3. **Cross-platform.** macOS (arm64 + x86_64), Linux (x86_64 + arm64),
   Windows (arm64 priority, x86_64 nice-to-have).

To these we add a fourth constraint coming from the project itself:

4. **Permissive licensing.** The `cli-rs` workspace and the top-level
   `LICENSE` both declare Apache-2.0. The binary is distributed under
   a permissive non-GPL license. Static linking GPL code into this
   binary would relicense the whole binary as GPL — a policy change
   well outside the scope of the recording task.

## The codec / licence matrix

| Encoder | Source licence | Patent regime | Static-link OK for permissive binary? |
|---|---|---|---|
| `libx264` (software AVC) | GPL-2.0+ (commercial avail.) | MPEG-LA AVC | **No** — would force GPL relicense |
| `libx265` (software HEVC) | GPL-2.0 | MPEG-LA HEVC + Access Advance + Velos | **No** — would force GPL relicense |
| `OpenH264` source build (Cisco) | BSD-2-Clause | MPEG-LA AVC — **not** covered by Cisco's per-binary fee when we build from source | License OK; **patent exposure** if we self-build |
| `OpenH264` Cisco-distributed binary | (binary only) | Cisco pays MPEG-LA on Cisco's binary distribution | OK for Cisco's binary only → would require runtime dynamic load, defeats "self-contained" goal |
| `h264_videotoolbox` (Apple framework, via libavcodec wrapper) | Apple SDK | Apple covers under macOS / iOS licence | **Yes** on macOS hosts |
| `h264_vaapi` / `h264_qsv` / `h264_nvenc` (libavcodec wrappers around hw encoders) | LGPL or libavcodec linkage; underlying drivers from hw vendor | OEM/vendor covers via hardware purchase | **Yes** on Linux when the host has the hardware |
| `h264_mf` (Windows Media Foundation, via libavcodec wrapper) | Windows SDK | Windows licence covers | **Yes** on Windows hosts |
| `rav1e` (software AV1) | BSD-2-Clause | AOMedia royalty-free pledge | **Yes** universally |
| `libaom-av1` (reference AV1 encoder) | BSD-2-Clause | AOMedia royalty-free pledge | **Yes** universally |

## Decision

Adopt a two-tier strategy:

### Tier 1 — hardware-accelerated H.264 (default when available)

Use libavcodec's hardware-encoder wrappers, selected at runtime per
host:

- macOS: `h264_videotoolbox` (matches the existing Swift `AVAssetWriter`
  path; same encoder, same royalty regime, just driven through libav
  instead of AVFoundation).
- Linux: probe `h264_vaapi` first (Intel/AMD iGPUs and AMD dGPUs),
  then `h264_nvenc` (NVIDIA), then `h264_qsv` (Intel Quick Sync) as
  fallbacks.
- Windows: `h264_mf` (Media Foundation; backed by the OEM driver).

This dodges both licensing problems — neither the encoder source nor
the patent royalties pose a question for us. The royalty obligation
sits with the OS/hardware vendor, who has already collected it from
the user.

### Tier 2 — software AV1 fallback (when no hardware encoder is present)

When Tier 1's runtime probe reports no available hardware encoder,
fall back to software AV1 via `rav1e` (preferred) or `libaom-av1`. Both
are BSD-2-Clause source and are covered by the AOMedia royalty-free
patent pledge. They statically link cleanly into a permissive binary.

The trade-off is encoder speed: real-time 1080p30 software AV1 is
unrealistic on a typical developer laptop; expect 5–15 fps encode for
non-tiled rav1e at default settings. For test recording this is
acceptable — captures are diagnostic artefacts, not real-time
streaming, and the recorder can buffer raw frames and encode behind
the wall clock if needed. Document this limitation in
`testanyware record --help`.

### Out of scope (deliberately)

- **`libx264` / `libx265`**: would require GPL'ing the whole binary.
  The project owner can revisit if there is later demand for "best-
  quality H.264 / HEVC software encoding" — the cost is a
  substantial licensing change, not a minor build option.
- **Self-built `OpenH264`**: open-source-licence-clean but patent-
  exposed. Cisco's MPEG-LA arrangement covers Cisco's binary
  distribution only; building from source ourselves does not inherit
  that coverage.
- **Runtime-fetched Cisco OpenH264 binary** (the Firefox model):
  technically licence-clean and patent-clean, but contradicts the
  "self-contained binary" constraint and adds first-run failure modes
  that hurt the doctor experience.
- **HEVC encoding** of any flavour: HEVC's three-pool patent regime
  (MPEG-LA, Access Advance, Velos Media) is materially more
  contentious than AVC's. Hardware HEVC via `hevc_videotoolbox` etc.
  could be added later as a Tier-1 option for users with HEVC
  hardware, but is not part of the initial recording task.

## Build configuration

`ffmpeg-next` is the Rust binding crate. The recording task should
configure its libavcodec build (whether vendored or system) to:

- **Enable** `videotoolbox`, `vaapi`, `nvenc`, `qsv`, `mediafoundation`
  hardware-encoder bridges per the host platform.
- **Enable** `librav1e` and/or `libaom`.
- **Disable** `libx264`, `libx265`, `libfdk_aac`, and any other
  GPL/non-redistributable-license codec to ensure the linked
  libavcodec binary is itself permissive (LGPL-compatible). This is
  enforced by the libavcodec build flags `--disable-gpl
  --disable-nonfree` (libavcodec defaults to LGPL when those are off).

The recording task's CI must run `ldd` (Linux), `otool -L` (macOS),
and `dumpbin /imports` (Windows) on the produced binary and assert
that no GPL shared library is linked.

## Platform-specific notes

### macOS

The existing Swift `AVAssetWriter` implementation uses VideoToolbox
internally. Replacing it with `h264_videotoolbox` via `ffmpeg-next`
uses **the same underlying encoder** — output should be effectively
indistinguishable bit-for-bit at matched settings. This is the lowest-
risk replacement path for macOS hosts and produces the strongest
parity with the macOS host's AVFoundation recordings.

### Linux

VA-API support requires `/dev/dri/renderD128` and a working Mesa or
vendor userspace driver. `testanyware doctor` (port pending in
`port-testanyware-doctor-with-linux-host-preflight-checks`) should
probe for at least one available hardware encoder and surface a
remediation hint when only the AV1 software fallback is available
(some users will be surprised by the encoder speed).

### Windows

Media Foundation H.264 is universally available on Windows 10+. The
Tier-1 path should hit on every supported host. Software AV1 fallback
is unlikely to be exercised on Windows.

## Patent expiry watch

The major MPEG-LA AVC patents are scheduled to expire by ~February
2027 in the US (later in some jurisdictions; check current MPEG-LA
filings). After expiry, software H.264 via OpenH264 source build
becomes patent-clean and is worth re-evaluating — but for the
recording task as designed today (target ship date well before that
window) the AV1 software fallback is the correct choice.

## Open questions for project ownership

1. **Future GPL build variant.** If a power user requests
   high-quality software H.264 via `libx264`, we could add a
   `--features gpl` cargo build that produces a separately-distributed
   GPL-licensed binary. Decision is "not now" but worth recording the
   knob exists.
2. **Legal review.** This document is engineering analysis, not legal
   advice. Have a lawyer review before tagging the first release that
   ships any encoder.

## Cross-references

- Original task:
  `port-testanyware-record-to-embedded-libav-ffmpeg-next-instead-of-subprocess`
  (cite this document in its **Constraints** section).
- This document was produced by:
  `investigate-video-codec-licensing-for-embedded-libav-static-linking`.
- See `docs/architecture/cli-design-contract.md` §4.4 for the
  `RECORD_*` error-code family.
