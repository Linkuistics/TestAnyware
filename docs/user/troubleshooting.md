# Troubleshooting

Known issues, VM quirks, platform-specific workarounds, and CLI edge
cases. Someone observed every entry here against real hardware and
software. The workarounds are load-bearing.

## VM state is non-deterministic

Each run generates different file explorer content and different terminal
state. Cross-session metric comparisons are meaningless. Re-run
baselines when you compare. Use A/B evaluation on captured snapshots
instead.

## Icon-only buttons pollute ground truth

Toolbar buttons with AX descriptions (for example "back" and "forward") are not
rendered text. The filter applies to button/toggle-button roles only. Non-button
roles (menu-item, image) still carry icon descriptions. High-precision
scenarios need extended GT filtering.

## macOS OCR has sparse coverage

Apple Vision OCR skips lines in dense monospace output (for example a terminal
`ls` listing). Terminal content GT helps Linux/Windows but hurts macOS
(-4.3 pp) because predictions do not exist. Disable terminal GT on
macOS.

## AT-SPI coordinate bug on Linux

GTK4 returns `(0,0)` for all coordinate types. The TestAnyware Linux
agent detects this and computes the offset via an `xdotool` window
search plus CSD padding. This workaround requires `WaylandEnable=false` in GDM
because xdotool cannot find native Wayland windows.

## Order matters in filter chains

Pipeline filters (showing check → role exclusion → text-content
inclusion → label extract → icon filter) apply sequentially. Later
filters only see elements that passed earlier ones. Changing the order
changes the result set.

## IoU spatial matching is fundamentally broken

AX boxes include padding (menu bar 30 px tall vs OCR 12 px). Median
IoU 0.305 even for matched text. Centre-distance replaced it. A
prediction matches when it falls within the GT box (±10 px margin) or
when the GT box fully contains it. A threshold change does not fix this,
because the metric itself was wrong.

## `--window` on input commands compensates for the Tahoe drop-shadow inset

The `--window <name>` flag on `testanyware input click` / `mouse-down`
/ `mouse-up` / `move` / `scroll` / `drag` translates caller-supplied
coordinates via the AX-reported window origin. On macOS Tahoe, that
origin sits ~40 px below the visible top of the window (the AX frame
includes the structural drop-shadow inset). The CLI now subtracts a
40 px top inset from the y origin when the resolved platform is
`macos`, so window-relative coordinates land where intended.

If your macOS version reports a different inset, override it with
`TESTANYWARE_WINDOW_TOP_INSET=<int>` (in pixels). For pixel-perfect
targeting against unfamiliar windows, capture a full-screen
`testanyware screenshot`. Read the screen-absolute coordinates off it.
Pass those coordinates directly, without `--window`. Mini Browser
verification surfaced this on 2026-04-18. The compensation landed on
2026-04-28.

## A tart guest dies when `screen` or `input` runs after 60 idle seconds (macOS 27 host)

On a host running macOS 27, a `testanyware screen` or `input` command can stop a
tart guest (macOS or Linux). The symptom is `I/O error: early eof`. The next command reports
`Connection refused`, and `tart list` shows the clone as `stopped`.

Every `screen` and `input` command opens a new VNC connection to `tart run`. The
host's VNC server (Apple's `_VZVNCServer`) aborts the tart process if a client
connects after the last client left 60 seconds or more earlier. The guest stops
with it. Tart 2.40.1 on macOS 27.0.1 shows this.

The guest does not matter. A macOS 26 guest and a Linux guest under tart fail the same way.
Windows guests run under QEMU, which has its own VNC server, and are not affected.
A first connection after boot works.

Two things avoid it:

- **Keep one VNC connection open for as long as the guest runs.** One open
  connection is enough, even a bare TCP connection that sends nothing. With it
  open, commands after any idle gap work. Run it in a spare terminal. Use the
  address and port from `testanyware vm list`:

  ```bash
  tail -f /dev/null | nc 127.0.0.1 <vnc-port>
  ```

  Stop it with Ctrl-C after `vm stop`. If you stop it first, the 60-second rule applies again.
- **Space commands less than 60 seconds apart.** Use this for short scripted runs.
  An agent that waits on the guest between commands should use the first option.

Check the host with `task vm:tart-vnc-idle-repro` (about 2.5 minutes). It runs a bare
RFB client, so the fault shows without TestAnyware. `task vm:tart-vnc-idle-repro HOLD=1`
shows that an open connection prevents it.

A client that closes its connection with unread data pending (an RST) also kills
tart, even after a 5-second gap. The TestAnyware client has not shown this.
Full evidence and the draft report for Apple and Cirrus Labs are in
[`docs/research/tart-vnc-reconnect-crash-macos-27.md`](../research/tart-vnc-reconnect-crash-macos-27.md).
Observed 2026-10-18 to 2026-10-19.

## `testanyware agent set-value` fails for NSTextField inside NSStackView

The macOS agent's element resolver does not reliably reach text
fields hosted inside `NSStackView` containers on Tahoe. The symptom is
that `--role textfield --window "..."` returns "Multiple elements matched".
Adding `--index N` then returns "No element found matching query". AppKit
does not always propagate stack-view children through
`kAXChildrenAttribute` on Tahoe.

Until the fix lands, use this workaround. Derive the field's screen-absolute VNC coordinates from a full-screen
screenshot. Triple-click at those coordinates to focus and select existing
text. Then run `testanyware input type` and then
`testanyware input key return`. Backlog item #8 tracks the fix.

## VirtioFS shared folders can serve stale file content

VirtioFS does not always propagate host-side file updates to the guest
immediately. VirtioFS is the FS-sharing mechanism of tart's `--dir` flag and
similar QEMU `virtiofsd` setups. Suppose you edit a source file on the host.
You then recompile or re-run it in the VM. The VM can pick up the old content.
This can happen even after the host write flushes.

Racket sample-app validation surfaced this on 2026-04-17.

The TestAnyware CLI does not configure VirtioFS itself. The problem appears
when callers stack a tart `--dir` mount on top of a TestAnyware-managed
VM. The agent-channel file-transfer commands bypass the shared-folder
layer entirely. They are the recommended cache-bust path:

```bash
testanyware upload   /host/path/file.rkt /Users/admin/file.rkt
testanyware download /Users/admin/output.txt ./output.txt
```

`upload` and `download` push and pull bytes through the in-VM HTTP
agent on port 8648, which reads and writes the guest's local
filesystem directly. There is no intermediate cache. If a host edit
must be visible to the guest within the same session, `upload` it.
Do not rely on a VirtioFS share that exists at VM start.

Restarting the VM also clears the cache, but `upload`/`download` is
the per-file alternative that does not interrupt other in-flight work.

## Electron apps on `testanyware-golden-linux-24.04` need `--disable-gpu`

Starting an Electron app (Obsidian, VSCode, Slack, etc.) inside the
Linux golden image produces a completely black framebuffer. X11
reports the window as created and focused.
`xdotool getactivewindow getwindowname` returns the window's actual title.
But `testanyware screenshot` and in-VM `scrot` both capture pure
black.

The `virtio-gpu` backend under tart on ARM64 Ubuntu 24.04
does not expose the GL acceleration that Electron expects for compositing.
By default, Electron falls back to a non-rendering path instead of software
compositing.

Workaround: start the app with `--disable-gpu --no-sandbox`. Software
compositing then renders properly into both the VNC framebuffer and local
`scrot`. The `--no-sandbox` flag is only needed if `chrome-sandbox` is not
suid-root. AppImage extraction drops the suid bit. Restore it with
`sudo chown root:root chrome-sandbox && sudo chmod 4755 chrome-sandbox`.
Then drop `--no-sandbox`.

The Ravel Obsidian symlink validation spike first observed this on
2026-04-13. The same symptom is expected for any Chromium-derived app.
