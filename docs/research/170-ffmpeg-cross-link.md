# `ffmpeg-next` cross-link proof — all four triples (leaf 170)

**Date:** 2026-06-04 · **Owns:** the ffmpeg cross-link risk `160` deferred to this
leaf (`170` adds `ffmpeg-next` to the tree, so it carries the real-integration
link proof).

## TL;DR

**GREEN on all four `140`-matrix triples.** With `ffmpeg-next = "8.1"` actually
in the graph (the non-macOS `VideoEncoder` arm), the `testanyware` binary /
`testanyware-video` test binary cross-links libav via `cargo-zigbuild` for every
triple — **including `aarch64-pc-windows`, the corner `160` called the "weakest
link."** The sysroot story `160` flagged as the load-bearing unknown is solved by
**BtbN's prebuilt per-triple ffmpeg dev libs**, not Debian multiarch `.deb`
extraction (160's tentative recipe) — one version-consistent source covers the
whole matrix, windows-arm64 included, so the build-on-target VM fallback `160`
reserved is **not needed**.

| Triple | ffmpeg link | Evidence |
|---|---|---|
| `aarch64-unknown-linux-gnu` | ✅ full `testanyware` bin | ELF aarch64, `NEEDED` libavcodec.so.62 / libavformat.so.62 / libavutil.so.60 / libswscale.so.9 |
| `x86_64-unknown-linux-gnu` | ✅ full `testanyware` bin | ELF x86-64, same four `NEEDED` |
| `x86_64-pc-windows-gnu` | ✅ `testanyware-video` test exe¹ | PE32+ x86-64, imports avcodec-62.dll / avformat-62.dll / avutil-60.dll / swscale-9.dll |
| `aarch64-pc-windows-gnullvm` | ✅ `testanyware-video` test exe¹ | PE32+ Aarch64, same four imports |

¹ **The windows _full-binary_ link is still blocked at the known, deferred
`monitor.rs` AF_UNIX gap** (Windows-host source pass, owned elsewhere — `160`,
root brief). That gap is unrelated to ffmpeg. To isolate the *ffmpeg* link from
it, the windows proof builds the `testanyware-video` **test binary** (which links
all of libav but never touches `monitor.rs`). So: ffmpeg cross-link is proven for
windows; the full windows `testanyware` bin remains blocked only at `monitor.rs`.

All four bind exactly **four** libs — `avcodec`, `avformat`, `avutil`,
`swscale` — and none of `avdevice` / `avfilter` / `swresample`, confirming the
feature reduction (below) took effect.

## What landed in the tree

- `testanyware-video/Cargo.toml`: `ffmpeg-next` as a
  `cfg(not(target_os = "macos"))` dep, **`default-features = false`** with only
  `["codec", "format", "software-scaling"]`. A video encoder needs avcodec +
  avformat (muxing) + swscale (RGBA→YUV420P); avutil is always linked. Dropping
  the audio (`software-resampling`) / `device` / `filter` defaults shrinks the
  binary, the bindgen output, and the link surface (4 libs, not 7).
- `testanyware-video/src/ffmpeg.rs`: the `FfmpegEncoder` — libx264/libx265 muxed
  to `.mp4`, swscale RGBA→YUV420P, PTS `frame_index/fps`, same `Setup`/`Append`/
  `Finish` error phases as the AVFoundation arm. Even-dimension guard up front
  (YUV420P 4:2:0 needs even w/h — the leaf's odd-dimension note).

## The recipe (per triple)

### 1. Obtain a sysroot — BtbN prebuilt ffmpeg dev libs

[BtbN/FFmpeg-Builds](https://github.com/BtbN/FFmpeg-Builds) ships
version-consistent **`gpl-shared`** dev bundles (headers + `lib/pkgconfig/*.pc` +
import libs + the codec libs, incl. libx264/libx265) for all four triples:

```
https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/
  ffmpeg-n8.1-latest-linuxarm64-gpl-shared-8.1.tar.xz   # aarch64-linux
  ffmpeg-n8.1-latest-linux64-gpl-shared-8.1.tar.xz      # x86_64-linux
  ffmpeg-n8.1-latest-win64-gpl-shared-8.1.zip           # x86_64-windows
  ffmpeg-n8.1-latest-winarm64-gpl-shared-8.1.zip        # aarch64-windows
```

Extract each (`tar xf` / `unzip`); no `dpkg` needed. **The `.pc` files use
`prefix=${pcfiledir}/../..` (relative)**, so pkg-config resolves `-I`/`-L` into
the extracted tree with **no `.pc` rewriting** — just point at the dir.

### 2. Point pkg-config + bindgen at it, then `cargo-zigbuild`

```sh
export PKG_CONFIG_ALLOW_CROSS=1                 # 160's blocker was exactly this
export PKG_CONFIG_LIBDIR="$SYSROOT/lib/pkgconfig"   # isolates from host .pc

# Linux: the full binary links libav directly.
cargo zigbuild -p testanyware-cli --bin testanyware --target aarch64-unknown-linux-gnu
cargo zigbuild -p testanyware-cli --bin testanyware --target x86_64-unknown-linux-gnu

# Windows: prove the ffmpeg link via the video test bin (full bin blocked at
# monitor.rs). bindgen needs the *clang* target — and clang rejects the
# `gnullvm` environment, so map gnullvm → gnu for the clang triple only:
BINDGEN_EXTRA_CLANG_ARGS="--target=x86_64-pc-windows-gnu" \
  cargo zigbuild -p testanyware-video --tests --target x86_64-pc-windows-gnu
BINDGEN_EXTRA_CLANG_ARGS="--target=aarch64-pc-windows-gnu" \
  cargo zigbuild -p testanyware-video --tests --target aarch64-pc-windows-gnullvm
```

**Gotcha (record for distribution):** `ffmpeg-sys-next` runs `bindgen`, and
clang errors `version 'llvm' in target triple ... is invalid` if handed the Rust
`*-gnullvm` triple. Pass clang the `*-gnu` form via `BINDGEN_EXTRA_CLANG_ARGS`
(the Rust/zig link target stays `gnullvm`). Linux/x86_64-windows did not strictly
need the bindgen `--target`, but setting it is correct for windows LLP64
(`long` = 32-bit) and harmless elsewhere.

## Runtime ABI — flagged for `190` + distribution (NOT solved here)

This leaf proves the **link**; runtime is `190`'s / distribution's call. The
binaries bind ffmpeg **8.1** sonames (`libavcodec.so.62`, …). Stock Ubuntu 24.04
ships ffmpeg **6.1** (`libavcodec.so.60`) — a soname mismatch — so a dynamically
linked binary will **not** run against the distro's libs unless the runtime host
has ffmpeg 8. Three options, to decide when distribution/`190` is wired:

1. **Bundle the BtbN shared libs** beside the binary (ship the `.so`/`.dll`, set
   rpath `$ORIGIN` on linux / same-dir DLL search on windows). Self-contained,
   version-matched, no distro dependency. Likely the simplest for `190`'s VM.
2. **Static link** — use BtbN's non-`shared` `gpl` bundle (static `.a`) for a
   single self-contained binary. Cleanest distribution; needs the static
   pkg-config dance (`Libs.private`, `--static`) verified.
3. **Require distro ffmpeg ≥ 8** — smallest binary, but stock Ubuntu 24.04 fails;
   would force a newer base image, conflicting with the "stock Ubuntu ARM64"
   plan. Not recommended.

`screen record` is the **only** surface that pulls ffmpeg (160 §3), so this
choice gates only `record`; the rest of the CLI is unaffected, and the seam
degrades cleanly (`ACTION_UNSUPPORTED`) where no encoder is wired.

## `scripts/` implications (extends 160's sketch)

- `release-doctor.sh`: when `screen record` ships for a triple, check that
  triple's ffmpeg sysroot is present (download/extract the matching BtbN bundle)
  **and** the bundled/static runtime strategy (above) is wired.
- `release-build.sh`: export `PKG_CONFIG_ALLOW_CROSS=1` +
  `PKG_CONFIG_LIBDIR=<sysroot>/lib/pkgconfig`, and
  `BINDGEN_EXTRA_CLANG_ARGS=--target=<clang-triple>` (gnullvm→gnu) before
  `cargo zigbuild`. Windows binaries still wait on the `monitor.rs` pass.

## Reproduce

```sh
SR=/tmp/taw-ffmpeg-sr; mkdir -p "$SR/dl"; cd "$SR/dl"
base=https://github.com/BtbN/FFmpeg-Builds/releases/download/latest
curl -sL -O $base/ffmpeg-n8.1-latest-linuxarm64-gpl-shared-8.1.tar.xz
curl -sL -O $base/ffmpeg-n8.1-latest-linux64-gpl-shared-8.1.tar.xz
curl -sL -O $base/ffmpeg-n8.1-latest-win64-gpl-shared-8.1.zip
curl -sL -O $base/ffmpeg-n8.1-latest-winarm64-gpl-shared-8.1.zip
for f in *.tar.xz; do tar xf "$f"; done; for z in *.zip; do unzip -qo "$z"; done
# then the four cargo-zigbuild invocations above, PKG_CONFIG_LIBDIR pointed at
# each extracted <dir>/lib/pkgconfig. Verify with `file` + `strings | grep '^libav'`
# (ELF NEEDED) or `strings | grep avcodec-62.dll` (PE imports).
```
