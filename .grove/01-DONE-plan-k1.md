# plan-k1

**Kind:** planning

## Goal

Decide *what* "increase the default screen size" means for TestAnyware guest VMs
and *where* the change must land, then grow the tree into the work leaves that
implement it. (Tentative until grilling settles it.)

## Context

Recon map of where guest display resolution is set today (subagent sweep,
2026-06-24). **Headline: TestAnyware applies no default resolution — when
`--display` is omitted, the hypervisor decides.**

- **CLI flag.** `vm start --display WxH` (`cli-rs/.../testanyware-cli/src/main.rs`)
  is `Option<String>`, no default. Routed to backends via `VmStartOptions`.
- **tart backend (macOS).** `tart.rs:209` `set_display` runs `tart set <id>
  --display WxH`, called at `tart.rs:355` only if `opts.display.is_some()`.
  Otherwise Virtualization.framework default (unknown value).
- **QEMU backend (Linux/Windows).** `qemu.rs:124` builds
  `virtio-gpu-pci,xres=W,yres=H` only when `spec.display.is_some()`; else bare
  `virtio-gpu-pci` (virtio-gpu default).
- **Golden creation sets NO resolution.** macOS (`golden.rs`), Linux
  (`golden_linux.rs`), Windows (`autounattend.xml`) provision login/agent but no
  display geometry. macOS `golden.rs:146` `sips -z 1080 1920` only sizes the
  wallpaper PNG (cosmetic, not guest resolution).
- **RFB stack.** `connection.rs:131` reads width/height from the server's
  ServerInit handshake; no hard-coded default. Rejects 0x0.
- **Embedded viewer.** `viewer.rs:174` provisional window `1024x768`, then
  resizes to the framebuffer once the first frame arrives.
- **Vision/ML training.** `vision/.../scenario_library.py:5` `_SCREEN_W=1920,
  _SCREEN_H=1080` — synthetic YOLO training assumes 1920x1080 guests. (Training
  data, not VM config, but a downstream coupling.)

## Done when

Tree grown: the open design questions (motivation, target resolution(s),
per-platform handling, where the default lives, downstream impact) are settled
enough to spawn the implementation leaf/leaves.

## Notes

## Decisions (running log)

**Q1 — Motivation & mechanism (settled).** Guests boot too small for the
accessibility/vision testing workload: the vision/OCR models expect ~1920x1080
and small hypervisor-default guests are off-distribution, and apps under test
need more screen real estate. Mechanism confirmed: **introduce an explicit,
larger TestAnyware-owned default resolution applied at `vm start` when
`--display` is omitted** — not golden-image baking. `--display` continues to
override.

**Q2 — Uniform vs per-platform (settled).** Uniform target across
macOS/Linux/Windows: the **RFB framebuffer must come back 1920x1080 _pixels_**
on every platform — that pixel count is the real contract (it's what the vision
pipeline consumes and what `scenario_library.py` trains on). Uniform target,
but **platform-aware input encoding** (see HiDPI finding).

**HiDPI finding (tart 2.32.1, `tart set --help`).** `--display` format is
`WIDTHxHEIGHT[pt|px]`; units are *hints* defaulting to **pt (points) for macOS
VMs** and **px (pixels) for Linux VMs**. So a bare `1920x1080` on a macOS guest
= 1920x1080 _points_ → at 2x backing scale a **3840x2160 _pixel_ framebuffer**,
overshooting the vision target. Lever: pass **`1920x1080px`** (force pixels) for
the macOS default to get a 1920x1080-px (LoDPI) framebuffer. Linux tart is px by
default; QEMU `virtio-gpu-pci,xres/yres` is pixels directly (Linux+Windows ✓).
Existing bare-string `--display` flag is a latent footgun on macOS (user-passed
values get pt semantics) — leave user values alone, but the *default* we emit
must carry the explicit `px` unit. Also noted: tart `--display-refit/
--no-display-refit` auto-fits display to window — likely irrelevant headless/VNC
but the work leaf should confirm it doesn't perturb a fixed-resolution VNC
session. **Empirical check owed (work leaf):** confirm VF actually yields exactly
1920x1080 px for `1920x1080px` via `screen size` against a running macOS golden
(VF may snap to a supported mode); eyeball LoDPI render fidelity vs the synthetic
training look.

**Q3 — Value (settled).** Default = **1920x1080** (px framebuffer). Matches the
vision training distribution exactly → guests come *onto* distribution with
**zero retraining**, while still a substantial increase over the small
hypervisor defaults. Bigger (e.g. 2560x1440) was rejected: it would push guests
off the vision training distribution and drag a synthetic-data-regeneration +
model-retraining effort into this grove. 1920x1080 serves both motives
(vision alignment + real estate) adequately.
