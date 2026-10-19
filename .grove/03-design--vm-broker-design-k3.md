# vm-broker-design-k3

## Goal

Design the VM broker and the TestAnyware app, to the requirements in the
decision log of `plan-k1`. Write the spec `docs/specs/vm-broker.md`. Rework
ADR-0004 and ADR-0019 in place. Add an ADR only where the three-part test of
`ADR-FORMAT.md` holds.

## Context

- `docs/research/vm-broker-prior-art-a.md`, which `vm-broker-prior-art-k2`
  writes.
- The decision log of `plan-k1`: the requirements, the words of the human and
  the rejected options.
- A survey of the code at `abbc370` reported the facts below. Read the code to
  confirm each fact before the spec relies on it.
  - `resolve.rs` resolves the target. `TESTANYWARE_VM_ID` beats an explicit
    `--agent` or `--vnc`, but the doc comment in `main.rs` says the reverse.
  - `testanyware-agent-client` sends plain HTTP with no authentication.
    `testanyware-rfb` opens each RFB connection. `viewer` and `screen record`
    hold long-lived RFB connections.
  - `vm start` detaches `tart run`, QEMU, swtpm and the VNC keeper with
    `setsid`. `vm list` rebuilds its view from `tart list`, the sidecars and the
    PIDs.
  - These paths connect to an address that is not loopback:
    - each agent call to a tart guest
    - the health wait of `vm start`
    - `display::apply`
    - the SSH of golden creation
    - explicit LAN endpoints.
  - QEMU forwards the agent port with no host address. QEMU documents that such
    a forward listens on each host interface. That would put the `/exec` of the
    agent on the LAN. Nobody confirmed this live.
  - The agent client can return `AGENT_INVALID_JSON`, but `surface::ERROR_CODES`
    does not list it.
  - `connection-spec.md` calls the QEMU VNC endpoint "LAN", and
    `multi-vm-networking.md` puts the QEMU agent at `<vm-ip>:8648`. Both are out
    of date.

## Questions the spec settles

1. Packaging: how the signed app ships through `linkuistics/taps`, its bundle
   id, and whether it needs notarisation.
2. Start: how the CLI starts the app when it is not running. What an agent sees
   before the first grant.
3. Access: how only the user who owns the app reaches the VM broker.
4. Protocol: whether the VM broker forwards connections or runs commands. How
   long-lived streams and file transfer pass through it, with the JSON envelope
   and exit codes unchanged.
5. Lifecycle: how the VM broker starts, stops and adopts VMs, VMs that an older
   CLI started included.
6. Version skew between the CLI and a running VM broker.
7. Where the Local Network diagnosis of ADR-0019 goes, and which remedy it
   names.
8. What `doctor`, `llm-instructions`, `vm start --json` and `vm list --json`
   report about the app and the VM broker.
9. How `vm create-golden` keeps its direct path.

## Facts to prove by experiment

Prove each fact on this Mac before the spec relies on it:

- The Local Network grant of an app with a Developer ID signature covers the
  connections of the app to the tart guest subnet.
- `open`, run from a herdr session, starts the app as its own responsible
  process.
- The grant survives an upgrade of the app to a new Homebrew path.
- The app can start `tart run`, QEMU and the VNC keeper as `vm start` does
  today.

## Done when

- `docs/specs/vm-broker.md` describes the design and cites the ADRs in its
  area. It states the three agreed test seams of the root brief.
- ADR-0004 and ADR-0019 state what now binds, and each citation of them is
  reconciled.
- `CONTEXT.md` defines each new term.
- Each fact above has evidence, or the spec names it as an open risk.
- The human agrees the spec.

## Notes

- The first Local Network grant is a system alert that only a human can answer.
  When an experiment reaches that alert, ask the human to allow the app.
- Do not cut implementation leaves. `vm-broker-increments-k4` decomposes the
  agreed design.
- If the spec needs an adversarial read, cut its review with `leaf-insert`
  ahead of `vm-broker-increments-k4`. Then planning reads a reviewed design.
