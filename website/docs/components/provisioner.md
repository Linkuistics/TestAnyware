---
title: Provisioner
---

# Component: `provisioner/` — VM lifecycle scripts + autounattend

Bash scripts and autounattend XML that build and run TestAnyware's
golden images. Scripts are thin wrappers around `testanyware vm ...`
where possible — the CLI is the source of truth for lifecycle logic.

## Layout

```
provisioner/
├── scripts/
│   ├── _testanyware-paths.sh             # XDG path helpers (matches VMPaths.swift)
│   ├── vm-start.sh                       # wrapper around `testanyware vm start`
│   ├── vm-stop.sh                        # wrapper around `testanyware vm stop`
│   ├── vm-list.sh                        # wrapper around `testanyware vm list`
│   └── vm-delete.sh                      # wrapper around `testanyware vm delete`
└── autounattend/                         # Windows unattended-install XML + payload
```

## Key files

| File | Role |
|------|------|
| `scripts/vm-start.sh`, `vm-stop.sh`, `vm-list.sh`, `vm-delete.sh` | Thin `exec testanyware vm …` wrappers. Retained so existing callers (docs, CI, tests) keep working; the CLI is canonical. |
| `scripts/_testanyware-paths.sh` | Shell-side mirror of `VMPaths.swift`. Must stay in sync. |
| `testanyware vm create-golden --platform macos` | Builds `testanyware-golden-macos-tahoe`. In-process CLI command (no script): downloads vanilla macOS Tahoe from Cirrus Labs; disables/re-enables SIP to install the TCC grant for the agent. |
| `testanyware vm create-golden --platform linux` | Builds `testanyware-golden-linux-24.04`. In-process CLI command (no script): provisions Ubuntu 24.04 via tart, installs `ubuntu-desktop-minimal`, AT-SPI2, the agent, and a systemd user service over 2 normal boots. |
| `testanyware vm create-golden --platform windows` | Builds `testanyware-golden-windows-11` under `$XDG_DATA_HOME/testanyware/golden/`. In-process CLI command (no script): orchestrates QEMU + swtpm through a Windows 11 ARM64 Microsoft evaluation ISO, provisioning over the in-VM agent (no SSH); requires `--iso <path>` on first run. |
| `autounattend/` | Windows unattended-install XML + payload. The agent's `publish` dir and a Task Scheduler xml are injected here. |

## Build / test

These scripts are not "built" — they run directly. Smoke-testing them:

```bash
# Sanity — prints a VM id, then removes it
vmid=$(./provisioner/scripts/vm-start.sh)
./provisioner/scripts/vm-stop.sh "$vmid"

# List all goldens + running clones
./provisioner/scripts/vm-list.sh
```

Golden-image creation is long and destructive; don't run it casually.

```bash
testanyware vm create-golden --platform macos
testanyware vm create-golden --platform linux
testanyware vm create-golden --platform windows --iso ~/Downloads/Win11_ARM64.iso
```

## Common pitfalls

- **SIP cycle during macOS golden build.** `testanyware vm create-golden
  --platform macos` reboots the VM into Recovery to disable SIP, writes a
  TCC grant for the agent tied to its code-signing requirement, then
  re-enables SIP. If this step is interrupted, the resulting image may have
  SIP disabled — rebuild from scratch.
- **Windows ISO cache.** Lives at
  `${XDG_DATA_HOME:-~/.local/share}/testanyware/cache/`. First
  invocation of `testanyware vm create-golden --platform windows`
  requires `--iso <path>`; subsequent invocations reuse the cache.
- **Autounattend payload size.** The autounattend media holds the
  agent's entire `publish/` dir plus VirtIO drivers. If you add
  large binaries, the ISO may exceed the media-drive size QEMU is
  configured to present to the installer.
- **Shell helpers mirror the CLI's path logic.** `_testanyware-paths.sh`
  must match `cli-rs/crates/testanyware-vm/src/paths.rs` because the
  golden-image scripts run before the host CLI is on `PATH` and still need
  to know where clones and caches live.
