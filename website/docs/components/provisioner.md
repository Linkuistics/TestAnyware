---
title: Provisioner
---

# Component: `provisioner/` — VM lifecycle scripts + autounattend

`provisioner/` holds the Bash scripts and the autounattend XML that build and run
the golden images of TestAnyware. Where possible, a script is a thin wrapper
around `testanyware vm ...`. The CLI is the source of truth for the lifecycle
logic.

## Layout

```
provisioner/
├── scripts/
│   ├── _testanyware-paths.sh             # XDG path helpers (matches VMPaths.swift)
│   ├── vm-start.sh                       # wrapper around `testanyware vm start`
│   ├── vm-stop.sh                        # wrapper around `testanyware vm stop`
│   ├── vm-list.sh                        # wrapper around `testanyware vm list`
│   └── vm-delete.sh                      # wrapper around `testanyware vm delete`
└── helpers/                              # files that golden creation embeds or uploads, such as autounattend.xml
```

## Key files

| File | Role |
|------|------|
| `scripts/vm-start.sh`, `vm-stop.sh`, `vm-list.sh`, `vm-delete.sh` | Thin `exec testanyware vm …` wrappers. They stay, so that the existing callers (docs, CI, tests) keep working. The CLI is canonical. |
| `scripts/_testanyware-paths.sh` | The shell copy of `VMPaths.swift`. Keep the two in step. |
| `testanyware vm create-golden --platform macos` | Builds or refreshes `testanyware-golden-macos-27`. It is an in-process CLI command, with no script. It clones the base VM of the latest release of the major, `testanyware-base-macos-<version>-<build>`. When that base does not exist, the command builds it from Apple's IPSW, and then it deletes the older bases and IPSWs (ADR-0027). `--version` takes a macOS major of 27 or later. The command disables SIP and enables it again to install the TCC grant of the agent. |
| `testanyware vm create-golden --platform linux` | Builds `testanyware-golden-linux-24.04`. It is an in-process CLI command, with no script. It provisions Ubuntu 24.04 through tart. Over 2 normal boots, it installs `ubuntu-desktop-minimal`, AT-SPI2, the agent and a systemd user service. |
| `testanyware vm create-golden --platform windows` | Builds `testanyware-golden-windows-11` under `$XDG_DATA_HOME/testanyware/golden/`. It is an in-process CLI command, with no script. It drives QEMU and swtpm through a Windows 11 ARM64 Microsoft evaluation ISO, and it provisions over the in-VM agent, with no SSH. The password of `admin` never expires, and the command checks this in the guest with `net user admin`. The first run needs `--iso <path>`. |
| `helpers/autounattend.xml` | The Windows unattended-install XML. The Windows golden command writes it to the install media, with the setup scripts of `helpers/`, the agent binary and the VirtIO drivers. |

## Build / test

These scripts need no build. They run directly. To smoke-test them:

```bash
# Sanity — prints a VM id, then removes it
vmid=$(./provisioner/scripts/vm-start.sh)
./provisioner/scripts/vm-stop.sh "$vmid"

# List all goldens + running clones
./provisioner/scripts/vm-list.sh
```

Golden-image creation is long and destructive. Do not run it casually.

```bash
testanyware vm create-golden --platform macos
testanyware vm create-golden --platform linux
testanyware vm create-golden --platform windows --iso ~/Downloads/Win11_ARM64.iso
```

## Common pitfalls

- **The SIP cycle of the macOS golden build.** `testanyware vm create-golden
  --platform macos` reboots the VM into Recovery to disable SIP. It writes a
  TCC grant for the agent, tied to the code-signing requirement of the agent.
  Then it enables SIP again. If an interruption stops this step, the image can
  have SIP disabled. Rebuild it from the start.
- **The Windows ISO cache.** It is at
  `${XDG_DATA_HOME:-~/.local/share}/testanyware/cache/`. The first run of
  `testanyware vm create-golden --platform windows` needs `--iso <path>`.
  Later runs use the cache.
- **The size of the autounattend payload.** The autounattend media holds the
  whole `publish/` directory of the agent and the VirtIO drivers. If you add
  large binaries, the ISO can become larger than the media drive that QEMU
  presents to the installer.
- **The shell helpers copy the path logic of the CLI.** `_testanyware-paths.sh`
  must match `cli-rs/crates/testanyware-vm/src/paths.rs`. The golden-image
  scripts run before the host CLI is on `PATH`, and they still need to know
  where clones and caches are.
