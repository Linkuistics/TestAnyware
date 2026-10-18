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
