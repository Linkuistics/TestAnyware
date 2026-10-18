# golden-from-27-base-k15

## Goal

`vm create-golden --platform macos --version 27` takes the macOS 27 base VM
through the existing provisioning and recovery/finalize stages and leaves
`testanyware-golden-macos-27`, and a clone of it reaches a Healthy agent. Expect
27 to break some of the provisioning; fix what breaks in the CLI, or cut a leaf
for what is not this leaf's.

## Context

- **The base exists.** `testanyware-base-macos-27` is a stopped tart VM:
  macOS 27.0.1 (26A434), `admin`/`admin`, Remote Login on, passwordless sudo,
  auto-login, no display or system sleep, Gatekeeper **on**, 1024×768 display,
  region US/English, Location Services off, hostname
  `Admins-Virtual-Machine`. `provision_boot1` already clones it when `--version`
  is all digits (`base_source`), and builds it first if absent, so there is
  nothing to wire. Built by `ipsw-base-vm-k14`; its running log and the module
  docs of `setup_assistant.rs` / `ipsw_base.rs` hold the detail.
- **Blocker to settle first — reaching a guest from this harness.** From the
  harness session that built the base, non-Apple binaries (the Rust CLI and test
  binaries, Python) got `No route to host` to a tart guest's `192.168.64.x`
  address, while `nc`, `curl` and the system `ssh` reached it. Consistent with
  macOS Local Network privacy; not proven. Boot-1 provisioning and the finalize
  stage are russh over exactly that route, and `vm start`'s agent wait uses it
  too (a Tahoe clone started here printed "agent did not reach health" while its
  `/health` answered to `curl`). VNC is on loopback and works. So: **run the
  build from a terminal that may reach guest IPs** (the human's own), or have
  the human grant Local Network access to what launches the harness. First
  step: check `nc -z <guest ip> 22` against a started clone and then a
  minimal russh connect, and if it fails, stop and ask the human which of the
  two they want. Do not work around it in product code.
- **Timing.** The base build was paid once: IPSW download about 25 minutes
  (24.8 GiB, cached at
  `~/.local/share/testanyware/cache/macos-27.0.1-26A434.ipsw`), install 3.5
  minutes, Setup Assistant about 5. From the base, a golden is the Tahoe
  pipeline's cost (about 10 to 20 minutes with the CLT and Homebrew installs).
- **Likely to trip on 27** (none of this was exercised):
  - Boot-1: `softwareupdate -l` and `parse_clt_label` for the Command Line
    Tools; the Homebrew installer; `defaults write com.apple.WindowManager
    StandardHideWidgets` (the desktop showed Calendar/Photos widgets);
    `swiftc` compiling the wallpaper helper.
  - Recovery (ADR-0008): the startup picker's "Options", the Utilities menu,
    the csrutil prompts `y/n`, `Authorized`, `Password`, and the result line,
    all read off Tahoe. The recovery driver needs a human-visible check on 27;
    `RecoverySession` is the same one `setup_assistant.rs` uses.
  - Finalize: TCC.db schema (`access` table columns changed before; the SQL is
    in `finalize.rs`), `tccd`, the agent's LaunchAgent starting in the
    auto-login session, and the health gate on port 8648.
  - Gatekeeper is on in the base (Cirrus's has it off; `spctl --global-disable`
    refuses on 27). If the agent or a helper is blocked by assessment, say so
    rather than adding a GUI step.
- **One macOS VM at a time.** With the 27 build VM running, a Tahoe clone
  twice went black and then exited by itself within minutes; run alone it
  stayed up and drawn for ten minutes. Cause unknown (concurrency is the
  suspect). A first VNC frame of a booting guest is a black 1280×720
  placeholder until the guest draws; wait for the real size before judging.
- Recipes already in the Taskfile: `task vm:ipsw-base-live`,
  `task vm:setup-assistant-live FRESH=<vm>`.

## Done when

- `testanyware vm create-golden --platform macos --version 27` completes and
  leaves `testanyware-golden-macos-27` (the default name).
- A clone of it starts (`vm start --platform macos --base
  testanyware-golden-macos-27`) and reaches a Healthy agent with no help over
  VNC; the agent reports `accessible: true`.
- `testanyware-golden-macos-tahoe` and `testanyware-base-macos-27` are intact;
  nothing is left running (`vm list`, `tart list`).
- Whatever 27 broke is fixed with a unit test where the fix is pure, or cut as
  a leaf under this node if it is not this leaf's. The golden ends committed
  and verified; this is a long build, so end the session on it.
- Last act: cut `checks-27` (impl), carrying what you learned. Do not write the
  fourth child yet.

## Notes

- Guests are driven only through the In-VM agent and VNC (and the SSH of
  provisioning); keep the image minimal.
- If the Local Network question needs the human and no human is reachable,
  stop and leave this leaf live.
