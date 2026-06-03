# 7. Golden-image provisioning uses `russh` (pure-Rust async SSH), not libssh2 or a shelled-out `ssh`

Date: 2026-06-03

## Status

Accepted

## Context

Porting `provisioner/scripts/vm-create-golden-macos.sh` into the Rust CLI's
`vm create-golden` command (grove node `110-vm-create-golden-macos`) requires an
SSH/SCP capability the Rust codebase does not yet have. The script drives the
setup VM almost entirely over SSH: dozens of `ssh user@ip '<cmd>'` calls
(`vm_ssh`) and a handful of `scp` uploads (`vm_scp`) for the pubkey, wallpaper
helper, agent binary, and LaunchAgent plist.

The auth shape matters: the vanilla Cirrus Labs image only allows **password**
auth (`admin/admin`) on first contact, so the script installs the host pubkey
over a password session and uses **pubkey** auth thereafter. In bash, the
one-time password step forces the fragile `SSH_ASKPASS` +
`SSH_ASKPASS_REQUIRE=force` + `DISPLAY=:0` dance — the exact mechanism memory
`vm-ssh-from-harness` records as breaking when the ssh process is backgrounded.

Two grove-level commitments constrain the choice:

1. **`zig cc` cross-compilation for distribution.** The grove distributes the
   Rust `testanyware` by cross-compiling locally with `zig cc` (no CI — releases
   run from `scripts/` on an arm64 Mac). C dependencies already fight this path:
   memory `linux-crosscheck-zig` records that `ring`'s C build fails the naive
   `cargo check --target x86_64-unknown-linux-gnu` and needs the `zig cc`
   wrapper. Adding another C library compounds that cost.
2. **Tier-2 reuse.** Linux and Windows golden creation (Tier 2) will reuse this
   same provisioning layer. A pure-Rust, async helper ports across hosts; a
   macOS-shaped shell-out (askpass semantics differ on Windows) does not.

The CLI is already `tokio`-async end to end.

## Decision

**`vm create-golden`'s SSH provisioning layer uses `russh` (+ `russh-sftp`), the
pure-Rust async SSH client.** A small `SshSession` helper in `testanyware-vm`
exposes `connect_password`, `connect_key`, `exec(cmd) -> {stdout, stderr, exit}`,
and `upload(local, remote)`, with an accept-any host-key handler (the setup VM is
a throwaway, matching the script's `StrictHostKeyChecking=no`).

`russh` is selected because it:

- does **native password and pubkey auth** in-process, eliminating the askpass
  fragility entirely (resolves the `vm-ssh-from-harness` gotcha by construction);
- adds **no C dependency**, keeping the `zig cc` cross-compile path clean and
  letting the Tier-2 linux/win goldens reuse the helper unchanged;
- is **async**, fitting the existing `tokio` runtime rather than forcing
  blocking I/O into an async CLI.

The cost — `russh` is lower-level than `ssh2`, so the `exec`/`upload` wrapper is
~100–150 lines of hand-written channel handling — is paid once in leaf `010` and
amortised across every provisioning call in `020`/`030` and all Tier-2 goldens.

## Considered Options

- **`ssh2` (libssh2 FFI bindings).** Mature, ergonomic high-level API
  (`Session`, `channel_session`, `scp_send`) — the least wrapper code. Rejected:
  it is a **C dependency** that directly fights the grove's `zig cc`
  cross-compile commitment (compounding the `ring` problem), and its **blocking**
  API sits awkwardly in the async CLI.
- **`Command::new("ssh"/"scp")` (shell out, 1:1 with bash).** Zero new
  dependencies, zero cross-compile risk (ssh is present on the host). Rejected:
  it **carries forward the askpass password-auth fragility** memory warns about,
  inherits the `COPYFILE_DISABLE`/tar gotchas, is hard to unit-test, and its
  askpass semantics **differ on Windows**, breaking Tier-2 reuse.

## Consequences

- `russh` + `russh-sftp` enter the `testanyware-vm` dependency tree (pure Rust;
  no native-toolchain burden — unlike `wgpu`/`ffmpeg-next`/libssh2). Leaf `010`
  re-runs the `zig cc` cross-check to confirm the dep tree stays cross-buildable.
- The `SshSession` helper is a **reusable provisioning seam**: `020` (normal-boot
  provisioning) and `030` (recovery/TCC over SSH) both consume it, and Tier-2
  linux/win golden creation reuses it without a host-specific rewrite.
- The askpass password step becomes a single `connect_password` call; the
  pubkey-install verification and all subsequent commands use `connect_key`.
  Memory `vm-ssh-from-harness`'s backgrounded-ssh/askpass warning is retired for
  this path (it remains relevant to ad-hoc harness SSH outside the CLI).
- The recovery automation (`030`) does **not** use SSH for the in-recovery UI
  steps — those run over RFB+OCR — but does use the SSH layer for the
  post-reboot `csrutil status` / TCC sqlite / health verification.
