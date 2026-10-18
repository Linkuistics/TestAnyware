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
