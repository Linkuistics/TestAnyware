# vnc-keeper-design-k19

## Goal

Decide whether `vm start` should keep one VNC connection open to a tart guest for the
guest's life, and if so how. The aim is that a `screen` or `input` command never kills a
macOS 27 clone. Record the decision as an ADR. Do not build it.

## Context

- **The fault and its fix are measured.** Read `docs/research/tart-vnc-reconnect-crash-macos-27.md`.
  Apple's `_VZVNCServer` aborts tart when a client connects 60 seconds or more after the
  last client left. One open connection, even a bare TCP connection that sends nothing,
  prevents it. The real CLI survived 300 s idle with an `nc` holder, and died again 70 s
  after the holder stopped.
- **Raised by `vnc-reconnect-crash-27-k17`.** That leaf documented the workaround
  (`docs/user/troubleshooting.md`, `LLM_INSTRUCTIONS.md`). It did not build a keeper,
  because the keeper is a process with a lifecycle.
- **ADR-0004** dropped the persistent shared-VNC server: a multiplexer that other
  invocations attached to. A keeper does not multiplex. It sends nothing and nobody
  attaches to it. The ADR still has to be read and cited, and the design must say
  whether the keeper reverses any part of it.
- **Where a keeper could live.** `vm start` spawns `tart run` with
  `detached::spawn_detached` (`cli-rs/crates/testanyware-vm/src/tart.rs`,
  `detached.rs`) and records its pid in the VM's meta. `vm stop` kills that pid.
- **Scope:** Linux guests under tart crash the same way (`testanyware-golden-linux-24.04`,
  70 s gap, measured). QEMU guests (Windows) have their own VNC server and are not affected.

## Done when

- An ADR (or an edit to ADR-0004) says what holds the connection, how it starts, how it
  stops, and what happens when it dies. It names the options weighed: a detached keeper
  process, a thread in a long-lived process, a wrapper around `tart run`, and doing nothing.
- The ADR says how a macOS 26 host is handled (does the keeper run there or not). It covers
  every tart guest, macOS and Linux.
- The ADR settles one more point: the keeper must not disturb `screen record`, the viewer,
  or the recovery driver, which each open their own connection.
- The follow-up `impl` leaf is cut, or the ADR says the keeper is not worth building.
- If the human has already decided that the documented workaround is enough, say so and
  retire without a design.

## Notes

- Check first whether Apple has fixed the fault: `task vm:tart-vnc-idle-repro` on the
  current host takes 2.5 minutes. If it no longer crashes, the leaf is moot.
- Guests are driven only through the In-VM agent and VNC, never a host-specific channel.

## Decisions (running log)

- **Not moot (2026-10-19).** The host is still macOS 27.0.1 (26A434) with tart 2.40.1.
  `task vm:tart-vnc-idle-repro` (70 s gap) still crashes tart. It printed:
  `second connection failed: connection closed after 0 of 12 bytes; tart state: stopped`.
  The script deleted its clone.
- **No prior human decision that the workaround is enough.** Neither `vnc-reconnect-crash-27-k17`
  nor `default-and-docs-27-k18` records one, so the design goes ahead.
- **Build a keeper (2026-10-19).** The documented workaround is not enough on its own. An LLM
  agent often pauses longer than 60 s between commands, and each such pause loses the guest.
- **A detached keeper process, not a wrapper or a thread (2026-10-19).** TestAnyware has no
  process that lives as long as the guest (ADR-0004), so a thread has no home. A wrapper around
  `tart run` changes what the meta's `pid` means, and it must relay tart's log and forward
  signals. It gives no extra protection: a dead wrapper leaves the same unprotected tart as a
  dead keeper.
- **The keeper is `testanyware _vnc-keeper`, a bare TCP holder that drains (2026-10-19).** It
  never writes and never sends ClientInit, so it cannot claim a session or get framebuffer
  updates. It reads and discards what the server sends (the 12-byte version). Its close is then a
  FIN. The research found that an RST crashes tart at the next connection.
- **It stops itself (2026-10-19).** When its connection ends it reconnects after about 1 s, and
  it exits when the port refuses. So it needs no cleanup on any `vm start` failure path. `vm stop`
  signals the recorded `keeper_pid` only as a backstop, and only if that process is still the
  keeper for this id.
- **No restart when it dies (2026-10-19).** A restart is a new connection, which is safe only
  within 60 s of the last client leaving. The guest falls back to today's behaviour.
- **Every host, no version gate (2026-10-19).** A macOS 26 host is untested. The keeper claims
  nothing on any server, and a wrong gate kills VMs. Killing the keeper by hand is the escape.
- **No keeper in golden creation (2026-10-19).** Each golden boot is a fresh `tart run`. The
  Setup Assistant and recovery drivers hold one connection per boot and do not reconnect in it.
- **ADR-0004 stands, edited in place (2026-10-19).** The keeper does not multiplex and nothing
  attaches to it. Only ADR-0004's "nothing persistent" sentence changes.
- **No review chain, no in-session reviewer (2026-10-19).** ADR-0018 is one small decision, and
  the impl leaf proves its risky claims with executable checks. Those checks include the live
  negative control (kill the keeper, then a 70 s gap crashes tart).
- **Done (2026-10-19).** ADR-0018 written. ADR-0004 and `CONTEXT.md` converted to controlled
  English in their own changes, then edited. The research note links to ADR-0018. Cut
  `vnc-keeper-k20` (impl). The node brief's order list was not updated: the tree shows the order.
