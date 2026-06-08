# provisioner/ — VM lifecycle scripts + autounattend

Bash scripts and Windows autounattend payload that build and run
TestAnyware's golden images. The lifecycle wrappers (`vm-start.sh`,
`vm-stop.sh`, etc.) are thin `exec testanyware vm …` stubs — the
host CLI (`cli-rs/`) is canonical.

## Working on this component

```bash
# Lifecycle — these just call through to testanyware vm ...
vmid=$(./provisioner/scripts/vm-start.sh)
./provisioner/scripts/vm-list.sh
./provisioner/scripts/vm-stop.sh "$vmid"

# Golden-image builds (long-running, destructive)
testanyware vm create-golden --platform macos   # macOS is built into the CLI
testanyware vm create-golden --platform linux   # Linux is built into the CLI
testanyware vm create-golden --platform windows --iso ~/Downloads/Win11_ARM64.iso  # Windows is built into the CLI
```

All three goldens (macOS, Linux, Windows) are in-process
`testanyware vm create-golden --platform <os>` commands — no golden
shell scripts ship any more; the only bundled script left is the
runtime path helper `_testanyware-paths.sh`. The Windows golden builds
Windows 11 ARM64 (`testanyware-golden-windows-11`) via QEMU+swtpm from
a Microsoft evaluation ISO (first run needs `--iso <path>`),
provisioning over the in-VM agent (no SSH).

There is no separate test suite. `testanyware` itself must be on `PATH`
for the lifecycle wrappers to work.

## Notes

- `_testanyware-paths.sh` is the shell-side mirror of
  `cli-rs/crates/testanyware-vm/src/paths.rs`. Keep them in
  sync.
- `testanyware vm create-golden --platform windows` caches the Windows
  ISO under `${XDG_DATA_HOME:-~/.local/share}/testanyware/cache/` after
  first use.
- The autounattend payload under `autounattend/` includes the Windows
  agent publish dir and VirtIO drivers — both load-bearing.

See [`docs/components/provisioner.md`](../docs/components/provisioner.md)
for module layout, key files, and common pitfalls.
