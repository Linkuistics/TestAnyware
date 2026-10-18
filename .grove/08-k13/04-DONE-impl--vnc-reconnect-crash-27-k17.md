# vnc-reconnect-crash-27-k17


## Goal

Stop a `testanyware screen` or `testanyware input` command from killing a macOS
27 clone. Today, a VNC connection made after an idle gap of about two minutes
makes tart abort. First isolate the trigger. Then choose the smallest mitigation
that works, or document the limit if none does.

## Context

- **The evidence is written down.** Read
  `docs/research/tart-vnc-reconnect-crash-macos-27.md`. It holds the five runs,
  the crash stack, what is unknown, and a draft upstream report for the human.
- **Found by `checks-27-k16`.** The crash is `EXC_BREAKPOINT` in Apple's
  `-[_VZVirtualMachineAccessor addAccessorObserver:]`, reached from
  `-[_VZVNCServer _setupVirtualMachineAccessor]`. tart 2.40.1, macOS 27.0.1.
- **What is known.** A connection 5 s after the last one works. A connection
  after 120 s or more crashes tart. The VM stays up through the idle gap. A first
  connection after boot works. The display switch is not needed.
- **What is not known.** The exact gap. Whether a client that stays connected
  prevents the crash. Whether the macOS 26 golden (`testanyware-golden-macos-tahoe`)
  on this host crashes too, which tells host from guest. Whether tart's `--vnc`
  flag (guest Screen Sharing) avoids it. Whether golden creation can hit it.
  The earliest crash report is from 21:41, before this grove's `checks-27-k16`.
- **Running a clone from this harness.** `testanyware vm start --platform macos
  --base testanyware-golden-macos-27` works here. It waits its whole boot window
  (about 4 minutes), prints `warning: agent did not reach health`, and exits 0.
  The clone is up and healthy. `curl http://<ip>:8648/health` answers
  `accessible: true`. Use `tart ip <id>` for the address. VNC on loopback works.
  Use the branch binary `cli-rs/target/debug/testanyware`.
- **A crash costs a clone start.** Each experiment that crashes tart needs a new
  `vm start`, about 4 minutes. Plan experiments so one clone answers as much as it can.
- **One macOS VM at a time.**

## Done when

- The trigger is isolated with a minimal VNC client, so the upstream report does
  not depend on the TestAnyware client. The gap limit is measured.
- Both mitigations are tried: a long-lived client, and a different tart VNC
  flag. The result of each is recorded.
- One of two things holds. A clone survives a `screen size` after 5 minutes
  idle, with the chosen mitigation in place. Or the limit and its workaround are
  documented where users read them (`docs/user/`, `README.md`).
- If the mitigation needs a design (a new daemon, a new flag in `vm start`), cut
  a `design` leaf and stop. Do not build it inline.
- `docs/research/tart-vnc-reconnect-crash-macos-27.md` states the final facts.
  The draft report is ready for the human to file. Do not file it.
- No clone or setup VM is left running (`vm list`).

## Notes

- Guests are driven only through the In-VM agent and VNC, never a host-specific
  channel.
- Never modify `testanyware-golden-macos-27` or the Tahoe golden. Run
  experiments on clones.
- Filing upstream is the human's call.

## Decisions (running log)

- **Trigger isolated (2026-10-19).** A bare stdlib RFB client, no TestAnyware code, crashes tart
  when it connects 60 s or more after the last client left. 57 s survives, 60 s dies. The
  `~120 s` in the first note was a coarse reading. The abrupt-close variant (RST) dies at once.
  Evidence and tables: `docs/research/tart-vnc-reconnect-crash-macos-27.md`.
- **Host fault, not guest (2026-10-19).** The macOS 26 golden and the Linux golden crash the
  same way on this host, so no guest fix exists.
- **Long-lived client works, tart `--vnc` does not (2026-10-19).** A bare TCP connection that
  sends nothing is enough. The real CLI survived 300 s idle with an `nc` holder. `tart run
  --vnc` needs Screen Sharing, which the golden has off, and Apple Remote Desktop auth, which the
  RFB client lacks. Not pursued: it breaks the minimal-image rule.
- **Build nothing inline.** A keeper in `vm start` is a process with a lifecycle, so the brief
  says to cut a `design` leaf. Cut `vnc-keeper-design-k19`. Documented the workaround meanwhile
  (`docs/user/troubleshooting.md`, `LLM_INSTRUCTIONS.md`, `README.md`) and added
  `task vm:tart-vnc-idle-repro` (HOLD=1 shows the fix) backed by `scripts/tart-vnc-idle-repro.py`.
- **Not filed.** The draft report in the research note is ready. Filing is the human's call.
