# tart crashes when a VNC client connects after the server sat idle on macOS 27

**Status:** isolated and measured, 2026-10-18 to 2026-10-19. Found by leaf `checks-27-k16`
and isolated by leaf `vnc-reconnect-crash-27-k17` of the issue #6 grove.

**Audience:** the human who decides what to file upstream, and whoever builds a mitigation.

**Bottom line:** on a macOS 27 host, Apple's `_VZVNCServer` aborts the tart process when a
client connects after no client was connected for 60 seconds. The VM stops with it. One open
connection prevents it. A macOS 26 guest and a Linux guest crash the same way, so the fault is in the host.

## What happens

The host is a Mac16,5 (Apple M4 Max) on macOS 27.0.1 (26A434). tart is 2.40.1.
TestAnyware starts a macOS guest with `tart run --no-graphics --vnc-experimental`.
That flag uses Apple's private `_VZVNCServer`.

Every `testanyware screen` and `testanyware input` command opens a new VNC connection.
A connection made less than 60 seconds after the last client left works. A connection made
60 seconds or more after the last client left makes the tart process abort.

The client sees `I/O error: early eof`. The next command sees `Connection refused`.
`tart list` then reports the clone as `stopped`.

## Results

Every run used a fresh clone of `testanyware-golden-macos-27`, started with `tart run`
directly. The client was a bare RFB 3.8 client written for this test. It uses only the
Python standard library and sends no TestAnyware code. "Handshake only" means the client
authenticates, reads ServerInit and closes. The script is `scripts/tart-vnc-idle-repro.py`.

### The gap limit is 60 seconds

| Client | Gaps tried (seconds, one clone, in order) | Result |
|---|---|---|
| Handshake only | 10, 20, 30, 45 | All connect |
| Handshake only | 60 | Crash |
| Handshake only, new clone | 48, 51, 54, 57 | All connect |
| Handshake only | 150 | Crash |
| Handshake only | 75 (macOS 26 guest, `testanyware-golden-macos-tahoe`) | Crash |
| Handshake only | 70 (Linux guest, `testanyware-golden-linux-24.04`) | Crash |
| Read the whole frame, then close cleanly | 5, 150 | 5 connects, 150 crashes |

The limit lies between 57 and 60 seconds. It looks like a 60-second timer. The gap
counts from the moment the last client left, whatever it did while connected.
An earlier note put the limit at about 120 seconds. That came from coarse runs.

### A client that stays connected prevents it

| Holder | Test | Result |
|---|---|---|
| Full RFB client, polling every 20 s | A second client connects after 90 s, twice | Both connect |
| Same | Holder leaves, then a client connects after 90 s | Crash |
| Bare TCP connection that sends nothing | A client connects after 75 s, twice | Both connect |
| Bare TCP connection (`nc`), real `testanyware screen size` | Idle 300 s, then `screen size` | Works |
| Same | `nc` stopped, 70 s later `screen size` | Crash |
| None, real `testanyware screen size` | `screen size`, 70 s, `screen size` | Crash |

A second client can connect while the holder is connected. The server accepts shared sessions.
A bare TCP connection that never starts the RFB handshake is enough to hold the server.

### A client that closes badly crashes it at once

A client that sends a framebuffer request and closes with the update unread (the kernel sends
an RST) crashed tart at the next connection. The gap was 5, 20, 30 or 40 seconds, on six
fresh clones. One older clone survived two such reconnects, so the effect has a race in it.
A client that reads the whole update and closes cleanly does not show this at 5 seconds.
The TestAnyware client has not shown this effect. Its connections 5 seconds apart worked.

### The other mitigation does not apply

`tart run --vnc` uses the guest's Screen Sharing instead of `_VZVNCServer`.
Port 5900 is closed in the golden, so Screen Sharing is off. Using it would need two changes.
The golden would need Screen Sharing on, and the RFB client would need Apple Remote Desktop
authentication, which it lacks. Neither fits the minimal-image rule or a small change.

### Golden creation

A golden build starts a new `tart run` for each boot, so each boot has a fresh server.
Recovery driving keeps one connection for the whole recovery boot (`recovery.rs`).
I found no other VNC client in the build code. This is a reading of the code and not a test.
Three of the crash reports are from 16 October and predate this grove. Their cause is not known.

## Crash reports

Nine reports exist in `~/Library/Logs/DiagnosticReports/`, named `tart-2026-10-16-{190809,191053,204721}`,
`tart-2026-10-18-{214131,232620,233310,233804,234343}` and `tart-2026-10-19-004556`, each with
the suffix `.ips`. macOS writes fewer reports than there were crashes, because it throttles
them. All nine have the same stack:

```
EXC_BREAKPOINT (SIGTRAP), "assertion failed"
thread queue: com.apple.virtualization.vzvirtualmachine-accessor-manager
  Base::assertion_trap()
  -[_VZVirtualMachineAccessor addAccessorObserver:]
  ... -[_VZVNCServer _setupVirtualMachineAccessor]::$_18 ...
  _dispatch_client_callout
```

## What is still not known

- The cause. One reading fits the evidence and is not proven. The server removes its VM accessor
  60 seconds after the last client leaves, or at once after an error. A client that arrives later
  adds the accessor again, and the assertion fires.
- Whether a macOS 27 point release fixes it. Nobody tested one.
- Whether a macOS 26 host crashes. Nobody tested one.
- Why the three reports from 16 October exist.

## Effect on TestAnyware

Any macOS guest loses its VM when a `screen` or `input` command follows 60 seconds with no
VNC client. This affects every long agent session on a macOS 27 host. Windows guests run under
QEMU, which has its own VNC server, so they are not affected. A Linux guest runs under tart with
the same flag and crashes the same way.
`vm start` makes its VNC calls within seconds, so it did not crash in these runs.

One smaller symptom came with it. The first connection after boot sees a 1280x720 frame, and
a later one sees 1024x768. One `screen capture` on a first connection failed with
`PNG encode failed: The Image's dimensions are either too small or too large`. A retry worked.

## Workaround and mitigation

**For a guest that `vm start` did not start:** keep one VNC connection open for as long as the guest runs. The command
`tail -f /dev/null | nc <vnc-host> <vnc-port>` does it. The user guide has the steps:
`docs/user/troubleshooting.md`. The same recipe in the Taskfile checks the host:
`task vm:tart-vnc-idle-repro` shows the crash, and `task vm:tart-vnc-idle-repro HOLD=1`
shows the fix.

**In the product:** ADR-0018 decides it, and `vm start` now does it. Each tart-backed `vm start` starts a VNC keeper.
The keeper is a detached process that holds one bare TCP connection for the life of the guest.
The manual recipe above applies only to a tart guest that `vm start` did not start.

## Where the findings landed

ADR-0018 (`docs/adr/0018-tart-guests-get-a-vnc-keeper.md`) uses these findings:

- The 60-second gap and the bare TCP holder are the reason for the keeper and its form.
- The RST finding is why the keeper reads and discards all that the server sends.
- The shared sessions are why the keeper cannot disturb the viewer, `screen record` or a command.
- The reading of the golden-creation code is why golden creation gets no keeper.
- The untested macOS 26 host is why the keeper runs on every host, with no version gate.
- The re-run on 2026-10-19 (macOS 27.0.1, tart 2.40.1, 70-second gap) crashed tart again.

## Draft report for Apple (Feedback Assistant) and Cirrus Labs (tart)

**Title:** `_VZVNCServer` asserts in `-[_VZVirtualMachineAccessor addAccessorObserver:]`
when a client connects 60 seconds after the last client left (macOS 27.0.1, tart 2.40.1)

**Steps:**

1. Run `tart clone <macOS guest> vm1`.
2. Run `tart run --no-graphics --vnc-experimental vm1`. Note the VNC URL.
3. Wait 30 seconds for the guest to boot.
4. Connect a VNC client, finish the RFB handshake and disconnect.
5. Wait 60 seconds. Do not connect.
6. Connect a VNC client again.

`scripts/tart-vnc-idle-repro.py --gap 70` runs these steps with a stdlib-only client.

**Expected:** the client gets a ServerInit and the VM keeps running.

**Actual:** the connection closes at once. The tart process dies with `EXC_BREAKPOINT`
in the stack above, and the VM stops.

**Observations that narrow it:**

- A 57-second gap works and a 60-second gap fails.
- A macOS 26 guest and a Linux guest fail the same way, so the guest is not the cause.
- Keeping any TCP connection open, even one that sends nothing, prevents the failure.
- A client that closes with unread data pending (RST) fails at the next connection,
  after a 5-second gap.

**Environment:** Mac16,5, macOS 27.0.1 (26A434), tart 2.40.1, macOS 27.0.1, macOS 26 and Linux guests.

**Attach:** one of the `.ips` files above. The filing is the human's decision.
