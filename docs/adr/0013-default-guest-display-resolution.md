# 13. Default guest display resolution: 1920×1080 px applied at `vm start`

Date: 2026-06-24

## Status

Accepted

## Context

TestAnyware applies **no default display resolution** today. `vm start --display
WxH` is an `Option<String>` with no default
(`cli-rs/crates/testanyware-cli/src/main.rs`); when omitted, the backend leaves
geometry unset and the *hypervisor* decides:

- **tart (macOS):** `set_display` (`tart.rs:209`) runs `tart set --display` only
  when `opts.display.is_some()` (`tart.rs:355`); otherwise the
  Virtualization.framework default applies.
- **QEMU (Linux/Windows):** `qemu.rs:124` builds `virtio-gpu-pci,xres=W,yres=H`
  only when `spec.display.is_some()`; otherwise a bare `virtio-gpu-pci` with the
  virtio-gpu default (commonly ~1024×768/1280×800).

Those hypervisor defaults are small and, worse, *unknown and inconsistent across
platforms*. Meanwhile the vision pipeline's synthetic training generator
(`vision/stages/window-detection/generator/src/window_gen/scenario_library.py`)
fixes `_SCREEN_W = 1920`, `_SCREEN_H = 1080`. Small guests therefore render
**off the distribution** the OCR/detection models were trained on, and real apps
under test get cramped/clipped for want of screen real estate.

The real contract is **not the string passed to the hypervisor but the pixel
dimensions the RFB framebuffer reports back** — that framebuffer is what the
`testanyware-rfb` stack negotiates (`connection.rs:131` reads width/height from
the server's ServerInit), what `screen size` reports, and what the vision
pipeline consumes.

A platform asymmetry complicates hitting that contract. `tart set --help`
(tart 2.32.1) documents `--display` as `WIDTHxHEIGHT[pt|px]` where the unit is a
*hint* defaulting to **pt (points) for macOS VMs** and **px (pixels) for Linux
VMs**. On a Retina/HiDPI macOS guest, a bare `1920x1080` is therefore 1920×1080
*points* → a **3840×2160 *pixel* framebuffer** at 2× backing scale, overshooting
the vision target. QEMU's `xres`/`yres` are pixels directly, and QEMU's backend
parser splits the display string on `x` — so a `px`-suffixed string would break
it (`1920x1080px` → `yres=1080px`, invalid).

## Decision

**Introduce a TestAnyware-owned default guest display resolution of 1920×1080
pixels, applied at `vm start` when `--display` is omitted.** The contract is the
*framebuffer*: 1920×1080 **px** on every platform.

- The default is **resolved per-backend**, because each backend owns its
  encoding: tart emits **`1920x1080px`** (explicit `px` to defeat the macOS
  points default); QEMU emits **`xres=1920,yres=1080`**.
- A user-supplied `--display` value is **passed through untouched** — we set a
  default, we do not rewrite explicit input.
- Scope is **`vm start` only** — not golden-image baking (golden creation is
  provisioning; the running test instance is where resolution matters) and not a
  guest-side configuration step.

The value is **1920×1080**, chosen to match the vision training distribution
exactly so guests come *onto* distribution with **zero retraining**. A larger
default (e.g. 2560×1440) was rejected: it would push guests *off* the training
distribution and require regenerating synthetic data and retraining the vision
models — a separate workstream, not a default change.

## Consequences

- Guests boot at a known, uniform 1920×1080-px framebuffer instead of an unknown
  per-hypervisor default; vision/OCR input is on-distribution and apps under test
  get materially more room.
- A future reader will see the macOS default carry `px` while Linux/Windows do
  not — this ADR is the answer to "why": the tart pt/px hint asymmetry.
- The pre-existing `--display` flag is a latent footgun on macOS (a user typing
  `1920x1080` gets points → a 2× framebuffer) and on QEMU (a `px` suffix breaks
  the `x`-split). This ADR does not fix the user-facing flag — only the default
  we emit — but records the asymmetry so it is not rediscovered the hard way.
- **Verification owed (work leaf `implement-default-resolution-k2`):** confirm
  empirically via `screen size` against a running golden that the framebuffer
  reports exactly 1920×1080 px on each platform — macOS especially, since VF may
  snap `1920x1080px` to a nearby supported mode rather than honoring it exactly;
  and that tart `--display-refit` does not perturb a fixed-resolution headless
  VNC session. If VF will not yield exactly 1920×1080 px, this ADR is amended.
- A 1920×1080-px framebuffer moves more bytes per RFB `FramebufferUpdate` than
  the prior small defaults (≈8 MB/frame uncompressed); the embedded viewer and
  `screen record` will work on larger frames. Expected acceptable; flagged for
  the work leaf to watch.

## Verification (2026-06-24, `implement-default-resolution-k2`)

Empirically checked via `screen size` (the negotiated RFB framebuffer — the
contract) against running goldens on a macOS host, `vm start` with no
`--display`:

- **Linux (tart): ✅ 1920×1080 px.** Stable; the guest renders a crisp LoDPI
  desktop. The clone's `tart get` shows `Display: 1920x1080px`; the Linux guest
  honors the host-configured display mode directly.
- **macOS (tart): ❌ 1024×768 px.** The clone's `tart get` *correctly* shows
  `Display: 1920x1080px` (our `px` encoding is accepted and stored) and the
  guest is **fully logged in** — yet the framebuffer is 1024×768. A macOS VF
  guest's framebuffer is **guest-controlled**: WindowServer restores the guest's
  *own saved display mode* on login (1024×768, baked into the golden — whose
  `tart get` shows `Display: 1024x768`), and Virtualization.framework sizes the
  actual framebuffer to the guest's chosen mode. The host-side `tart set
  --display` sets the VM's display *configuration* (a ceiling / available-mode
  hint) but does **not** force the guest to switch modes. (`--display-refit`
  is a window-fit option, irrelevant to a headless VNC session.)
- **Windows (QEMU): not run** (no Windows golden on the verification host).
  Mechanically expected correct and unit-tested: `build_qemu_args` emits
  `virtio-gpu-pci,xres=1920,yres=1080`, and virtio-gpu's `xres`/`yres` size the
  framebuffer directly with no guest-side override — the same path Linux-on-qcow2
  would take.

**Finding.** The "applied at `vm start`, not golden-image baking" scope above is
**sufficient for QEMU and for Linux-on-tart, but insufficient for macOS-on-tart.**
Reaching a 1920×1080-px framebuffer on macOS requires changing the **guest's**
saved resolution — baking it into the macOS golden (`golden.rs`) or setting it
guest-side at start — i.e. exactly the golden-side work this ADR scoped out.

**The decision stands for what it covers.** The per-backend default is the
correct `vm start`-layer change (necessary on every backend, fully sufficient on
QEMU + Linux-tart), and the tart `px` encoding is confirmed. The macOS gap was
resolved by the follow-up planning leaf `macos-guest-resolution-k3`, which chose a
**runtime, agent-mediated guest-side switch** (over golden-bake) — see **ADR-0014**.
That ADR also corrects a premise stated above ("VF may snap…") and, more
importantly, the assumption that blocked a guest-side approach: the in-guest agent
**does** expose a generic `/exec` channel, so guest-side setting is feasible.
Implementation is deferred to a dedicated grove.
