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
│   ├── vm-delete.sh                      # wrapper around `testanyware vm delete`
│   ├── vm-create-golden-macos.sh         # build macOS golden (tart, ~10 min)
│   ├── vm-create-golden-linux.sh         # build Linux golden (tart, ~10 min)
│   └── vm-create-golden-windows.sh       # build Windows golden (QEMU+swtpm, 20-40 min)
└── autounattend/                         # Windows unattended-install XML + payload
```

## Key files

| File | Role |
|------|------|
| `scripts/vm-start.sh`, `vm-stop.sh`, `vm-list.sh`, `vm-delete.sh` | Thin `exec testanyware vm …` wrappers. Retained so existing callers (docs, CI, tests) keep working; the CLI is canonical. |
| `scripts/_testanyware-paths.sh` | Shell-side mirror of `VMPaths.swift`. Must stay in sync. |
| `scripts/vm-create-golden-macos.sh` | Builds `testanyware-golden-macos-tahoe`. Downloads vanilla macOS Tahoe from Cirrus Labs; disables/re-enables SIP to install the TCC grant for the agent. |
| `scripts/vm-create-golden-linux.sh` | Builds `testanyware-golden-linux-24.04`. Installs `ubuntu-desktop-minimal`, AT-SPI2, the agent, and a systemd user service. |
| `scripts/vm-create-golden-windows.sh` | Builds `testanyware-golden-windows-11` under `$XDG_DATA_HOME/testanyware/golden/`. Orchestrates QEMU + swtpm through the Windows 11 ARM64 installer; requires the ISO on first run. |
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
./provisioner/scripts/vm-create-golden-macos.sh
./provisioner/scripts/vm-create-golden-linux.sh
./provisioner/scripts/vm-create-golden-windows.sh --iso ~/Downloads/Win11_ARM64.iso
```

## Common pitfalls

- **SIP cycle during macOS golden build.** The script reboots the VM
  into Recovery to disable SIP, writes a TCC grant for the agent
  tied to its code-signing requirement, then re-enables SIP. If this
  step is interrupted, the resulting image may have SIP disabled —
  rebuild from scratch.
- **Windows ISO cache.** Lives at
  `${XDG_DATA_HOME:-~/.local/share}/testanyware/cache/`. First
  invocation of `vm-create-golden-windows.sh` requires `--iso <path>`;
  subsequent invocations reuse the cache.
- **Autounattend payload size.** The autounattend media holds the
  agent's entire `publish/` dir plus VirtIO drivers. If you add
  large binaries, the ISO may exceed the media-drive size QEMU is
  configured to present to the installer.
- **Shell helpers mirror Swift helpers.** `_testanyware-paths.sh`
  must match `VMPaths.swift` because the golden-image scripts run
  before the Swift CLI exists and still need to know where clones
  and caches live.
