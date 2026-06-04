# Spike: full four-triple cross-build matrix + `ffmpeg-next` link risk

**Date:** 2026-06-04 · **Branch/HEAD:** `port-swift-cli-to-rust` · **Leaf:** `160-crossbuild-matrix-spike`
**Extends:** `docs/research/080-crosscompile-spike.md` (which proved the `cargo-zigbuild` toolchain on two triples).

## Verdict

**The non-ffmpeg matrix is GREEN across all four triples** (modulo the one known,
already-owned Windows source gap). **`ffmpeg-next` is confirmed as the single
cross-compile blocker** — it cross-links on *neither* of its two build modes
out-of-the-box, for two orthogonal reasons. Both have tractable workarounds, but
they are real per-target work that `170` + the linux/win distribution leaf must
budget for; neither is a `cargo-zigbuild`/zig defect.

### Matrix — today's HEAD, no `ffmpeg-next` in the graph

| Target | Result | Detail |
|---|---|---|
| `x86_64-unknown-linux-gnu` | ✅ **link-complete** | 17 MB stripped ELF, 94 s. (re-confirms `080`) |
| `aarch64-unknown-linux-gnu` | ✅ **link-complete** | 15 MB stripped ELF, 104 s. **NEW — closes `080`'s "same path, not built" gap.** |
| `x86_64-pc-windows-gnu` | ⚠️ **toolchain proven, blocked at `monitor.rs`** | every native dep cross-compiled; stops at the known deferred source gap. (re-confirms `080`) |
| `aarch64-pc-windows-gnullvm` | ⚠️ **toolchain proven, blocked at `monitor.rs`** | **NEW — the genuinely-new risk, resolved.** `ring`+`wgpu`+`wgpu_hal`+`wgpu_core`+`naga` all produced aarch64-windows rlibs; stops at the *same* `monitor.rs` gate. |

Both Windows builds fail with the identical, expected error — **not** a toolchain
blocker:

```
error[E0432]: unresolved import `tokio::net::UnixStream`
  --> crates/testanyware-vm/src/monitor.rs:12:5
```

This is the `#[cfg(unix)]` QEMU-monitor seam `080` already identified as
Windows-host source work (AF_UNIX → named-pipe/TCP), owned by the deferred
Windows-host pass. The spike's job — confirm everything *up to* that point links
for all four triples — is done.

## `aarch64-pc-windows-*` resolved

There is **no `aarch64-pc-windows-gnu`** in rustup — only `gnullvm` and `msvc`.
`msvc` cannot cross-link from a Mac, so the matrix's fourth triple is
**`aarch64-pc-windows-gnullvm`**, and `cargo-zigbuild` supports it. It produced
aarch64-windows rlibs for the entire hard native surface (`ring`'s C build,
`wgpu`/`wgpu-hal`/`wgpu-core`/`naga` DX12 stack, `eframe`/`egui-wgpu`) before
stopping at our own `monitor.rs`. **The new-arch risk is retired: the toolchain
cross-links aarch64-windows.** (For x86_64-windows, `080` used `-gnu`; `-gnullvm`
also exists and is interchangeable for our purposes — `-gnu` is the proven one,
keep it.)

> Canonical four-triple set:
> `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`,
> `x86_64-pc-windows-gnu`, `aarch64-pc-windows-gnullvm`.

## `ffmpeg-next` cross-link — the load-bearing question

Probed by adding a throwaway `ffmpeg-next = "8.1"` to `testanyware-video`
(non-macOS target dep, mirroring the ADR-0006 seam), then cross-building the
crate for `x86_64-unknown-linux-gnu`. **Reverted before commit** — this leaf
ships knowledge, not the dep. `ffmpeg-next 8.1.0` pulls `ffmpeg-sys-next 8.1.0`,
which has two build modes; **both fail out-of-the-box, for different reasons:**

### Mode 1 — system linking (default features): blocked on a target sysroot

`ffmpeg-sys-next`'s `build.rs` probes for system `libav*` via `pkg-config` and
panics:

```
pkg-config has not been configured to support cross-compilation.
Install a sysroot for the target platform and configure it via
PKG_CONFIG_SYSROOT_DIR and PKG_CONFIG_PATH, or install a
cross-compiling wrapper for pkg-config and set it via PKG_CONFIG.
```

This fails **upstream of any linking** — it is the classic cross-compile sysroot
problem, not a zig/linker issue (contrast `ring`, whose C source is *vendored* in
the crate and built fine by `zig cc`; ffmpeg expects to *find* prebuilt target
libs). To make it work you must, **per target**, obtain a sysroot containing the
ffmpeg dev libraries (`.pc` + headers + `.so`/`.a`) and point the build at it:

- `PKG_CONFIG_ALLOW_CROSS=1`
- `PKG_CONFIG_SYSROOT_DIR=<sysroot>`
- `PKG_CONFIG_PATH=<sysroot>/usr/lib/<triple>/pkgconfig` (or equivalent)

How a sysroot is obtained per triple:
- **linux x86_64 / aarch64:** extract `libavcodec-dev libavformat-dev
  libavutil-dev libswscale-dev` (+ their `lib*.so`/headers) from Debian/Ubuntu
  **multiarch** `.deb`s for the target arch into a sysroot dir. Cheap, scriptable,
  no VM. Pin to the distro's ffmpeg ABI (so the runtime host must match — log a
  glibc/ffmpeg floor).
- **windows x86_64 / aarch64:** prebuilt MinGW ffmpeg dev libs
  (e.g. MXE, or the BtbN/gyan Windows ffmpeg "dev" zips for x86_64). **aarch64-windows
  ffmpeg dev libs are scarce** — this is the least-served corner and may force the
  fallback below.

### Mode 2 — build-from-source (`build` feature): blocked on a cross-prefix naming gap

`ffmpeg-sys-next`'s `build` feature clones ffmpeg and runs its `./configure &&
make`, and **does** wire cross-compilation (`--enable-cross-compile --arch
--target-os --cross-prefix`). It got past the clone and into configure, then
failed ffmpeg's C-compiler sanity test:

```
WARNING: Unknown C compiler zigcc-x86_64-unknown-linux-gnu-gcc
configure: line 1038: zigcc-x86_64-unknown-linux-gnu-gcc: command not found
C compiler test failed.
```

**Root cause (a naming integration gap, not a fundamental limitation):**
`ffmpeg-sys-next` derives `--cross-prefix=zigcc-<triple>-` from the Rust TARGET,
assuming **GNU-binutils naming** (`<prefix>gcc`, `<prefix>ar`, …). ffmpeg's
configure then composes the compiler as `<prefix>gcc` =
`zigcc-x86_64-unknown-linux-gnu-gcc` — but `cargo-zigbuild`'s actual wrapper is
uniquely suffixed (`zigcc-x86_64-unknown-linux-gnu-<pid>.sh`), so the composed
name does not exist on PATH. Configure also warns
`zigcc-<triple>-pkg-config not found`, the same assumption applied to the other
tools.

Workaround (untested here — flagged for `170`, not adopted): shim the GNU-style
names ffmpeg expects (`zigcc-<triple>-gcc`, `-ar`, `-pkg-config`) onto PATH,
pointing at `cargo-zigbuild`'s wrappers, **or** override `--cross-prefix` /
`CC`/`AR` via env. This is fragile per-triple yak-shaving and is exactly the gap
the `ffmpeg-sys-next-crossfix` fork on crates.io exists to paper over — a strong
signal that ffmpeg-on-cargo-zigbuild is a known rough edge.

### Recommendation for `170` + distribution

1. **System-sysroot path is the more tractable of the two for linux** (extract
   multiarch ffmpeg dev `.deb`s into a per-triple sysroot; set the three
   `PKG_CONFIG_*` vars). Prove it in `170` when `ffmpeg-next` actually lands.
2. **aarch64-windows ffmpeg is the weakest link** (no easy dev-libs source). If
   it stays infeasible, the **fallback is build-on-target via VMs** for the
   ffmpeg-dependent `screen record` on that triple — the harness already runs up
   native-arch host-VMs (ADR-0009), so a build-on-target step for the encoder is
   in reach. Record this so the distribution leaf re-plans around it rather than
   blocking on a missing sysroot.
3. **`screen record` is the only surface that pulls `ffmpeg-next`.** The rest of
   the CLI cross-links today on all four triples (proven above). So an
   ffmpeg blocker does **not** gate linux/win distribution of the
   non-record surface — it only gates `screen record` on the affected triple.
   The seam (ADR-0006) already returns `ACTION_UNSUPPORTED` where no encoder is
   wired, so a triple shipping without record degrades cleanly, not fatally.

## `scripts/` sketch — updated for the four-triple reality

`080` sketched the additive cross changes; this updates them for four triples,
heeding `080`'s `pipefail` warning and the glibc floor.

**`release-doctor.sh`** — add, beside `check_swift`/`check_dotnet`:
- `zig` on PATH (floor 0.16); `cargo-zigbuild` installed.
- `rustup target add x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu
  x86_64-pc-windows-gnu aarch64-pc-windows-gnullvm`.
- **only if `screen record` ships for a triple:** that triple's ffmpeg sysroot
  present (linux) or the build-on-target fallback wired (aarch64-windows).

**`release-build.sh`** — a `build_cli_cross <triple>` beside the native
`build_cli`:

```sh
set -o pipefail   # 080: `… | tail; EXIT=$?` reports the pipe, not cargo

build_cli_cross() {
  local triple="$1"
  # Pin the glibc floor on linux triples for portability (cargo-zigbuild syntax).
  case "$triple" in
    *-linux-gnu) triple="${triple}.2.17" ;;
  esac
  cargo zigbuild --release -p testanyware-cli --bin testanyware --target "$triple"
  # stage target/${triple%%.*}/release/testanyware[.exe]
}

for t in x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu \
         x86_64-pc-windows-gnu aarch64-pc-windows-gnullvm; do
  build_cli_cross "$t"
done
```

(Windows triples will fail at `monitor.rs` until the deferred Windows-host source
pass lands — sequence the windows builds *after* that pass, per the root brief.)

**Packaging / formula** —
`scripts/templates/testanyware.rb.tmpl` carries a single
`@SHA_AARCH64_APPLE_DARWIN@`. Add per-target placeholders + `url`/`sha256` blocks
for each shipped triple (linux: Homebrew bottle or `tar.xz`; windows: zip of
`testanyware.exe` + bundled agents). One placeholder per distributed triple.

**Stale metadata (carried from `080`):** `cli-rs/Cargo.toml
[workspace.metadata.dist] ci = "github"` contradicts the local-from-`scripts/`,
no-CI decision (`feedback_local_release_no_ci`). Drop or neutralise when
distribution is wired.

## Reproduce

```sh
cd cli-rs
rustup target add x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu \
                  x86_64-pc-windows-gnu aarch64-pc-windows-gnullvm
# Non-ffmpeg matrix (today's HEAD):
for t in x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu \
         x86_64-pc-windows-gnu aarch64-pc-windows-gnullvm; do
  cargo zigbuild --release -p testanyware-cli --bin testanyware --target "$t"
done
# ffmpeg probe: add `ffmpeg-next = "8.1"` (non-macOS dep) to testanyware-video,
# then `cargo zigbuild -p testanyware-video --target x86_64-unknown-linux-gnu`.
# System mode → pkg-config-cross panic; `build` feature → ffmpeg configure
# "zigcc-<triple>-gcc: command not found". Revert the dep afterwards.
```
