# tart crashes when a VNC client reconnects after an idle gap on macOS 27

**Status:** observed and reproduced, 2026-10-18. Found by leaf `checks-27-k16` of the
issue #6 grove. The cause is not yet isolated.
**Audience:** the human who decides what to file upstream, and whoever builds a
mitigation.
**Bottom line:** on this host, tart 2.40.1 aborts when a VNC client connects after
the last client left a while ago. The VM stops with it.

## What happens

The host is a Mac16,5 (Apple M4 Max) on macOS 27.0.1 (26A434). tart is 2.40.1.
TestAnyware starts a macOS guest with `tart run --no-graphics --vnc-experimental`.
That flag uses Apple's private `_VZVNCServer`.

Every `testanyware screen` and `testanyware input` command opens a new VNC
connection. Connections made a few seconds apart work. A connection made after a
gap of 120 seconds or more makes the tart process abort.

The client sees `I/O error: early eof`. The next command sees `Connection refused`.
`tart list` then reports the clone as `stopped`.

## Evidence

The leaf session ran five clones of `testanyware-golden-macos-27`. Four crashed.
The host holds five crash reports, one of them older.

| Clone | Sequence | Result |
|---|---|---|
| `9708ab6e` | Two connections, a display switch, about 3 minutes with no VNC client, one connection | Crash at the last connection |
| `9593fd9d` | Six connections 2 s apart, a display switch, four connections 5 s apart, 120 s idle, one connection | Crash at the last connection |
| `e9a6b395` | Two connections 5 s apart, no display switch, 150 s idle, one connection | Crash at the last connection |
| `40b3ad31` | One connection, 200 s idle, `tart list` (state `running`), one connection | Crash at the last connection |
| `a50cc0fc` | Eight connections no more than about 60 s apart | No crash |

Three results follow from these runs.

- The display switch is not needed. Clone `e9a6b395` never had one.
- The idle gap alone does not stop the VM. Clone `40b3ad31` was still `running` after
  200 s. The next connection stopped it.
- The first connection after boot works, even minutes after boot.

All five crash reports have the same stack. The reports are
`~/Library/Logs/DiagnosticReports/tart-2026-10-18-{214131,232620,233310,233804,234343}.ips`.
The 21:41 report is from before this session.

```
EXC_BREAKPOINT (SIGTRAP), "assertion failed"
thread queue: com.apple.virtualization.vzvirtualmachine-accessor-manager
  Base::assertion_trap()
  -[_VZVirtualMachineAccessor addAccessorObserver:]
  ... -[_VZVNCServer _setupVirtualMachineAccessor]::$_18 ...
  _dispatch_client_callout
```

## What is not known

- The exact gap. 5 s is safe and 120 s is not. The limit lies between.
- Whether the first client must have left. A client that stays connected may avoid
  the crash. Nobody tested it.
- Whether macOS 26 hosts crash. Nobody tested one.
- Whether tart's own `--vnc` (which uses guest Screen Sharing) is free of the fault.
- The cause. One hypothesis fits the stack: the server removes its accessor when the
  last client leaves, and a later client asserts when it adds one again. This is a
  reading of the stack, not a result.

## Effect on TestAnyware

Any macOS guest loses its VM when a `screen` or `input` command follows an idle
period of about two minutes or more. This affects every long agent session on a
macOS 27 host. `vm start` makes its VNC calls within seconds, so it did not
crash in these runs. Nobody tested golden creation. The 21:41 report may come from it.

One smaller symptom came with it. The first connection after boot saw a 1280x720
frame, and a later one saw 1024x768. One `screen capture` on a first connection
failed with `PNG encode failed: The Image's dimensions are either too small or too
large`. A retry worked.

## Draft report for Apple (Feedback Assistant) and Cirrus Labs (tart)

**Title:** `_VZVNCServer` asserts in `-[_VZVirtualMachineAccessor addAccessorObserver:]`
when a client connects after an idle gap (macOS 27.0.1, tart 2.40.1)

**Steps:**

1. Run `tart clone <macOS 27 guest> vm1`.
2. Run `tart run --no-graphics --vnc-experimental vm1`. Note the VNC URL.
3. Wait for the guest to boot.
4. Connect a VNC client, read one frame and disconnect.
5. Wait 120 seconds. Do not connect.
6. Connect a VNC client again.

**Expected:** the client gets a frame and the VM keeps running.

**Actual:** the connection closes at once. The tart process dies with `EXC_BREAKPOINT`
in the stack above, and the VM stops.

**Environment:** Mac16,5, macOS 27.0.1 (26A434), tart 2.40.1, macOS 27.0.1 guest.

**Attach:** one of the `.ips` files above.

Before filing, run the steps with a minimal VNC client. The runs above used the
TestAnyware client, which sends the normal RFB handshake.
