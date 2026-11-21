# tart crashes when a VNC client connects after the server sat idle on macOS 27

**Status:** isolated and measured, 2026-10-18 to 2026-10-19. Found by leaf `checks-27-k16`
and isolated by leaf `vnc-reconnect-crash-27-k17` of the issue #6 grove. Measured again on
2026-11-21 by leaves `plan-k1` and `keeper-removal-k2` of the issue #13 grove, on macOS 27.2.1.

**Audience:** whoever meets this fault on a macOS host, and whoever decides whether
TestAnyware needs a mitigation again.

**Bottom line:** on macOS 27.0.1, Apple's `_VZVNCServer` aborts the tart process when a
client connects after no client was connected for 60 seconds. The VM stops with it. One open
connection prevents it. A macOS 26 guest and a Linux guest crash the same way, so the fault is in the host.
macOS 27.2.1 (26B131) removed the fault. On that release, neither trigger crashed tart.

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

## Measured again on macOS 27.2.1

On 2026-11-21 the host ran macOS 27.2.1 (26B131), and the installed tart was 2.42.0, the latest
release. No run had a holder. The reset rows used `scripts/tart-vnc-input-control.py` for the
handshake: a client asked for a full frame, waited one second, and closed with the update
unread. A peek before each close counted the unread bytes, so each close sent a reset.

| Run | Gap or rounds | Result |
|---|---|---|
| `task vm:tart-vnc-idle-repro`, tart 2.42.0 | 70 s | Connects. tart runs |
| The same | 300 s | Connects. tart runs |
| The same with the tart 2.40.1 release | 70 s | Connects. tart runs |
| Unread update, reset, a new connection after 5 s, on a guest that `vm start` started, with its keeper killed | 3 rounds, 96,544 bytes unread each | All connect. tart runs |
| The same on a fresh clone that `tart run` started | 3 rounds, 49,241 bytes unread each | All connect. tart runs |
| The real CLI through the TestAnyware app, no keeper: `screen size`, 5 s, `screen size`, 75 s, `screen size` | One sequence | All work. tart runs |

The digest of the tart 2.40.1 release matched the SHA-256 that GitHub publishes. Its signature
named the same Cirrus Labs team as the installed tart. That release crashed in every
tested run on 27.0.1, and it passed here. So macOS removed the fault, not tart.

The crash-report folder held no tart report before or after the runs, so its silence only
corroborates the result. The count of runs is small. On 27.0.1, the reset crashed tart on six of
seven clones, and here it crashed none of six. The app resets each guest connection when its
client leaves, so the last row also passed the reset trigger through the product.

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
- Which point release first fixed it. 27.2.1 does not have the fault. Nobody tested a release
  between 27.0.1 and 27.2.1.
- Whether a macOS 26 host crashes. Nobody tested one.
- Why the three reports from 16 October exist.

## Effect on TestAnyware

On a host with the fault, any macOS guest loses its VM when a `screen` or `input` command
follows 60 seconds with no VNC client. This affects every long agent session on such a host. Windows guests run under
QEMU, which has its own VNC server, so they are not affected. A Linux guest runs under tart with
the same flag and crashes the same way.
`vm start` makes its VNC calls within seconds, so it did not crash in these runs.

One smaller symptom came with it. The first connection after boot sees a 1280x720 frame, and
a later one sees 1024x768. One `screen capture` on a first connection failed with
`PNG encode failed: The Image's dimensions are either too small or too large`. A retry worked.

## Workaround and mitigation

**On a host with the fault:** one open VNC connection prevents the crash. A bare TCP connection
that sends nothing is enough. `task vm:tart-vnc-idle-repro` checks a host, and
`task vm:tart-vnc-idle-repro HOLD=1` shows that a held connection prevents the crash.

**In the product:** from 2.2.0 to 2.9.0, each tart-backed `vm start` started a VNC keeper. The
keeper was a detached process that held one bare TCP connection for the life of the guest.
TestAnyware now carries no mitigation. ADR-0018 requires the latest release of macOS 27 and of
tart on a macOS host, and that host does not have the fault.

## Where the findings landed

ADR-0018 (`docs/adr/0018-a-macos-host-runs-the-latest-macos-27-and-tart.md`) uses these findings:

- The 60-second gap and the bare TCP holder are the history of the keeper.
- The measurements on macOS 27.2.1 are the reason for the host requirement. They are also the
  reason to remove the keeper and the app's VNC probe gate.
- The reset rows on 27.2.1 show that the app can reset a guest connection with no holder.
- The idle reproduction stays the instrument. A later release that fails it reopens ADR-0018.

## Draft report for Apple (Feedback Assistant) and Cirrus Labs (tart)

macOS 27.2.1 removed the fault, so this report needs no filing. It stays as the record of the
signature of the fault, in case a later release brings it back.

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
