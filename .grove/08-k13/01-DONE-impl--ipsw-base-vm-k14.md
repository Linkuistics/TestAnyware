# ipsw-base-vm-k14

## Goal

The CLI can produce a macOS 27 base VM that is equivalent, for golden creation,
to what a Cirrus vanilla image is for tahoe: a stopped Tart VM whose `admin`
user has password `admin` and whose Remote Login (SSH) is on, so the existing
boot-1 provisioning can take over from it. It does this from Apple's IPSW,
unattended, by driving Setup Assistant over RFB + OCR.

## Context

- The node brief (`macos-27-golden-k13`) holds the reasoning and the human's
  decisions: no Cirrus macOS 27 vanilla image exists, so the human chose to
  automate the IPSW install in the CLI.
- `tart create --from-ipsw <path|URL|latest>` creates the VM (`--disk-size`
  default 50 GB; `latest` fetches the newest supported IPSW, which on this host
  should be 27.0.1 build 26A434). Check what `latest` resolves to before
  downloading about 18 GB, and cache the IPSW under
  `~/.local/share/testanyware/cache/` like the Windows ISOs.
- `recovery.rs` already drives a macOS guest's framebuffer with
  `RecoverySession::wait_for_text` and `settle`, and `golden.rs`'s
  `SetupVm`/`SshSession` own the VM-start and SSH side. Reuse them; read
  ADR-0008 first. Setup Assistant on 27 is unseen, so expect to capture its
  screens as you go and to tune the waits live.
- The vanilla image's first-contact credentials are `VANILLA_USER` /
  `VANILLA_PASS` (`admin`/`admin`) in `golden.rs`. The new stage must leave the
  guest satisfying those, so boot-1 needs no change.
- Setup Assistant creates the account through the GUI. Remote Login can be
  enabled by Setup Assistant-era means or afterwards with `systemsetup`/System
  Settings over VNC; find what works on 27 and say what you tried.
- Guests are driven only through VNC and SSH (the agent comes later). Keep the
  image minimal: no test-specific tooling. Never run host-affecting display
  helpers on the host.
- A step whose design is genuinely open is the `--version` selector: today it
  names a Cirrus vanilla tag. See the brief's "On the horizon". Decide it where
  the code is, record it here, and keep the tahoe/sequoia/sonoma path working.

## Done when

- A command path (a `vm create-golden` stage or a subcommand it uses; your
  call, recorded below) creates a Tart VM from the 27 IPSW and brings it to the
  base state above with no human input.
- Verified live on this Mac: `ssh admin@<ip>` with password `admin` works on
  the resulting VM. The VM is left stopped under a documented name, and no
  setup VM is left running (`vm list` / `tart list`).
- Unit tests pin the pure parts (name/selector mapping, IPSW resolution,
  whatever screen-matching tables the automation uses); the live automation is
  verified by actually running it, as the rest of golden creation is.
- The tahoe path (`--version tahoe`) still resolves to its vanilla image; its
  existing tests pass.
- The Setup Assistant screen sequence on 27 is recorded where the next reader
  will find it (a short note in the code's module docs, or a research note
  under `docs/research/`), since a later macOS will need it again.
- Last act: cut the next leaf of the node, `golden-from-27-base` (impl), with a
  body that carries what you learned (the base VM's name, how long it took,
  anything the provisioning is likely to trip on). Do not write the third and
  fourth children yet.

## Notes

- If the Setup Assistant automation proves bigger than one session, decompose
  this leaf again rather than running long. A good seam is "VM from IPSW and
  first boot reaches Setup Assistant, with screenshots recorded" then "walk
  the screens".
- If no human is reachable and a decision only they can make arises, stop and
  leave the leaf live.

## Decisions (running log)

- **Selector (2026-10-18).** `--version` stays one flag. An all-digit value
  (`27`) means "build the base from Apple's IPSW for that macOS major"; any
  other value is a Cirrus vanilla tag exactly as before (`tahoe`, `sequoia`,
  `sonoma`). Decided by the pure `ipsw_base::base_source`. No new flag, so no
  new schema or surface entry.
- **Command path.** Not a new subcommand: the base is a stage of
  `vm create-golden --platform macos --version 27` (`provision_boot1` clones
  from `ipsw_base::ensure_base_vm`, which builds the base if it does not exist).
  The base is kept as the stopped tart VM `testanyware-base-macos-27` so the
  install is paid once per release, and every golden clones from it. The live
  check of this leaf is an ignored live test (the `recovery-live` pattern).
- **IPSW source.** Apple's catalogue
  (`mesu.apple.com/.../com_apple_macOSIPSW.xml`) lists exactly one macOS
  restore image today, `UniversalMac_27.0.1_26A434_Restore.ipsw`, **24.8 GB**
  (the brief's 18 GB was low). Resolved by major from the catalogue rather than
  `tart create --from-ipsw latest`, so a future macOS 28 cannot silently become
  "27". Cached at `~/.local/share/testanyware/cache/macos-27.0.1-26A434.ipsw`
  by a resumable `curl -C -`.
- **What a base needs beyond SSH (2026-10-18).** Boot-1 runs `sudo` with no
  terminal and the agent is a per-user LaunchAgent, so a base must also have
  passwordless sudo and auto-login (plus no display sleep so an idle clone's
  framebuffer does not go black). Cirrus's vanilla image has these; a fresh
  IPSW install has none. `sysadminctl -autologin set` sets the user but fails to
  write the password (`SACSetAutoLoginPassword error:22`, no `/etc/kcpassword`),
  so the CLI writes `/etc/kcpassword` itself (XOR-obfuscated password, a known
  vector is unit-tested) and the reboot proved auto-login works. Remote Login is
  turned on from Terminal with `sudo launchctl load -w …/ssh.plist`, not
  `systemsetup` (which needs Full Disk Access).
- **Pure selector in its own ungated module.** `--dry-run` runs on every host,
  so `base_source`/`base_vm_name` live in `base_source.rs` (not macOS-gated);
  `ipsw_base` re-exports them. The dry-run plan gained a `base` line/field.
- **Screen table, not a script.** `setup_assistant.rs` lists screens as OCR
  anchors + actions and matches whatever is on screen, so an added or dropped
  screen is a table row, not a broken run. Sheets precede their pages.
- **Gatekeeper left on (2026-10-18).** Cirrus's image has assessments disabled
  (`spctl --status`); on 27 `spctl --global-disable` refuses ("needs to be
  confirmed in System Settings"), and nothing in golden creation needs it, so
  the base keeps Gatekeeper on rather than adding another driven GUI screen.
  A required step that cannot succeed on 27 would have aborted every build.
- **Reference check against the Tahoe golden.** A clone of
  `testanyware-golden-macos-tahoe` (macOS 26.5) read back through its agent:
  `/etc/sudoers.d/admin-nopasswd` = `admin ALL=(ALL) NOPASSWD: ALL`, a 12-byte
  `/etc/kcpassword`, `autoLoginUser admin`, `displaysleep`/`sleep`/`disksleep`
  0, screensaver idle 0 — the 27 base now matches all of it. (Its full name is
  "Managed via Tart", ours "Admin"; nothing reads it.)

## Outcome (2026-10-18)

**Delivered and verified live:**
- The IPSW path: catalogue → resumable download (24.8 GiB, about 25 minutes
  here) → `tart create --from-ipsw` (3 min 34 s) → Setup Assistant to desktop
  (about 5 minutes, 24 screens, run three times against clones of one fresh
  VM; two stalls found and fixed: the Location Services "Are you sure" sheet and
  the Select Your Time Zone screen, both only seen with Location Services
  left unticked) → Terminal opened through Spotlight (`Cmd+Space`) →
  `sudo launchctl load -w …/ssh.plist` typed with the sudo password.
- The result, checked after a reboot: macOS 27.0.1 (26A434), password SSH as
  `admin`/`admin`, passwordless sudo, `/etc/kcpassword` and an `admin` console
  session at boot (auto-login), no display or system sleep, `com.openssh.sshd`
  enabled. Left stopped as **`testanyware-base-macos-27`**; the Tahoe golden is
  untouched; `tart list` shows nothing running.
- `vm create-golden --platform macos --version 27` now clones that base
  (building it first if absent); `--version tahoe|sequoia|sonoma` is unchanged
  (unit-tested, and the dry-run plan names the base either way).

**Not verified from this session, and why.** The SSH half of
`configure_fresh_vm` (russh: wait for password SSH, run `base_config_commands`,
shut down) could not run here. From this harness, non-Apple binaries
(a Rust test binary, Homebrew's and Apple's Python, the `testanyware` CLI)
get `No route to host` connecting to a guest's `192.168.64.x` address, while
`nc`, `curl` and the system `ssh` reach the same address and port. That fits
macOS Local Network privacy, but the mechanism is not proven (the sandbox is not
the cause: it fails identically with it off). So the three config commands were
run, byte for byte as the Rust code generates them, over the system `ssh`, and
their effect was checked after a reboot. The russh plumbing is the one already
verified for Tahoe golden creation (ADR-0007); `task vm:ipsw-base-live` (or
`task vm:setup-assistant-live FRESH=<vm>`) from a terminal that can reach guests
closes it. The base was published by hand (`tart rename`) from that run.
