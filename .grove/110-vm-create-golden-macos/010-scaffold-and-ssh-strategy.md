# 010-scaffold-and-ssh-strategy

**Kind:** work

## Goal

Lay the foundation for `vm create-golden`: wire the command into the surface and
clap, stub the handler with a working `--dry-run` boot plan, and build the
**`russh` SSH provisioning helper** (ADR-0007) that boots 020/030 will drive.

## Context

- SSH strategy is **already decided**: `russh` (pure-Rust, async) — see
  ADR-0007 and the node BRIEF's decisions log. This leaf *implements* that
  decision; it is not a re-grilling.
- `vm create-golden` is **not in the surface today**. Add it to
  `surface.rs::CANONICAL_COMMANDS` as
  `CommandSpec { path: &["vm", "create-golden"], mutating: true,
  data_producing: true, schema_id: Some("vm-create-golden") }`, plus the schema
  file under the cli-schemas dir, plus the clap `VmAction::CreateGolden`
  variant and dispatch in `main.rs`.
- Flags (match the script): `--platform <p>` (macos only this node),
  `--version <v>` (default `tahoe`), `--name <n>` (default
  `testanyware-golden-macos-<version>`), plus `--json`, `--dry-run`.
- Mirror the contract pattern in `commands/vm.rs::run_vm_start`: dry-run →
  validate + emit plan + return; else execute. Add error codes to the
  `ERROR_CODES` catalogue as needed (e.g. `SSH_CONNECT_FAILED`,
  `GOLDEN_CREATE_FAILED`; reuse `TART_FAILED`, `USAGE_ERROR`, `INVALID_PLATFORM`).
- `#[cfg(target_os = "macos")]` — gate the whole command like `tart.rs`. The
  non-macOS build should surface a clean "platform not supported" error, not a
  compile break.

## SSH provisioning helper (the deliverable)

A small async module (likely `testanyware-vm/src/ssh.rs` or a new seam) exposing
roughly:

- `connect_password(host, port, user, password) -> SshSession` — for the **one**
  initial connection to the vanilla `admin/admin` image.
- `connect_key(host, port, user, key_path) -> SshSession` — pubkey auth for
  every subsequent call.
- `SshSession::exec(cmd) -> { stdout, stderr, exit_code }` — the `vm_ssh`
  equivalent; the workhorse (the script makes dozens of these).
- `SshSession::upload(local_path, remote_path)` — the `vm_scp` equivalent
  (`russh-sftp`). Used for pubkey, wallpaper helper+png, agent binary, plist.
- Host-key policy: accept-any (the script uses `StrictHostKeyChecking=no` against
  a throwaway VM) — implement russh's `client::Handler` to accept unconditionally.
- Connect retry/wait helper: poll until SSH is reachable (replaces the script's
  `Waiting for SSH...` loop), using `tart list` state + `tart ip`
  ([[tart-ip-lies]]), not `tart ip` alone.

This helper is the seam 020 and 030 both consume; design it so a Tier-2
linux/win golden can reuse it (no macOS-specific assumptions in the SSH layer
itself — those live in the boot leaves).

## Done when

- `testanyware vm create-golden --platform macos --dry-run` prints a boot plan
  (the 5-boot sequence) and mutates nothing, in both text and `--json`.
- The command is registered in `surface.rs`, has a schema, and the
  `cli-contract.rs` integration test still passes for the full surface.
- `russh`/`russh-sftp` added to `testanyware-vm` deps; the `SshSession` helper
  compiles with a unit test for `exec` parsing / host-key acceptance (a live
  round-trip is exercised by 020, not required here).
- Cross-check the build still cross-compiles via `zig cc`
  ([[linux-crosscheck-zig]]) — the russh-vs-C-dep rationale (ADR-0007) is moot if
  the dep tree regresses the cross-build. Quick `cargo check` on the linux target.

## Notes

- Real boot orchestration (actually cloning + booting the vanilla VM and running
  provisioning) is **020's** job; this leaf stops at a compiling skeleton + the
  SSH helper + a non-mutating dry-run. Keep the seam clean.
- `russh` API note for the implementer: it's lower-level than `ssh2` — you
  implement a `client::Handler` and drive `channel.exec()` / collect
  `ChannelMsg::Data`/`ExitStatus`. `russh-sftp` rides over a subsystem channel
  for `upload`. Budget the wrapper at ~100-150 lines.
