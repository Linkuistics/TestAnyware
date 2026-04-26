---
title: Troubleshooting
---

# Troubleshooting

Known issues, VM quirks, platform-specific workarounds, and CLI edge
cases. Every entry here has been observed against real hardware and
software; workarounds are load-bearing.

## VM state is non-deterministic

Each run generates different file explorer content, different terminal
state. Cross-session metric comparisons are meaningless; re-run
baselines when comparing. Use A/B evaluation on captured snapshots
instead.

## Icon-only buttons pollute ground truth

Toolbar buttons with AX descriptions (e.g., "back", "forward") aren't
rendered text. Filter applied to button/toggle-button only; non-button
roles (menu-item, image) still carry icon descriptions. Extended GT
filtering needed for high-precision scenarios.

## macOS OCR has sparse coverage

Apple Vision OCR skips lines in dense monospace output (e.g., terminal
`ls` listing). Terminal content GT helps Linux/Windows but hurts macOS
(-4.3 pp) because predictions don't exist. Disable terminal GT on
macOS.

## AT-SPI coordinate bug on Linux

GTK4 returns `(0,0)` for all coordinate types. The TestAnyware Linux
agent detects this and computes the offset via an `xdotool` window
search plus CSD padding. Requires `WaylandEnable=false` in GDM because
xdotool can't find native Wayland windows.

## Order matters in filter chains

Pipeline filters (showing check → role exclusion → text-content
inclusion → label extract → icon filter) apply sequentially; later
filters only see elements that passed earlier ones. Changing the order
changes the result set.

## IoU spatial matching is fundamentally broken

AX boxes include padding (menu bar 30 px tall vs OCR 12 px). Median
IoU 0.305 even for matched text. Replaced with center-distance:
predicts within GT box (±10 px margin) or GT fully contains predict.
Not a threshold fix — the metric itself was wrong.

## `--window` on input commands includes Tahoe drop-shadow inset

The `--window <name>` flag on `testanyware input click` / `mouse-down`
/ `mouse-up` / `move` / `scroll` / `drag` translates caller-supplied
coordinates via the AX-reported window origin. On macOS Tahoe, that
origin includes the window's drop-shadow inset, so every click lands
~40 px below the intended position. The failure is silent: the CLI
reports a successful click; the target UI control is untouched. For
precise targeting, capture a full-screen `testanyware screenshot`,
read screen-absolute coords off it, and pass those directly without
`--window`. `--window` is still useful for approximate targeting
where 40 px drift is tolerable. Surfaced 2026-04-18 during Mini
Browser verification.

## `testanyware agent set-value` fails for NSTextField inside NSStackView

The macOS agent's element resolver does not reliably reach text
fields hosted inside `NSStackView` containers on Tahoe. Symptom:
`--role textfield --window "..."` returns "Multiple elements matched";
adding `--index N` returns "No element found matching query". AppKit
does not always propagate stack-view children through
`kAXChildrenAttribute` on Tahoe. Workaround until fixed: derive the
field's screen-absolute VNC coords from a full-screen screenshot,
triple-click at those coords to focus and select existing text, then
`testanyware input type` + `testanyware input key return`. Backlog
item #8 tracks the fix.

## Electron apps on `testanyware-golden-linux-24.04` need `--disable-gpu`

Launching an Electron app (Obsidian, VSCode, Slack, etc.) inside the
Linux golden image produces a completely black framebuffer — X11
reports the window as created and focused,
`xdotool getactivewindow getwindowname` returns the correct title,
but `testanyware screenshot` and in-VM `scrot` both capture pure
black. The `virtio-gpu` backend under tart on ARM64 Ubuntu 24.04
doesn't expose the GL acceleration Electron expects for compositing,
and Electron falls back to a non-rendering path instead of software
compositing by default. Workaround: launch the app with
`--disable-gpu --no-sandbox`. Software compositing then renders
correctly into both the VNC framebuffer and local `scrot`. The
`--no-sandbox` flag is only needed if `chrome-sandbox` is not
suid-root (AppImage extraction drops the suid bit; restore it with
`sudo chown root:root chrome-sandbox && sudo chmod 4755 chrome-sandbox`
and drop `--no-sandbox`). First observed during the Ravel
Obsidian symlink validation spike (2026-04-13); same symptom
expected for any Chromium-derived app.
