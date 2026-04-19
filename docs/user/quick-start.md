# Quick Start

Get from a fresh clone to your first screenshot in about 15 minutes.

## Prerequisites

- macOS 14+ on Apple Silicon
- Xcode 16+ (`swift --version` should report 6.0+)
- `tart` — `brew install cirruslabs/cli/tart`

That's enough to run macOS VMs. Linux VMs need the same tools; Windows
VMs additionally need `qemu`, `swtpm`, and .NET 9 — see the
[full requirements in `README.md`](../../README.md#requirements).

## 1. Build the CLI

```bash
cd cli
swift build -c release
```

Binary appears at `cli/.build/release/testanyware`. For convenience,
symlink into your `PATH`:

```bash
ln -sf "$PWD/.build/release/testanyware" /usr/local/bin/testanyware
```

## 2. Create the macOS golden image (one-time, ~10 minutes)

```bash
./provisioner/scripts/vm-create-golden-macos.sh
```

The script downloads a vanilla macOS Tahoe image, installs the
TestAnyware agent, grants accessibility, and finalises the golden. You
only need to do this once per host.

## 3. Start a VM

```bash
vmid=$(testanyware vm start --viewer)
export TESTANYWARE_VM_ID="$vmid"   # lets you omit --vm on every command
```

`--viewer` opens a macOS VNC viewer window so you can watch the VM
boot. You'll see `testanyware-<hex8>` printed on stdout — that's the
instance id.

## 4. Take your first screenshot

```bash
testanyware screenshot -o first.png
```

Done. Open `first.png` — that's your VM desktop.

## 5. Try a few more things

```bash
# What's the display size?
testanyware screen-size

# Type into the foreground app
testanyware input key cmd-space         # open Spotlight... wait, that's wrong
testanyware input key space --modifiers cmd
testanyware input type "Terminal"
testanyware input key return

# Run a command inside the VM
testanyware exec "uname -a"

# Look at the accessibility tree
testanyware agent windows

# Find text on screen
testanyware find-text "Terminal"
```

## 6. Stop the VM

```bash
testanyware vm stop "$TESTANYWARE_VM_ID"
unset TESTANYWARE_VM_ID
```

## Next steps

- [`cli-commands.md`](../reference/cli-commands.md) — every command
  and flag.
- [`key-names.md`](../reference/key-names.md) — accepted keys and
  modifiers.
- [`troubleshooting.md`](troubleshooting.md) — if something goes wrong.
- [`golden-images.md`](golden-images.md) — what's inside the golden
  images.
- [`multi-vm-networking.md`](multi-vm-networking.md) — running several
  VMs concurrently.
