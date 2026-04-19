# Golden Images

TestAnyware boots every VM from a pre-built golden image. Golden image
creation is a one-time setup step per platform and takes 10-40 minutes
depending on the OS.

## Shared properties (all platforms)

All golden images share these properties:

- **User** — `admin`, with autologin to desktop session.
- **testanyware-agent** — HTTP service on port 8648, started automatically on boot.
- **Solid gray wallpaper** — clean background for screenshot analysis.
- **Notifications and widgets disabled** — no visual clutter during tests.

## macOS (`testanyware-golden-macos-tahoe`)

- **macOS Tahoe** (Apple Silicon, via Cirrus Labs vanilla image).
- **Agent autostart** — LaunchAgent at `/usr/local/bin/testanyware-agent`
  (label `com.linkuistics.testanyware-agent`).
- **Accessibility** — TCC grant via system TCC database with code
  signing requirement (SIP disable/enable cycle during image creation).
- **Package manager** — Homebrew (`/opt/homebrew/bin/brew`).
- **Dev tools** — Xcode Command Line Tools (`swift`, `clang`, `git`, `make`).
- **SSH key auth** — host's public key in `authorized_keys` (used during
  golden image creation).
- **Session restore disabled** — apps don't reopen old windows.
- **SIP enabled** — standard security posture after image creation.

## Linux (`testanyware-golden-linux-24.04`)

- **Ubuntu 24.04 Desktop** (ARM64, via Cirrus Labs vanilla image +
  `ubuntu-desktop-minimal`).
- **Agent autostart** — systemd user service
  (`testanyware-agent.service`).
- **Accessibility** — AT-SPI2 enabled, `python3-pyatspi` for bindings,
  `xdotool` for window management fallback.
- **Package manager** — apt.
- **SSH key auth** — host's public key in `authorized_keys` (used during
  golden image creation).
- **Silent boot** — GRUB hidden, Plymouth splash, no text-mode console
  output.
- **Screen lock and blanking disabled** — no interruptions during tests.
- **NetworkManager** — configured via netplan (replaces
  systemd-networkd from base image).

## Windows (`testanyware-golden-windows-11`)

- **Windows 11 Pro** (ARM64, installed from Microsoft evaluation ISO via
  QEMU).
- **Agent autostart** — Task Scheduler logon task (`TestAnywareAgent`).
- **Accessibility** — UI Automation via FlaUI (built into Windows).
- **Package manager** — Chocolatey.
- **No SSH** — agent binary installed from autounattend media; all
  communication via agent HTTP.
- **First-logon animation disabled** — clones boot straight to desktop
  without OOBE.
- **UEFI + TPM 2.0** — standard Windows 11 secure boot via swtpm.
- **VirtIO networking** — virtio-net-pci driver installed during setup.

## Creating the golden images

```bash
./provisioner/scripts/vm-create-golden-macos.sh
./provisioner/scripts/vm-create-golden-linux.sh
```

For Windows, first download the Windows 11 ARM64 ISO from
[Microsoft](https://www.microsoft.com/en-us/software-download/windows11arm64),
then pass it to the script:

```bash
./provisioner/scripts/vm-create-golden-windows.sh --iso ~/Downloads/Win11_ARM64.iso
```

The ISO is cached after first use — subsequent runs don't need `--iso`.
The Windows installation is fully automated via `autounattend.xml`
(typical time: 20-40 minutes).

## Where they live

- **macOS / Linux goldens:** tart-managed, under `~/.tart/vms/`.
- **Windows goldens:** `${XDG_DATA_HOME:-~/.local/share}/testanyware/golden/`.

Use `testanyware vm list` to see all goldens (tart + QEMU) and their
running clones. Use `testanyware vm delete <name>` to remove one.
