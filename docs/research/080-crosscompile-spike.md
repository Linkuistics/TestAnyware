# Spike: single-Mac `zig cc` cross-build of the Rust `testanyware` CLI

**Date:** 2026-06-02 · **Branch/HEAD:** `port-swift-cli-to-rust` · **Leaf:** `080-crosscompile-spike`

## Verdict

**FEASIBLE — proceed with the `zig cc` cross-compile path. The build-on-target
(VM) fallback is NOT needed.**

| Target | Result | Notes |
|---|---|---|
| `x86_64-unknown-linux-gnu` | ✅ **runnable binary today** | full release link, 18 MB stripped ELF |
| `x86_64-pc-windows-gnu` | ✅ **cross-toolchain proven**, ⚠️ blocked by one in-tree `#[cfg]` gap | every hard native dep cross-compiled; failure is a source-portability item already owned by Tier-2 Windows-host work, **not** a zig/linker/sysroot blocker |

The spike's job was to de-risk the **toolchain**. The toolchain is proven for
both targets. Linux produces a shippable binary now; Windows needs a small,
already-scoped source port (cfg-gate the QEMU monitor transport) and nothing
from the toolchain side.

## Method

Host: arm64 Mac (`aarch64-apple-darwin`). Tools: `zig` 0.16.0
(`/opt/homebrew/bin/zig`), `cargo-zigbuild` (already installed). Targets added
via `rustup target add x86_64-unknown-linux-gnu x86_64-pc-windows-gnu`.

Real **release link** of the shipping binary (not `cargo check`), on current
HEAD — which already links the two hardest native deps, `wgpu` (ADR-0005
embedded viewer) and `ring` (reqwest/rustls TLS):

```sh
cargo zigbuild --release -p testanyware-cli --bin testanyware --target <triple>
```

Release profile is `lto = "thin"`, `codegen-units = 1`, `strip = "symbols"`
(workspace `[profile.release]`).

> Note on tooling: `cargo-zigbuild` supersedes the hand-rolled `zcc` wrapper in
> memory `reference_linux_crosscheck_zig`. It ships the zig-cc-as-C-compiler and
> zig-cc-as-linker plumbing *and* a glibc-version-pinning shim, so it resolves
> both `ring`'s C build and GLIBC symbol-versioning in one tool. Prefer it over
> the manual `CC_*` / `CARGO_TARGET_*_LINKER` env dance.

## Linux x86_64 — ✅ runnable binary

- Built clean in **1m18s** from a cold-ish cache. Output:
  `target/x86_64-unknown-linux-gnu/release/testanyware` —
  `ELF 64-bit LSB pie executable, x86-64, dynamically linked,
  interpreter /lib64/ld-linux-x86-64.so.2, for GNU/Linux 2.0.0, stripped`, 18 MB.
- `ring`'s C build compiled via `zig cc` cleanly — the documented offender is
  fully handled by `cargo-zigbuild`.
- `eframe`/`wgpu`/`egui-wgpu`/`winit` linked **with no graphics sysroot**. Why:
  the Linux windowing/GPU stack is `dlopen`-ed at runtime, not linked —
  `winit` loads X11 via `x11-dl` and Wayland via runtime loaders, `wgpu` loads
  Vulkan via `libloading`. So the link step has zero system-library dependency;
  those libs only matter on the *target host at runtime*.
- Only output: the two pre-existing benign `qemu_profile.rs` warnings
  (`unused import: Path`, `unused mut dirs` — both inside its own macOS cfg
  block; documented as expected in `reference_linux_crosscheck_zig`).
- **Runtime smoke** (does the ELF actually run on Linux) is deliberately *not*
  done here — that is the job of the Tier-2 self-hosted verification harness
  (`140-tier2-plan`): run up a Linux host-VM, install this binary, drive the
  non-`vm-start` surface. The spike answers the *build/link* question: yes.
- `aarch64-unknown-linux-gnu` (also in the dist target list) was not built but
  is the same path with a different triple — low risk given x86_64 is green.

## Windows x86_64 — ✅ toolchain proven, ⚠️ one source gap

The build cross-compiled the **entire hard native surface** for
`x86_64-pc-windows-gnu` before stopping at TestAnyware's own code. Confirmed
artifacts under `target/x86_64-pc-windows-gnu/release/deps/`:

- `libring-*.rlib` — `ring`'s C build via `zig cc` for windows-gnu: **success**.
- `libwgpu-*.rlib`, `wgpu-core`, `wgpu-hal`, `naga`, `libeframe-*.rlib`,
  `libegui_wgpu-*.rlib` — the **DX12 / native graphics stack**: **success**.

The compile then failed in `testanyware-vm`:

```
error[E0432]: unresolved import `tokio::net::UnixStream`
  --> crates/testanyware-vm/src/monitor.rs:12
     UnixStream is #[cfg(all(unix, feature = "net"))] in tokio
```

`monitor.rs` talks to the QEMU monitor over an **AF_UNIX socket**, imported and
used unconditionally (lines 12, 28). On Windows, QEMU exposes the monitor over a
**named pipe or TCP**, not a Unix domain socket. This is the kind of `#[cfg]`
portability work already scoped to **Tier-2 Windows-host support** ("backlog
task 14" stubs in `qemu_profile.rs`/`process.rs`), not a cross-toolchain blocker.

**Why this is the cleanest possible "yes" for the toolchain:** a
zig/sysroot/linker showstopper can only manifest in the third-party native
crates (`ring`'s C, `wgpu`'s HAL). All of those produced windows-gnu rlibs. A
`#[cfg(unix)]` gap in our own source is identical to what a *native* Windows
`cargo build` would hit — it has nothing to do with cross-compiling from a Mac.

**Windows-host port surface (input for `140`):** most Unix-specific code is
*already* `#[cfg(unix)]`-gated and will need `#[cfg(windows)]` counterparts:
- `monitor.rs` — **the one un-gated seam** (QEMU monitor: AF_UNIX → named pipe/TCP).
- `process.rs` — already cfg(unix)/cfg(windows) paired (PID liveness, signals via `nix`).
- `spec.rs`, `detached.rs`, `doctor.rs` — already `#[cfg(unix)]` (`PermissionsExt`,
  `nix::unistd::setsid/getsid`).
The Rust-level Windows port is bounded and largely anticipated; the toolchain
imposes no additional Windows cost.

> ⚠️ The spike command's `… | tail … ; EXIT_WIN=$?` reported `EXIT_WIN=0` — that
> is the `tail` pipe's exit, not cargo's. cargo actually failed (E0432). Use
> `set -o pipefail` (or check `${PIPESTATUS[0]}`) in the eventual release script.

## Not yet covered — `ffmpeg-next` (re-run when `100` lands)

`ffmpeg-next` (the Linux/Windows `screen record` encoder, ADR-0006) is **not yet
in the tree** — it arrives with leaf `100-screen-record-encoder-macos`'s Tier-2
follow-on. It is the **one remaining cross-compile risk**: unlike `wgpu` (runtime
`dlopen`), `ffmpeg-next` links against system `libav*` at **link time** via
`pkg-config`, which needs a target sysroot with the ffmpeg dev libs. Re-run this
spike's matrix with `ffmpeg-next` in the graph once the macOS encoder lands; a
green wgpu+ring link is already the strong signal, but ffmpeg-next must be
proven separately before Tier-2 Linux/Windows distribution commits.

## Sketch: `scripts/` changes for Tier-2 distribution

Current `scripts/release-build.sh` builds the **Swift** CLI for
`aarch64-apple-darwin` only and bundles it into a Homebrew tarball. Tier-2
distribution (cross targets) is **additive**:

1. **`release-doctor.sh`** — add checks mirroring `check_swift`/`check_dotnet`:
   - `zig` on PATH (floor: 0.16); `cargo-zigbuild` installed
     (`cargo install cargo-zigbuild`); `rustup target add` for
     `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`, `x86_64-pc-windows-gnu`.
2. **`release-build.sh`** — add a `build_cli_cross <triple>` alongside the
   existing native `build_cli`:
   ```sh
   cargo zigbuild --release -p testanyware-cli --bin testanyware --target "$triple"
   # stage target/$triple/release/testanyware[.exe]
   ```
   Consider pinning the glibc floor for portability, e.g.
   `--target x86_64-unknown-linux-gnu.2.17` (cargo-zigbuild syntax). Add
   `set -o pipefail` so a failed cross-build isn't masked.
3. **Packaging** — Linux: Homebrew bottle (`brew` supports Linux) or plain
   tar.xz; Windows: zip of `testanyware.exe` + bundled agents. The
   `testanyware.rb.tmpl` formula currently carries a single
   `@SHA_AARCH64_APPLE_DARWIN@`; add per-target SHA placeholders.
4. **Stale metadata to reconcile (note for the distribution leaf):**
   `cli-rs/Cargo.toml [workspace.metadata.dist]` has `ci = "github"`, which
   contradicts the local-from-`scripts/`, no-CI decision
   (`feedback_local_release_no_ci`). Either drop the cargo-dist block or set it
   to a non-CI mode when distribution is wired up.

## Reproduce

```sh
cd cli-rs
rustup target add x86_64-unknown-linux-gnu x86_64-pc-windows-gnu
cargo install cargo-zigbuild   # if absent; needs zig on PATH
cargo zigbuild --release -p testanyware-cli --bin testanyware --target x86_64-unknown-linux-gnu
cargo zigbuild --release -p testanyware-cli --bin testanyware --target x86_64-pc-windows-gnu
```
