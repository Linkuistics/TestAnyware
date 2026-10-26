# tart drops VNC input that a reset follows at once, on a macOS 27 host

**Status:** measured on 2026-10-26 by leaf `input-live-k2` of the issue #12 grove.

**Audience:** whoever fixes the fault of GitHub issue 12 (`input-delivery-k3`), and the human
who decides what to file upstream.

**Bottom line:** the VNC server of tart discards input events when a reset (RST) follows them
at once. A reset after a pause of 10 ms keeps every event. An ordinary close (FIN) keeps every
event at any time. The app ends the guest connection of a relay with a reset (`close_guest`,
since 2.5.0). It does so as soon as the client leaves.

An `input` command closes straight after its last event. So the app drops the input of every `input` command. The fault lies on
the path through the app. It does not lie in the CLI or in the guest.

## What the user sees

Issue 12 reports that `input type`, `input scroll` and `input drag` exit 0 on a macOS 27 guest
and change nothing. The live check `scripts/input-live.sh` (`task input:live`) shows the same
for `input click`. The guest cursor stays where it was.

## The reproduction

The run used the development bundle, built at the commit that follows v2.6.0. No file of the
input path differs from v2.6.0 (see the running log of `plan-k1`). The development bundle
reports the build `0.0.1`. The installed 2.6.0 app could not take part: macOS reported its
Local Network grant as `denied` that evening, and a person must allow it again.

The host is macOS 27.0 on arm64. The guest is a clone of `testanyware-golden-macos-27`
(macOS 27.0.1). tart is 2.40.1. The fixture is `scripts/fixtures/input-fixture.swift`: a text
field above a scroll view of 40 rows, with a legacy scroll bar.

| Step | Result through the app |
|---|---|
| The oracle: `agent set-value` on the field, read with `agent snapshot` | The field changed. The agent surface works. |
| `input click` on the field | The field stayed unfocused. |
| `input type "input-live-ok"` | The field stayed empty. |
| `input scroll --dy=-5` on the scroll view | `row-20` stayed at y=986. |
| `input drag` of the thumb, 60 points down | `row-20` stayed at y=986. |

Two more runs by hand gave the same result for `--dy=-30` and for a drag of 111 points.

## The control

The control is `scripts/tart-vnc-input-control.py`. It is a bare RFB 3.8 client with VNC
authentication. It connects to the loopback endpoint that tart serves. That endpoint is the
`vnc` object of the connection spec. The client sends the events of one command. Then it closes
in the way that its option names.

It uses no TestAnyware code. The VNC keeper (ADR-0018) held its usual connection throughout.

Each row is one command to one guest. `row-20` started each group at the y that the previous
row left, and each wheel command is five wheel steps of 10 points.

| Client | Close | Gap between events | Result |
|---|---|---|---|
| Handshake only, then `scroll` | ordinary | 20 ms | `row-20` moved 50 points. |
| Handshake, SetPixelFormat and SetEncodings of the TestAnyware client, then `scroll` | ordinary | 20 ms | Moved 50. |
| Bare client, `scroll` | reset | 20 ms | Moved 50. |
| TestAnyware handshake, `scroll` | reset | 20 ms | Moved 50. |
| TestAnyware handshake, `scroll` | reset | none, one write per event | Moved 10 of 50. |
| TestAnyware handshake, `scroll` | ordinary | none | Moved 50. |
| TestAnyware handshake, `scroll`, all events in one write (`--burst`) | **reset** | none | **Moved 0 of 50, three runs in three.** |
| The same | ordinary | none | Moved 50, two runs in two. |
| The same | reset after a pause of 10 ms, 50 ms, 200 ms, 1 s | pause | Moved 50 in each of four runs. |
| `click`, `type "abc"`, `drag` of the thumb, all in one write | ordinary | none | Field focused, text "abc" typed, the view moved. |

The one-write rows copy the app. The app reads the client's bytes and writes them to the guest
together, and it resets when the client leaves. The reset rows with a gap or a pause show
that tart reads input in the time between events. The one-write row shows that a reset discards
the bytes that tart has not read yet.

The runs of the reset rows measured `scroll` only. `click`, `type` and `drag` went through the
ordinary close, which works.

The count of runs is small. The burst with a reset failed in 3 of
3 runs. A pause of 10 ms or more failed in 0 of 4 runs. The effect is large. The rate of a
failure with a pause below 10 ms is not measured.

## What the control settles

- The cause is on the path through the app. The same events through a bare client reach the
  guest.
- The reset of 2.5.0 is the cause, together with the speed of an `input` command. A reset does
  not drop input that tart has had time to read. The relay resets before tart reads.
- The lead of `plan-k1` holds. The macOS kernel keeps unread bytes after a reset on loopback.
  The VNC server of tart does not act on them once it sees the reset. The measurement does not
  say which event tart reads first. It shows only that input is lost.

## A second finding, not the cause

A client that sends a `SetEncodings` without the pseudo-encoding DesktopSize (-223) makes tart
stop the guest. tart logs `It is unclear if we can support clients that don't support this
pseudo encoding` and the guest ends. The first version of the control sent Raw alone and
stopped a guest this way. The TestAnyware client always sends DesktopSize, so it does not
trigger this. The control sends the list of the TestAnyware client with `--like-cli`.

## For the fix

- A fix keeps the reset of 2.5.0 for the descriptors. The spec section "Guest traffic on a
  macOS host" gives the reason. The fix must not reset before tart reads the input.
- A pause of 10 ms was enough in four runs. A fixed pause is a guess. A better signal comes
  from the guest, for example a read that proves that tart took the bytes.
- The regression guard is the routing seam (`task app:routing-test`). Its fake RFB server must
  discard unread input when it sees a reset, as tart does.
- The live check `task input:live` is the acceptance test, and it needs the fix to pass.

## The fix, and how it was checked

Leaf `input-delivery-k3` fixed the fault in the RFB client of the CLI. After the last event of
an `input` command, the client sends a one-pixel FramebufferUpdateRequest that is not
incremental, and it waits for the update. The app keeps the reset of 2.5.0 and guesses no pause.

- `task input:live` passed on the development bundle: `input click` focused the field, `input
  type` typed the text, `input scroll` moved `row-20` from y=986 to y=936, and `input drag` of
  the thumb moved it to y=759. It failed all four steps before the fix.
- `task app:stream-end-live` passed: the app held 33 descriptors after one command and 32 after
  151, and tart held one connection in `CLOSE_WAIT` after one command and after 151.
- The routing seam has a guest that reads late and drops unread input at a reset. Its two tests
  failed before the fix, and they pass after it.

The runs did not show that tart sends an update that we did not ask for. Such an update would
end the wait early. The live check passed in one run, so it gives no rate.

## Not measured

- Linux and Windows guests. They do not use tart's VNC server in the same way, and the issue
  scopes macOS.
- A macOS 26 guest. The grove abandons it.
- Whether the 2.4.0 app passes. The control settles the cause, so the build was not needed.
