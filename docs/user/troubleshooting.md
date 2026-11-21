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

## `--window` on input commands subtracts a 40 px top inset (set it to 0 on macOS 27)

The `--window <name>` flag on `testanyware input click` / `mouse-down`
/ `mouse-up` / `move` / `scroll` / `drag` translates caller-supplied
coordinates via the AX-reported window origin. The CLI subtracts a
40 px top inset from the y origin when the resolved platform is
`macos`. Mini Browser verification measured that inset on macOS 26 on
2026-04-18. There, the AX origin sat ~40 px below the visible top of the
window.

macOS 27 reports the visible top of the window as the AX origin. A
standard window and a Finder window showed it on 2026-10-26. With the
default inset, a window-relative click on a macOS 27 guest lands 40 px
above the target. Set `TESTANYWARE_WINDOW_TOP_INSET=0` for a macOS 27
guest. The variable takes any other inset in pixels.

For pixel-perfect targeting against unfamiliar windows, capture a
full-screen `testanyware screenshot`. Read the screen-absolute
coordinates off it. Pass those coordinates directly, without `--window`.

## A tart guest dies when `screen` or `input` runs after 60 idle seconds (host below the requirement)

On a macOS host below the host requirement, a `testanyware screen` or `input` command
can stop a tart guest (macOS or Linux). The symptom is `I/O error: early eof`. The next
command reports `Connection refused`, and `tart list` shows the clone as `stopped`.

Every `screen` and `input` command opens a new VNC connection to `tart run`. On macOS
27.0.1, the host's VNC server (Apple's `_VZVNCServer`) aborts the tart process in two
cases. In the first, a client connects 60 seconds or more after the last client left.
In the second, a client closes its connection with unread data pending (an RST), and the
next client connects. The second case kills tart even after a 5-second gap. The guest
stops with tart.

The guest does not matter. A macOS guest and a Linux guest under tart fail the same way.
Windows guests run under QEMU, which has its own VNC server, and are not affected.
A first connection after boot works.

macOS 27.2.1 removed both faults. On a macOS host, TestAnyware requires the latest
release of macOS 27 and the latest release of tart. It carries no workaround for this
fault. Nobody tested a release between 27.0.1 and 27.2.1.

To fix the fault, update the host:

1. Update macOS to the latest release of macOS 27, in System Settings → General →
   Software Update.
2. Update tart to its latest release with `brew upgrade cirruslabs/cli/tart`.
3. Check the host with `task vm:tart-vnc-idle-repro` (about 2.5 minutes). It runs a bare
   RFB client, so it shows the fault without TestAnyware. It passes on a host that
   meets the requirement.

Full evidence and the draft report for Apple and Cirrus Labs are in
[`docs/research/tart-vnc-reconnect-crash-macos-27.md`](../research/tart-vnc-reconnect-crash-macos-27.md).
The fault showed on macOS 27.0.1 from 2026-10-18 to 2026-10-19. On 2026-11-21, macOS
27.2.1 with tart 2.42.0 passed the same reproduction.

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
