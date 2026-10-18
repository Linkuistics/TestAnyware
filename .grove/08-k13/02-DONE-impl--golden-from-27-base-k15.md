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

## Decisions (running log)

- **Local Network blocker confirmed (2026-10-18).** Cloned
  `testanyware-base-macos-27` to a throwaway `probe-27-net`, started it, and got
  `192.168.64.7`. From this harness: `nc -z` to port 22 succeeded; Homebrew
  Python `connect()` and a freshly compiled Rust `std::net::TcpStream` both got
  `No route to host (os error 65)`; the system `ssh` reached the host (it got as
  far as an auth refusal). Same split as `ipsw-base-vm-k14` saw, so russh (boot-1
  provisioning, finalize, `vm start`'s agent wait) cannot run from here. Probe VM
  deleted; `tart list` shows no clone and nothing running. Not worked around in
  product code, as the leaf says. Waiting on the human: build from a terminal
  that may reach guest IPs, or grant Local Network access to what launches the
  harness. No build run yet.
- **Terminal run 1 (2026-10-18, the human's terminal).** The installed
  Homebrew `testanyware` 2.1.0 predates `ipsw-base-vm-k14`, so it read `27` as a
  Cirrus tag and failed on `macos-27-vanilla` (403). Not a guest-access fault.
  The branch binary `cli-rs/target/debug/testanyware` was run instead. From that
  terminal russh reached the guest: **the Local Network blocker is specific to the
  harness, not the CLI.** Boot 1, `csrutil disable`, the TCC grant and its
  after-grant check, and `csrutil enable` all ran on 27. The run then failed at
  "TCC verification failed (after SIP re-enable): Accessibility=missing, Full Disk
  Access=missing", and teardown deleted the setup VM.
- **Cause of that failure: an unreadable database, not a missing grant
  (2026-10-18).** On a clone of the base (SIP on), `sudo sqlite3
  "/Library/Application Support/com.apple.TCC/TCC.db" "select count(*) from
  access;"` over `ssh` exits 1 with `unable to open database ...: authorization
  denied`. `tcc_row` dropped stderr and read the empty stdout as "no row". So the
  after-re-enable check could never pass on 27 whatever the grants held. Whether
  the grants survived the SIP re-enable is **not yet observed**; the health gate
  below is what shows it.
- **Fix.** `finalize.rs`: `classify_tcc_read` separates `Unreadable` from `Absent`;
  `tcc_verdict` defers an unreadable database to the health gate only when SIP is
  on (`Sip::On`) and keeps it fatal with SIP off, where the grant step needs the
  read; `verify_agent_health` now requires `accessible: true` (the agent's
  `AXIsProcessTrusted()`, as the Linux gate already did), via the shared
  `golden_linux::health_accessible`. Unit-tested (187 pass in the crate). This
  makes the health gate the single post-SIP proof of the grant, so it is stricter
  than before on Tahoe too.

## Outcome (2026-10-18)

**Delivered and verified live:**
- `vm create-golden --platform macos --version 27` (branch binary, human's
  terminal) completed and left `testanyware-golden-macos-27`. The pipeline ran
  on 27 without change except the TCC verification: boot 1 (CLT, Homebrew,
  agent), `csrutil disable`, the TCC grant and read-back, `csrutil enable`, the
  health gate, the clone.
- A clone started with `vm start --platform macos --base
  testanyware-golden-macos-27` (human's terminal; the display switched to
  1920x1080 @ 1x). From the harness, `curl http://192.168.64.4:8648/health`
  returned `{"accessible":true,"platform":"macos"}` and `/windows` returned
  windows. `defaults read com.apple.WindowManager StandardHideWidgets` = 1, and
  `csrutil status` = enabled, in the clone.
- The clone was stopped with `vm stop` (it deletes the clone). Afterwards `vm
  list` shows none running; `tart list` shows the base, the 27 golden, the Tahoe
  golden and the Linux golden, all stopped.

**What 27 broke, and the fix:** one thing, the post-SIP TCC read-back (see the
running log). Fixed in `finalize.rs` and unit-tested; the live build is the test
of the fix. Nothing in boot-1 provisioning, the recovery OCR targets or the TCC
insert needed a change.

**Not exercised here:** the Tahoe path with the stricter health gate; the
Display / HiDPI and accessibility checks beyond `/health` and `/windows`
(`checks-27`); the desktop widgets still draw on 27 (passed to `checks-27`).

**Harness note:** the Local Network blocker is specific to the harness. The
human's terminal reaches guests, so russh steps (`vm create-golden`, `vm start`)
ran there. Saved to memory; the human wants it fixed later.
