# 010-machinery-and-endpoint-free

**Kind:** work

## Goal

Stand up the reusable harness skeleton and prove ADR-0009's #1 claim — **the
cross-compiled `testanyware` executes on a real aarch64-linux host** — by
provisioning a stock Ubuntu ARM64 HUT over ssh and running the **endpoint-free
smoke band** green. No macOS golden, no port-forward, no OCR. See the node
BRIEF for the shared design and the libav-is-load-time warning.

## Build (do this on the macOS host before touching VMs)

1. **Obtain the ffmpeg-8 aarch64-linux sysroot** (for the cross-build link) and
   the matching **runtime `.so` bundle** (for the HUT). Same BtbN `gpl-shared`
   tarball serves both:
   `ffmpeg-n8.1-latest-linuxarm64-gpl-shared-8.1.tar.xz` from
   `github.com/BtbN/FFmpeg-Builds/releases/download/latest/`. Extract; the `.pc`
   files use a relative `prefix`, so no rewriting.
2. **Cross-build** (recipe from `docs/research/170-ffmpeg-cross-link.md`):
   ```sh
   export PKG_CONFIG_ALLOW_CROSS=1
   export PKG_CONFIG_LIBDIR="$SYSROOT/lib/pkgconfig"
   cargo zigbuild -p testanyware-cli --bin testanyware \
     --target aarch64-unknown-linux-gnu --release
   ```
   Verify the artifact: `file` → `ELF aarch64`; confirm the four `NEEDED`
   libav entries (`readelf -d` / `strings | grep '^libav'`).
3. Collect the four runtime `.so`s (`libavcodec.so.62`, `libavformat.so.62`,
   `libavutil.so.60`, `libswscale.so.9`) + their libx264/libx265 deps from the
   bundle's `lib/`, to upload beside the binary.

## Harness skeleton (`tests/linux-host-harness.rs`)

- `#[ignore]` + `TESTANYWARE_LINUX_HARNESS=1` gate (mirror `live-vm-gate.rs`).
- **HUT lifecycle** via `testanyware-vm::tart` (`clone`/`run_detached`/`poll_ip`/
  `remove_existing` are `pub`) — clone `ghcr.io/cirruslabs/ubuntu:24.04` to a
  throwaway `testanyware-hut-<id>`, run detached, poll IP (state-gated). A
  `Drop` guard stops+deletes the clone even on panic (à la `VmGuard`).
- **Provisioning channel seam** — define it as a small trait/enum now (e.g.
  `ProvisionChannel { exec, upload }`) with an ssh impl over `SshSession`, so the
  Windows leaf later adds an agent impl without touching the band driver. This is
  the node's reuse seam; get it right here.
- **Provision:** `SshSession::wait_for_password(ip, 22, "admin", "admin", …)`,
  install harness pubkey into `~/.ssh/authorized_keys`, reconnect via
  `connect_key`. Upload the binary + the four ffmpeg `.so`s into one dir; run the
  binary with `LD_LIBRARY_PATH=<dir>` (or set rpath `$ORIGIN` at build via
  `-C link-arg=-Wl,-rpath,$ORIGIN` and skip the env). **First in-guest command
  must confirm the binary execs** (`testanyware --version`) — if it errors with
  `cannot open shared object file: libavcodec.so.62`, the `.so` staging is wrong;
  fix before proceeding (this is the load-time-libav check).
- **Band driver:** a runner that takes a list of (cmd args, assertion) and runs
  each over the channel, parsing `--json`. Factor it band-agnostic so `020`/`030`
  add bands without rewriting it.

## Endpoint-free band (assert `--json` envelopes)

`capabilities`, `schema`, `llm-instructions`, `doctor`, `--help`, and a
`--dry-run` of a mutating command. These need no endpoint — they prove the
binary runs, links resolve, and the contract envelopes emit correctly on the
target. (`doctor` may report missing tools — assert it *runs and emits a valid
envelope*, not that every check passes.)

## Done when

- `TESTANYWARE_LINUX_HARNESS=1 cargo test -p testanyware-cli --test
  linux-host-harness -- --ignored` runs **green** on this Mac: clones+starts a
  stock Ubuntu ARM64 HUT, ssh-provisions the aarch64 binary + ffmpeg-8 `.so`
  bundle, runs the endpoint-free band, asserts, and tears the HUT down.
- The binary is confirmed to **exec** on aarch64-linux (the `--version`/`--help`
  load-time check passes — libav `NEEDED` resolves).
- Pure helpers (band assertion, any host/endpoint parsing) have offline unit
  tests that run in a plain `cargo test`.
- The **x86_64 build-verified-only gap is logged** in the harness doc-comment
  (no native x86_64 guest on this Mac; ADR-0009 no-silent-caps).
- The provisioning-channel seam + forward-less skeleton are factored so `020`
  bolts on the golden + forward + endpoint bands cleanly.

## Notes

- VM clone+start is cheap ([[vm-costs]]); iterate freely.
- `cargo-zigbuild`, `zig`, `tart` confirmed on PATH; `socat` is **not** (hence
  the in-process forward, which `020` adds — `010` needs no forward).
- `tart-ip-lies`: trust the IP only once `tart list` state == running (the
  `poll_ip` helper already gates on this).
- May need `tokio` dev-deps (`rt-multi-thread`, `macros`, `net`) added to
  `testanyware-cli` for the async russh/forward harness; `SshSession` is `pub`
  from `testanyware-vm`.
