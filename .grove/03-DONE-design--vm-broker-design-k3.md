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
- On 2026-10-20 the human added a requirement during `vm-broker-prior-art-k2`.
  Their words:

  <!-- tc: off because this is the human's message, quoted verbatim -->

  > BTW: the configuration should be like the ravel configuration mechanism
  > (i.e. code), but using Rhai (which is what I want to change Ravel to)

  <!-- tc: on -->

  Ravel reads an owner policy that is code: a TypeScript file that a worker
  evaluates (`ravel start-session --config ./policy.ts`). TestAnyware has no
  configuration file today. It reads the environment variables in
  `docs/reference/env-vars.md` and the XDG directories. The spec settles these
  points, and asks the human where the answer is not clear:
  - What the Rhai configuration configures: the VM broker and the TestAnyware
    app, or the Host CLI too.
  - Where the file lives, and which process evaluates it.
  - How it sits beside the environment variables, whose meaning stays fixed.

## Decisions (running log)

**A signed app that `open` starts from herdr holds the Local Network grant.** A
throwaway probe app tested this on 2026-10-20, on macOS 27.0.1. The probe was a
signed LSUIElement bundle, `dev.antony.TestAnyware.probe`. An ad hoc copy of
`nc` in this herdr shell got "No route to host", and the platform `nc` connected.

`open` from this shell started the probe with `launchd` as its parent. Its first
connections to the guest failed with errno 65, and the human allowed its alert.
The retry in the same process then connected. Each later start from herdr
connected at once. So facts 1 and 2 hold. The evidence goes into a research
document beside the spec.

**The app can start every guest process, and each one outlives the app.** The
probe spawned `tart run`, the VNC keeper, a QEMU with HVF and an swtpm, each with
`POSIX_SPAWN_SETSID`. Each child named the probe as its responsible process. Each
kept running after the probe exited. So fact 4 holds. One side finding: tart's
VNC listens on `*:<port>`, a wildcard socket, although its URL names
`127.0.0.1`.

**The grant follows the main executable's UUID, not the path.** Fact 3 was
worded for a new Homebrew path. The probe measured each input apart:

- A new main executable UUID at one path gave errno 65 once, then a connection
  2 s later. This happened three times. One of the three raised the alert again.
- The same executable with a new signature and version connected at once, with
  no alert. So did the same build at a second path.
- A launcher that stays the same, with a helper that changes, connected at once.
- A new build whose UUID was set to the old value connected at once, with no
  alert.

System Settings showed one row for the app throughout. So the release pins the
UUID of the app's executable to one value, and the path is free. The launcher
split works too, but it costs a second process.

**The TestAnyware app is a menu bar app, as Modaliser and Koine are.** The
human said: "The TestAnyware app / router should be a menubar app like
Modaliser and Koine." That also keeps it out of the background-only class,
which the survey says never receives the alert.

**Homebrew carries the approval of the first start across an upgrade.** Koine's
release acceptance shows that a quarantined, notarised app asks "downloaded from
the Internet" when `open` first starts it. Homebrew 7.0.9 copies the user's approval to
the new copy when the signing identity is unchanged (`inherit_user_approval!` in
`cask/quarantine.rb`). With `uninstall quit:`, it also quits the app during the
upgrade and reopens it with `open -b`. So the cask keeps quarantine, as Koine
does, and nothing strips it. Nobody has tested this with a TestAnyware release.

**The app starts exactly as Koine does, and a client never starts it.** The
human chose this over the recommended mix of a login item and a start by the
CLI. Their words:

<!-- tc: off because this is the human's message, quoted verbatim -->

> I would rather launch TestAnyware not from LaunchServices, but have it launch
> exactly Koine, which is the closest match.

<!-- tc: on -->

Koine is a login item through `SMAppService.mainApp`. The user starts it once
and turns on its start at login during setup. macOS then starts it at each
later login.

So requirement 1 of `plan-k1` changes. While the app runs, an agent never waits
for a human. After a quit or a crash, a command fails and names the remedy, and
a human opens the app again. The root brief's done-when now says so.

**One Rhai file configures the app and the Host CLI.** The human chose this
over a file for the app alone. The file is `~/.config/testanyware/config.rhai`,
and it is optional. Both processes evaluate it with one shared evaluator, so
one file gives one value.

**The app refuses directories that differ from its own.** The human chose
this. The CLI sends the state and data directories that it resolved. If
`XDG_STATE_HOME` or `XDG_DATA_HOME` make them differ from the app's, the
command fails, names both, and names `config.rhai` as the remedy.

**"TestAnyware app" is the one term, and "VM broker" goes under _Avoid_.** The
human said: "I think the VM Broker and the TestAnyware app are the same thing."
The app has one process, and serving guest traffic is all that it does apart
from its menu. The human chose "TestAnyware app" over "VM broker". So the spec is
`docs/specs/testanyware-app.md`, and the error codes stay `APP_*`. The handles
of this grove's leaves keep their slugs, because a handle never changes.

**The human agreed the spec, and chose a review ahead of planning.** On
2026-10-20 the human agreed `docs/specs/testanyware-app.md`, ADR-0004, ADR-0019,
ADR-0020 and ADR-0021. They chose the recommended option: a `review-design` leaf
ahead of `vm-broker-increments-k4`, so that planning reads a reviewed design.
