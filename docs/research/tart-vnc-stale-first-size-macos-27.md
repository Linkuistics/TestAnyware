# tart's VNC server gives a stale size to a fast first client after a boot, on a macOS 27 host

**Status:** measured on 2026-11-23 by leaf `first-frame-size-k9` of the issue #13 grove, on macOS
27.2.1 (26B131) with tart 2.42.0, over 33 guests. The fault is present on that host.

**Audience:** whoever changes how `vm start` hands a tart guest to its first command. Also whoever
decides whether TestAnyware still needs the VNC size wait after a new release of macOS or tart.

**Bottom line:** the first RFB session after a tart boot can get a stale size in ServerInit:
1280x720, whatever the size of the guest display. This happens when the client sends ClientInit
within a few milliseconds of the connect, and the TestAnyware RFB client is that fast. Within 1
to 10 seconds, the server sends a `DesktopSize` change to the real size. Each later session gets
the real size. The fault is in the VNC server of tart, not in the display switch of `vm start`.
A Linux guest and a plain `tart run` have it too.

## What the user sees

The first `screen size` after `vm start` prints 1280x720 on a guest whose display is 1920x1080.
The next one prints 1920x1080.

A first `screen capture` writes a 1280x720 PNG. Usually it is the top-left 1280x720 of the real
screen. The server sent `DesktopSize` and a full frame, but the command read its crop size from
ServerInit. Once in four runs, the server sent no frame within the 5 s capture bound, and the
PNG was black.

`screen record`, the HiDPI scale check and an `input` command also read the size of ServerInit.

## The runs

Each run started one guest from `testanyware-golden-macos-27` (macOS 27.2.1), or from
`testanyware-golden-linux-24.04` for the Linux rows. There were three batches. The runs went one
at a time, through the development bundle of commit `9a3194f1`. The installed 2.9.0 app was
quit, because it starts a VNC keeper.

"Fast probe" and "slow probe" are bare RFB 3.8 clients in Python. They use no TestAnyware code,
connect to tart's loopback VNC endpoint directly, and ask for Raw pixels. The fast probe sent
ClientInit 3 to 4 ms after the connect. The slow probe computes the VNC password answer with a
new `openssl` process, which costs about 5 ms, so its ClientInit comes later. The fast poll
opens fast-probe sessions 250 ms apart, and each one closes after ServerInit. The probes, the
drivers and the records stay in the scratch directory of that session.

| Series | First client, and when | First size | Next size |
|---|---|---|---|
| `screen size`, at once after `vm start` | Host CLI | 1280x720 on 3 of 4 | 1920x1080 on 4 of 4, 5 s later |
| `screen capture`, at once after `vm start` | Host CLI | 1280x720 on 4 of 4 | 1920x1080 on 4 of 4 |
| `screen size`, 60 s after `vm start` | Host CLI | 1280x720 on 1 of 2 | 1920x1080 on 2 of 2 |
| `screen size`, 60 s after `vm start`, then a second display switch | Host CLI | 1280x720 on 2 of 2 | 1920x1080 before and after the switch, on 2 of 2 |
| Fast probe, at once after `vm start` | no delay before ClientInit | 1280x720 on 3 of 3 | 1920x1080 |
| Fast probe, at once after `vm start` | 300 ms before ClientInit | 1920x1080 on 3 of 3 | 1920x1080 |
| Slow probe, at once after `vm start` | `openssl` before each auth answer | 1920x1080 on 2 of 2 | 1920x1080 |
| Slow probe, 90 s after a plain `tart run` | `openssl` before each auth answer | 1920x1080 on 2 of 2 | not read |
| Fast probe, 60 s after a plain `tart run` | no delay before ClientInit | 1280x720 on 2 of 2 | not read |
| Linux guest, `screen size` at once after `vm start` | Host CLI | 1280x720 on 1 of 2 | 1920x1080 on 2 of 2 |
| Fast poll, at once after `vm start`, macOS | sessions 250 ms apart | 1280x720 on 3 of 3 | 1920x1080 at the second session, on 3 of 3 |
| Fast poll, at once after `vm start`, Linux | sessions 250 ms apart | 1280x720 on 2 of 2 | 1920x1080 at the second session, on 2 of 2 |
| Fast poll, 60 s after a plain `tart run` | sessions 250 ms apart | 1280x720 on 2 of 2 | 1920x1080 at the second session, on 2 of 2 |

A plain `tart run` here is a clone of the golden with `tart set --display 1920x1080px`, started
with `tart run --no-graphics --vnc-experimental`. Nothing switches its display. A slow probe 90 s
after its boot read 1920x1080.

## What the runs settle

- **The server reports the stale size, and the speed of the client decides.** A fast probe or a
  fast poll got 1280x720 at its first session on each of its 10 guests. The same probe with 300 ms before ClientInit got
  1920x1080 on each of its three. The path through the app is not the cause, because a direct
  fast probe got the stale size too.
- **The display switch of `vm start` is not the cause.** A plain `tart run` with no switch gave
  a fast first client 1280x720 on 4 of 4 boots. A Linux guest has no switch and gave 1280x720
  on 3 of 4. A second switch on
  a guest whose server already knew the size left the next session at 1920x1080.
- **The fault touches only the first session.** In all three batches, each later session read
  the real size. In the fast poll, the second session came 0.26 to 0.27 s after the first one.
  It got the real size on 7 of 7 guests.
- **Time alone does not fix it.** After 60 s with no client, the first session was still stale
  on 3 of 4 guests.
- **The server sends the real size on its own.** In each fast-probe run after `vm start`, the
  `DesktopSize` change came within 1 s of ServerInit. After a plain `tart run`, it came 9.2 s
  after ServerInit once, and not within 11 s once. A new session got the real size sooner than
  a held one did.
- **A cheap request does not reveal the size early.** A 1x1 request 2 ms after a stale
  ServerInit came back as one Raw pixel of the 1280x720 surface, with no `DesktopSize`.
- **A second request can wait.** On a still screen, a second full request on a session got no
  answer within 10 s once in three runs. The capture code already bounds that wait.

## What is still not known

- Why the server chooses 1280x720. The display configuration of each clone was 1920x1080px,
  and no run saw the guest display at 1280x720. 1280x720 is one of the modes that
  Virtualization.framework offers (ADR-0014).
- How long a held first session can stay stale. One stayed stale for more than 11 s. Leaf
  `macos-golden-refresh-run-k13` saw a gap of about 11 s after the first frame of a Recovery
  boot. That was during a boot, not after it.
- Why a second session gets the real size so soon. The fast poll measures it, but not the
  cause.
- Whether a release before macOS 27.2.1 or after tart 2.42.0 behaves the same. On macOS
  27.0.1, the first connection after a boot once saw 1280x720, so the fault is probably older.
- Whether the server forgets the size later, for example after a long idle. The runs read at
  most 65 s after the first session.

## Effect on TestAnyware

Up to 2.9.0, the VNC keeper of each tart guest held a bare TCP connection to the VNC port from
the start of tart. No run measured whether that hid this fault.
Without the keeper, the first command after `vm start` is the first session of the server, and
its ClientInit is fast. So on 7 of 8 macOS guests and on 1 of 2 Linux guests, the first command
read the stale size.

## Where the findings landed

- ADR-0018 records that `vm start` ends with the VNC size wait, because this fault is present
  on the latest host.
- The glossary term *VNC size wait* in `CONTEXT.md` names the step.
- Leaf `vnc-size-wait-k14` of the issue #13 grove implements the wait.
