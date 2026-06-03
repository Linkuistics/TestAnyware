# 020-normal-boot-provisioning

**Kind:** work

## Goal

Port **boot 1** of the script — the normal-mode provisioning that runs over the
`010` SSH layer: clone+boot the vanilla VM, install the host pubkey, set macOS
defaults, wallpaper + hide widgets, Xcode CLT, Homebrew, and install the agent
binary + LaunchAgent plist.

## Context

- Builds directly on `010`'s `SshSession` helper and the command skeleton.
- Parity reference: `vm-create-golden-macos.sh` lines ~98–328 and ~673–675
  (`install_agent`). Map each `vm_ssh`/`vm_scp` call to `exec`/`upload`.
- VM bring-up: clone `ghcr.io/cirruslabs/macos-<version>-vanilla:latest` →
  `<setup-vm>` via `tart clone` (reuse `tart.rs` helpers / `run_tart`), boot
  `tart run --no-graphics --vnc-experimental` detached, wait for VNC line then
  for SSH-reachable. Delete any same-named existing golden first (script
  lines 98–105). Use a unique setup-VM id; ensure a cleanup path (stop+delete
  setup VM) on failure — the script's `trap cleanup EXIT`.
- Provisioning steps to port (each an `exec`, a few `upload`):
  - pubkey install + verify key-auth works without password (lines 175–189).
  - macOS defaults: disable session restore, hide desktop widgets (191–220).
  - solid-gray wallpaper: compile `provisioner/helpers/set-wallpaper.swift` on
    the **host** (`swiftc`), upload binary + generate the 1×1→scaled png on the
    guest, run it (199–215). If `swiftc` absent, warn + skip (parity).
  - Xcode CLT install (222–239), Homebrew install (241–250) — long, tolerate
    failure-with-warning exactly as the script does.
  - close Terminal + clear saved app state (252–257).
  - `install_agent` (261–328): resolve host CLI bin + agent bin (honor
    `TESTANYWARE_CLI_BIN_OVERRIDE` / `TESTANYWARE_AGENT_BIN_OVERRIDE`; else
    `brew --prefix testanyware/share/...`), scp agent → `/usr/local/bin`, scp
    plist → `~/Library/LaunchAgents/`.
- **Decision needed (small):** how the Rust handler resolves its own
  agent-binary path when run from a contributor build vs brew. Keep the override
  env vars for parity; document the default.

## Done when

- From a `vm create-golden` run, the setup VM boots and reaches the end of boot-1
  provisioning: pubkey auth verified, defaults applied, agent binary at
  `/usr/local/bin/testanyware-agent`, plist at `~/Library/LaunchAgents/`.
- Failure-tolerant steps (CLT, Homebrew, wallpaper) warn-and-continue, matching
  the script; hard-fail steps (pubkey auth, agent install) error with stable
  codes.
- Verified live on the Mac up to this point (the SIP/TCC recovery cycle is 030);
  i.e. boot-1 leaves the VM provisioned and SSH-reachable, ready for 030 to take
  over. Cheap to run ([[vm-costs]]).

## Notes

- This leaf does **not** do the recovery cycle, TCC grants, health gate, or
  final clone — those are `030`. The natural handoff is "setup VM is provisioned
  and running, pid + IP known."
- `COPYFILE_DISABLE` / tar gotchas ([[vm-ssh-from-harness]]) are mostly moot now
  that file transfer is `russh-sftp` not tar-over-ssh — but watch macOS xattrs on
  uploaded binaries (codesign/quarantine) when the agent is later launched.
