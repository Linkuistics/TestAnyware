# The Local Network grant of a signed app, measured from herdr

Evidence for `docs/specs/testanyware-app.md`, ADR-0019 and ADR-0020. Measured on
2026-10-20 on this Mac: macOS 27.0.1 (26A434), tart 2.40.1, Apple silicon. The
measuring shell ran in a Claude session under herdr, which is a login service.
That shell is the context that ADR-0019 diagnoses.

## The questions

The design of the TestAnyware app needs four facts:

1. Does the Local Network grant of a Developer ID app cover its connections to
   the tart guest subnet?
2. Does `open`, run from a herdr session, start the app as its own responsible
   process?
3. Does the grant survive an upgrade of the app?
4. Can the app start `tart run`, QEMU, swtpm and the VNC keeper as `vm start`
   does today, so that each one outlives the app?

## The probe

The probe was a throwaway app bundle, `TestAnywareProbe.app`, with the bundle id
`dev.antony.TestAnyware.probe`. It had these properties:

- A Swift executable that runs `NSApplication` with the accessory policy.
- `LSUIElement` set, and an `NSLocalNetworkUsageDescription`.
- A Developer ID Application signature of team `TA43A4RUP3`, with the hardened
  runtime. It had no notarisation, no secure timestamp and no quarantine
  attribute.
- No listener. Each start ran one fixed experiment that its arguments named,
  wrote one log line for each step, and exited.

The probe connected only to `192.168.64.0/24`, and only to the agent port. Each
start logged the probe's pid, its responsible pid and its parent pid. The
responsible pid came from `responsibility_get_pid_responsible_for_pid`. Each
spawn used `posix_spawn` with `POSIX_SPAWN_SETSID`, as `spawn_detached` does
with `setsid`.

The guest was a clone of `testanyware-golden-linux-24.04` at `192.168.64.2`,
whose In-VM agent answered on port 8648.

## The controls

These ran from the measuring shell at the same moment as the first probe runs.

| Executable | Result |
|---|---|
| `/usr/bin/nc`, platform-signed | connected |
| a copy of `/usr/bin/nc`, re-signed ad hoc | `No route to host` |

So the shell was denied the guest subnet, and the guest's agent was up.

A first attempt at a control for fact 2 failed to discriminate. Under herdr, the
shell, Claude and herdr each report themselves as their own responsible process.
So "the probe is its own responsible process" says nothing by itself. The
evidence for fact 2 is the connection below.

## Facts 1 and 2: the grant, and a start from herdr

`open -g -n <bundle> --args …` from the measuring shell started the probe with
`launchd` as its parent. Its first five connections failed with errno 65, and
macOS showed the Local Network alert for TestAnywareProbe. The human chose
Allow. The sixth attempt, two seconds later in the same process, connected.

Each later start from the same shell connected on its first attempt. So a
signed app that herdr starts through `open` holds its own grant. The grant
covers the app's connections to the tart guest subnet. The probe did not need to
restart after the grant.

## Fact 3: what an upgrade does to the grant

Each row replaced the installed bundle, or copied it, and then ran one connect.
"Same UUID" compares the `LC_UUID` of the main executable with the build before.
The human watched the screen for each row and reported each alert.

| Change | Main executable UUID | First attempt | Alert |
|---|---|---|---|
| 1.0.0, first start | new | errno 65, then connected after the grant | yes, allowed |
| 1.0.0 → 1.0.1 at one path | new | errno 65, connected 2 s later | yes, allowed |
| 1.0.1 → 1.0.2, same executable, new version and signature | same | connected | no |
| 1.0.2 copied to a second path | same | connected | no |
| 1.0.2 → 2.0.0, a stable launcher that spawns a helper | new | errno 65, connected 2 s later | no |
| 2.0.0 → 2.0.1, a new helper only | same | connected | no |
| 2.0.1 → 1.0.3, a new single executable | new | errno 65, connected 2 s later | no |
| 1.0.3 → 1.0.4, new code with its UUID set to that of 1.0.3 | same | connected | no |

System Settings showed one TestAnywareProbe row throughout.

What the table shows:

- A new main executable UUID gave a refusal of about two seconds, three times
  out of three. One of those three raised the alert again.
- A main executable with an unchanged UUID never gave a refusal or an alert. This
  held with a new signature, at a second path, and with new code.
- In the launcher rows, the helper's connections counted as the app's. The
  helper named the launcher as its responsible process.

So on this Mac, the main executable's UUID decides whether macOS evaluates the
grant again. The path did not decide it. That differs from the reports in
`docs/research/vm-broker-prior-art-a.md` (section 3). In those reports each new
path also came with a new build, so the path and the UUID changed together.

## Fact 4: the guest processes outlive the app

The probe spawned each process below and then exited.

| Process | Responsible pid | Running after the probe exited |
|---|---|---|
| `tart run ta-probe-k3 --no-graphics --vnc-experimental` | the probe | yes |
| `testanyware _vnc-keeper` for the guest's VNC port | the probe | yes, holding its connection |
| `qemu-system-aarch64` with HVF, paused and diskless | the probe | yes |
| `swtpm socket --tpm2` | the probe | yes |

So the app can start every guest process as `vm start` does, and none of them
depends on the app's life.

## What this does not show

- The probe was not notarised, quarantined or installed in `/Applications`. A
  release build is all three.
- Each row ran once, and the one alert at the 1.0.1 upgrade has no explanation.
- Nobody tested a new path together with a new UUID, which is the case of a
  Homebrew formula.
- Nobody tested a quit of the app through its menu, or a logout.
- macOS 27.0.1 cannot reset a Local Network row. Before macOS 27.2, each test
  bundle id leaves its row in System Settings.

## A side finding

tart's VNC server listened on `*:<port>`, a wildcard IPv6 socket, while its
`vnc://` URL named `127.0.0.1`. The VNC password still protects it.

## Where the findings landed

- ADR-0020 keeps one signing identity for the app, from fact 3. It accepts that
  a new UUID in each release makes macOS evaluate the grant again. The owner
  rejected the pinned UUID and the stable launcher in leaf
  `vm-broker-design-k6`.
- ADR-0019 moves the Local Network diagnosis into the TestAnyware app, from facts 1
  and 2.
- `docs/specs/testanyware-app.md` relies on fact 4 for the lifecycle of VMs.
