# Golden Images

TestAnyware boots every VM from a pre-built golden image. You create the golden
image of each platform once. Creation takes 10 to 40 minutes, by OS.

## Shared properties (all platforms)

All golden images share these properties:

- **User** — `admin`, with autologin to the desktop session.
- **testanyware-agent** — an HTTP service on port 8648. It starts automatically at boot.
- **Solid grey wallpaper** — a clean background for screenshot analysis.
- **Notifications and widgets disabled** — no visual clutter during tests.

## macOS (`testanyware-golden-macos-27`)

- **The latest release of macOS 27** (Apple Silicon). Golden creation clones a
  base VM, `testanyware-base-macos-<version>-<build>`, such as
  `testanyware-base-macos-27.2.1-26B131`. It builds that base from Apple's IPSW
  when the base does not exist. This is the default macOS golden: `vm start
  --platform macos` clones it when you pass no `--base`.
- **Only macOS 27 or a later major.** `vm create-golden --platform macos
  --version` takes a major number of 27 or later. A Cirrus Labs tag (`tahoe`,
  `sequoia`, `sonoma`) or a major below 27 fails with a usage error. The
  command fails before it clones or downloads anything.
- **Agent autostart** — a LaunchAgent at `/usr/local/bin/testanyware-agent`
  (label `com.linkuistics.testanyware-agent`).
- **Accessibility** — a TCC grant in the system TCC database, with the
  code-signing requirement of the agent. Image creation disables SIP and
  enables it again to write the grant.
- **Package manager** — Homebrew (`/opt/homebrew/bin/brew`).
- **Developer tools** — Xcode Command Line Tools (`swift`, `clang`, `git`, `make`).
- **No SSH at run time** — golden creation uses Remote Login. It disables
  Remote Login (`systemsetup -f -setremotelogin off`) before the final
  shutdown. Clones communicate through the agent on port 8648.
- **Session restore disabled** — apps do not open their old windows again.
- **SIP enabled** — the standard security state after image creation.

Golden creation boots its VM several times, and macOS runs two guests at most.
When the TestAnyware app runs, a macOS golden creation asks it for a slot before
it changes anything. It waits in the queue of `vm start --wait`, with no bound,
and stderr shows its place and the VMs that hold the slots. Press Ctrl-C to end
the wait. The golden creation then holds the slot until its setup VM has
stopped, so no start takes the slot between two boots.

`testanyware vm slots` shows the hold with the kind `golden`. With `macos_slots`
at 0 in `config.rhai`, golden creation fails with `VM_LIMIT_REACHED`. If the app
stops while the golden creation holds its slot, it continues and writes one line
to stderr. If the app stops while it waits, it fails with `APP_UNREACHABLE`, and
it has changed nothing. When the app does not run, golden creation takes no
slot.

When another guest holds the second slot, the framework can refuse a boot.
Golden creation sees that at once, and it tries the boot again every 3 s for
30 s. The framework can free the slot of the previous boot a moment after it
ends. If the refusal continues, golden creation fails with `VM_LIMIT_REACHED`
and `refused_by` `framework`, and it deletes its setup VM. Stop a guest with
`testanyware vm stop`, then run `vm create-golden` again. Linux and Windows
golden creation take no slot.

A guest that the app cannot see, such as a guest of another user, can take the
free slot between two boots. The app counts only the guests that it sees.
Golden creation then fails with the error above.

## Linux (`testanyware-golden-linux-24.04`)

- **Ubuntu 24.04 Desktop** (ARM64, from the Cirrus Labs vanilla image and
  `ubuntu-desktop-minimal`).
- **Agent autostart** — a systemd user service
  (`testanyware-agent.service`).
- **Accessibility** — AT-SPI2 enabled, `python3-pyatspi` for the bindings,
  and `xdotool` as the fallback for window management.
- **Package manager** — apt.
- **No SSH at run time** — golden creation uses `openssh-server`. It disables
  and masks the service (`systemctl disable ssh && systemctl mask ssh`)
  immediately before the final shutdown. Clones communicate through the agent
  on port 8648.
- **Silent boot** — GRUB hidden, a Plymouth splash, and no console output in
  text mode.
- **Screen lock and blanking disabled** — no interruptions during tests.
- **NetworkManager** — configured through netplan. It replaces the
  systemd-networkd of the base image.
- **50 GB disk** — golden creation grows the disk of the 20 GB Cirrus image
  to 50 GB before its first boot. cloud-init in each clone grows `/` at boot.
  A clone has about 36 GB free, against 8.8 GB on the 20 GB disk. The host
  uses no space for it until the guest writes. Golden creation reads the size
  of `/` over SSH, and it fails the build when the size is under 40 GB.

  A golden built before this change keeps its 20 GB disk. Give it the space
  without a rebuild:

  ```bash
  tart set testanyware-golden-linux-24.04 --disk-size 50
  ```

  tart can only grow a disk, so you cannot undo this. To check a golden from
  a checkout, run
  `task vm:linux-golden-disk-live GOLDEN=testanyware-golden-linux-24.04`. It
  starts a clone and reports the free space of `/`.

## Windows (`testanyware-golden-windows-11`)

- **Windows 11 Pro** (ARM64, installed under QEMU from a Microsoft evaluation
  ISO).
- **Agent autostart** — a Task Scheduler logon task (`TestAnywareAgent`).
- **Password never expires** — the password of `admin` (auto-logon) never
  expires. So a golden older than the Windows password age of 42 days still
  logs in and starts the agent. Golden creation verifies this with
  `net user admin`.
- **Accessibility** — UI Automation through FlaUI (built into Windows).
- **Package manager** — Chocolatey.
- **No SSH** — the agent binary comes from the autounattend media. All
  communication goes through the HTTP service of the agent.
- **First-logon animation disabled** — clones boot directly to the desktop,
  with no OOBE.
- **UEFI and TPM 2.0** — standard Windows 11 secure boot through swtpm. When
  the QEMU of the host has no `tpm-tis-device`, golden creation builds the
  golden without a TPM, and its clones start without one. When `swtpm` is
  missing on a host that could supply a TPM, the result is the error
  `SWTPM_MISSING`, not a silent omission.
- **VirtIO networking** — setup installs the virtio-net-pci driver.

## Keeping goldens current

A golden holds the latest release of its major at the time of its build
(ADR-0027). Its name names the major only, so a refresh keeps the name. To
refresh a golden, run `vm create-golden` again. The earlier golden goes only
when the new golden is ready, so a failed build keeps it. Nothing checks at run
time whether a golden is older than the latest release.

- **macOS.** The release is the newest one that Apple's IPSW catalogue lists for
  the device `VirtualMac2,1` in the major. The name of the base VM carries that
  release, so `tart list` shows it. When the base of the release does not exist,
  golden creation downloads the IPSW (about 25 GB) and builds the base. This
  drives Setup Assistant, which takes about 10 minutes more. When the base is
  ready, golden creation deletes each other base of the major, such as
  `testanyware-base-macos-27` of TestAnyware 2.9.0. It also deletes each cached
  IPSW of another release. If the catalogue lists no `VirtualMac2,1` release of
  the major, golden creation fails with `GOLDEN_CREATE_FAILED`, and it never
  uses another build.
- **Linux.** The build installs the current Ubuntu updates of the release in
  the name.
- **Windows.** The golden holds the release of the ISO that you supply.
  TestAnyware cannot download that ISO, and it runs no Windows Update. So the
  golden misses each cumulative update that Microsoft releases after its ISO.
  To refresh it, pass a current ISO with `--iso`.

To see what a macOS refresh does before you run it, use the dry run:

```bash
testanyware vm create-golden --platform macos --dry-run
```

It reads the catalogue and `tart list`. It names the release and its build, and
it says whether the base of that release exists. It names each older base that
the run deletes, and it says when the run replaces a golden. The dry run and the
run both need the network. Without it, both fail with the same error.

## Creating the golden images

```bash
testanyware vm create-golden --platform macos
testanyware vm create-golden --platform linux
```

For Windows, first download the Windows 11 ARM64 evaluation ISO from
[Microsoft](https://www.microsoft.com/en-us/software-download/windows11arm64).
Then pass it to the in-process command. The command builds
`testanyware-golden-windows-11` with QEMU and swtpm, and it provisions over the
in-VM agent, with no SSH:

```bash
testanyware vm create-golden --platform windows --iso ~/Downloads/Win11_ARM64.iso
```

The command caches the ISO at its first use, so later runs do not need `--iso`.
`autounattend.xml` automates the whole Windows installation. It typically takes
20 to 40 minutes.

## Rebuilding after an agent change

Each golden image holds the in-VM agent. So a **breaking change to the agent
protocol needs a rebuild of every golden**. A clone of a stale golden runs the
old agent, and it fails against a matching new host CLI. There is no version
negotiation (see ADR-0001, the change of `/upload` and `/download` to streams).

The create commands need no change for this, because each one takes the agent
from the *current* source. The macOS command copies the built
`testanyware-agent` binary over SFTP. Point it at a contributor build with
`TESTANYWARE_AGENT_BIN_OVERRIDE=…`. The Linux command copies the Python source
in `agents/linux`. So, to rebuild, build the new agents, and then run
`vm create-golden` for each platform again. A rebuild also refreshes the golden
to the latest release of its major.

The Windows golden needs a rebuild on a Windows
host. The create flow needs the `net9.0-windows` and FlaUI agent, which cannot
run on Apple Silicon.

## Where they live

- **macOS and Linux goldens:** tart manages them, under `~/.tart/vms/`.
- **Windows goldens:** `${XDG_DATA_HOME:-~/.local/share}/testanyware/golden/`.

`testanyware vm list` shows all goldens (tart and QEMU) and their running
clones. `testanyware vm delete <name>` deletes one.
