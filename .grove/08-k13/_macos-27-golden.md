# macos-27-golden-k13 — brief

## Goal

Rebuild the macOS golden as macOS 27, because the human has found significant
changes in 27 that warrant it. The result is a macOS 27 golden, built under a
new name, that reaches a Healthy agent from a clone and passes the 27 checks
below. Whether it becomes the default golden is a separate, later decision.

Raised by the human in the `finish-k12` session (2026-10-18), after this
grove's own goal (#6, the Windows TPM and password faults) was met. It is not
part of #6; it sits ahead of the finish sentinel so the grove is not torn down
before it is decided.

## Done when

- A macOS 27 golden is built with `vm create-golden --platform macos` under
  a new name (`testanyware-golden-macos-27`), and a clone of it starts and
  reaches a Healthy agent with no help over VNC.
- The two 27 checks the human named pass in the new guest: the Display / HiDPI
  behaviour (ADR-0014, ADR-0015, ADR-0016: the guest resolution switch and the
  HiDPI build) and the accessibility agent (the macOS In-VM agent's permissions
  and its window/element queries). Anything that breaks in the CLI or agent is
  fixed or externalised.
- The VNC keymap (Command = XK_Alt_L 0xffe9, Option = XK_Meta_L 0xffe7) is
  confirmed on 27 as a side effect of driving the guest; the human did not
  select it as a check, so a failure there is fixed only if it blocks the above.
- `testanyware-golden-macos-tahoe` is untouched and still present.
- The human has been asked whether 27 becomes the default (the macOS default
  base is `testanyware-golden-macos-tahoe` in `lifecycle.rs`, and docs say
  "tahoe"). If yes, the default and every doc that names macOS 26 or "tahoe"
  are updated to match. If no, the docs say how to select the 27 golden. A
  live Taskfile recipe is added only if the work needs one.
- No clone or setup VM is left running (`vm list`).

## Decomposition

The first leaf is cut now; the rest are cut by the session that finishes the
one before it (decomposition stays lazy). Intended order:

1. `ipsw-base-vm` (impl). The CLI can produce a macOS 27 base VM with an
   SSH-reachable `admin`/`admin`, which is what the Cirrus vanilla image
   gives the tahoe path for free: `tart create --from-ipsw`, then Setup
   Assistant driven over RFB + OCR (the ADR-0008 machinery), then Remote Login
   on. Verified by SSH-ing in as admin. It comes first because nothing else
   can start without a 27 base, and because Setup Assistant on 27 is unseen,
   so it is where the unknowns are.
2. `golden-from-27-base` (impl). `vm create-golden --platform macos` takes the
   27 base through the existing provisioning and recovery/finalize stages and
   leaves `testanyware-golden-macos-27`; a clone reaches a Healthy agent. Long
   build, so it should end on a committed, verified golden. Expect 27 to break
   some of the provisioning (CLT, TCC, the SIP recovery cycle's OCR targets).
3. `checks-27` (impl). The Display / HiDPI and accessibility-agent checks in
   the new guest, and the fixes or externalised leaves they call for.
4. `vnc-reconnect-crash-27` (impl). `checks-27` cut it. On this macOS 27.0.1
   host, tart 2.40.1 aborts when a VNC client reconnects after about two idle
   minutes. The leaf isolates the trigger. It then picks the smallest mitigation,
   or documents the limit. It sits before the default decision, because the human
   should know the result first.
5. `default-and-docs-27` (impl). Ask the human whether 27 becomes the default.
   Then update the default, the docs and the website copies to match. It also
   asks two questions that `checks-27` left. One is a live HiDPI `@2x` run, which
   needs the host display switched. The other is a golden rebuild to carry an
   agent fix.

## Decisions (running log)

Settled with the human, 2026-10-18:

- **Base: automate the IPSW install in the CLI.** Cirrus Labs publishes no
  macOS 27 vanilla image: `ghcr.io/cirruslabs/macos-tahoe-vanilla` lists tags
  up to 26.6.2, and `macos-27-vanilla`, `macos-macos27-vanilla`,
  `macos-beta-vanilla`, `macos-tahoe-beta-vanilla`, `macos-27-base`,
  `macos-27-xcode` and `macos-next-vanilla` do not exist. Apple's IPSW is
  available (`UniversalMac_27.0.1_26A434_Restore.ipsw`, from the
  `mesu.apple.com` macOS IPSW catalogue), and `tart create --from-ipsw` takes
  a path, URL or `latest`. The human chose the reproducible route over a
  hand-install or waiting for Cirrus. This is why the leaf became a node.
- **Swap: build under a new name, keep Tahoe.** The 27 golden is
  `testanyware-golden-macos-27`. Tahoe is not deleted or replaced. Whether 27
  becomes the default is asked separately, in `default-and-docs-27`.
- **27 checks: Display / HiDPI and accessibility agent.** Not selected: agent
  health alone, and the keymap as its own check.

## Pointers

- `cli-rs/crates/testanyware-vm/src/golden.rs` (boot-1 provisioning from the
  vanilla image; `vanilla_image`, `GoldenOptions.version`), `recovery.rs`
  (RFB + OCR driving of macOS Recovery) and `finalize.rs` (boot 2, TCC, health
  gate, final clone).
- ADR-0008 (recovery automation via RFB + OCR), ADR-0007 (golden provisioning
  over SSH), ADR-0014 / ADR-0015 / ADR-0016 (macOS guest resolution and HiDPI).
- Glossary: Golden image, Healthy agent, In-VM agent, Host CLI.
- Tart is 2.40.1 on this host; the host is macOS 27.0.1.

## On the horizon

- Whether `vm create-golden`'s `--version` selector should keep meaning "a
  Cirrus vanilla tag" with 27 as a different kind of value, or become a base
  selector that covers both. `ipsw-base-vm` decides it where the code is.

## Notes

- Guests are driven only through the In-VM agent and VNC, never a host-specific
  channel. Keep the image minimal: no test-specific tooling.
- Never run `set-display-mode.swift` on the host; compile on the host, execute
  inside the guest.
- A build is long (the Windows golden took about an hour). IPSW download is
  about 18 GB, plus the install.
- If no human is reachable when a question comes up, stop and leave the leaf
  live. Do not build, swap or replace on an assumed answer.
