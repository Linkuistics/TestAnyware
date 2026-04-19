# Multi-VM Networking

TestAnyware is built to run many VMs concurrently on the same host.
Each VM gets its own id, its own spec file, and its own network
address — there's nothing shared, no port conflicts, no global state.

## The model

Every `testanyware vm start` produces:

- a unique id (`testanyware-<hex8>`, or `--id <custom>`),
- a per-VM spec at `$XDG_STATE_HOME/testanyware/vms/<id>.json`,
- a per-VM backend clone (tart clone or QEMU copy),
- VNC on a dynamically-assigned port,
- the agent on port 8648 inside each VM (reached via the VM's own LAN IP).

All commands target exactly one VM at a time via `--vm <id>` (or
`TESTANYWARE_VM_ID` in the current shell). There is no "default VM" —
you must name the one you want.

## Typical multi-VM workflow

```bash
# Start three VMs in parallel (each prints its own id)
vm1=$(testanyware vm start --platform macos)
vm2=$(testanyware vm start --platform linux)
vm3=$(testanyware vm start --platform windows)

# Drive them independently
testanyware --vm "$vm1" screenshot -o mac.png
testanyware --vm "$vm2" exec "uname -a"
testanyware --vm "$vm3" agent windows

# Tear all down
testanyware vm stop "$vm1"
testanyware vm stop "$vm2"
testanyware vm stop "$vm3"
```

## Addressing a single VM from many shells

If you want a subshell or CI step to pick up the VM a sibling step
started, export the id into the environment or write it to disk.

**Pattern 1 — environment variable (single shell tree):**

```bash
vmid=$(testanyware vm start)
export TESTANYWARE_VM_ID="$vmid"
# every child process sees TESTANYWARE_VM_ID and omits --vm
testanyware screenshot -o a.png
testanyware vm stop "$vmid"
```

**Pattern 2 — per-operation handle file (CI / long-running scripts):**

```bash
vmid=$(testanyware vm start)
printf '%s\n' "$vmid" > .testanyware-vmid

# later, from any fresh shell
testanyware screenshot --vm "$(cat .testanyware-vmid)" -o a.png

# teardown
testanyware vm stop "$(cat .testanyware-vmid)"
rm .testanyware-vmid
```

The filename is arbitrary; pick one that fits the operation and
`.gitignore` it.

## Recovering the id after a shell exit

The id is recoverable from disk even if the shell that captured it
exits:

```bash
ls "${XDG_STATE_HOME:-$HOME/.local/state}/testanyware/vms/"*.json
# → /Users/you/.local/state/testanyware/vms/testanyware-a3f7b2c1.json
```

The filename (minus `.json`) is the id.

## Port layout

No host ports are shared:

- **VNC ports** are dynamically assigned per-VM (tart picks a free
  high port; QEMU is configured with `-vnc 127.0.0.1:<auto>`). The
  actual port is written into `<id>.json → vnc.port`.
- **Agent port** is 8648 *inside the VM*. The host reaches it via
  `<vm-ip>:8648`, where `<vm-ip>` is allocated per VM by tart
  (`192.168.64.<n>`) or by the QEMU virtio-net setup.

Because the VNC port and agent host are per-VM and recorded in the
spec, there are no collisions regardless of how many VMs are running.

## Limits

There's no hard cap in TestAnyware itself — the limits are your
host's RAM, CPU, and network stack. A rough guide on a 32 GB machine:

| Platform | RAM per VM | Typical concurrent count |
|----------|------------|--------------------------|
| Linux (tart) | ~2 GB | 8-10 |
| macOS (tart) | ~4-6 GB | 3-4 |
| Windows (QEMU+swtpm) | ~4-6 GB | 2-3 |

For CI workloads, prefer Linux goldens where you have a choice —
boot time and footprint are significantly smaller.
