# provisioner/ — VM lifecycle scripts + autounattend

Bash scripts and Windows autounattend payload that build and run
TestAnyware's golden images. The lifecycle wrappers (`vm-start.sh`,
`vm-stop.sh`, etc.) are thin `exec testanyware vm …` stubs — the
Swift CLI is canonical.

## Working on this component

```bash
# Lifecycle — these just call through to testanyware vm ...
vmid=$(./provisioner/scripts/vm-start.sh)
./provisioner/scripts/vm-list.sh
./provisioner/scripts/vm-stop.sh "$vmid"

# Golden-image builds (long-running, destructive)
./provisioner/scripts/vm-create-golden-macos.sh
./provisioner/scripts/vm-create-golden-linux.sh
./provisioner/scripts/vm-create-golden-windows.sh --iso ~/Downloads/Win11_ARM64.iso
```

There is no separate test suite; the golden-build scripts are the
integration tests. `testanyware` itself must be on `PATH` for the
lifecycle wrappers to work.

## Notes

- `_testanyware-paths.sh` is the shell-side mirror of
  `cli/Sources/TestAnywareDriver/VM/VMPaths.swift`. Keep them in
  sync.
- `vm-create-golden-windows.sh` caches the Windows ISO under
  `${XDG_DATA_HOME:-~/.local/share}/testanyware/cache/` after first
  use.
- The autounattend payload under `autounattend/` includes the Windows
  agent publish dir and VirtIO drivers — both load-bearing.

See [`docs/components/provisioner.md`](../docs/components/provisioner.md)
for module layout, key files, and common pitfalls.
