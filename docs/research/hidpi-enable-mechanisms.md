# Enabling HiDPI/Retina (2× backing scale) on a macOS Virtualization.framework guest — mechanism survey + spike findings

**Status:** research + spike complete, 2026-06-25. Commissioned by grove
`hidpi-vision`, leaf `spike-hidpi-feasibility-k2`.
**Audience:** the feasibility gate that unblocks the grove's downstream
target-shape + vision-disposition design, and any future work that needs a
Retina macOS guest under VF/tart.
**Bottom line:** **No guest-side mechanism can make a VF virtual display
advertise a 2× mode.** The advertised CoreGraphics mode list — including each
mode's *backing scale* — is derived from the **host-side** VF display
configuration (`VZMacGraphicsDisplayConfiguration`), not reachable from inside
the guest. Reaching HiDPI is therefore a **host-side** change, and tart as
shipped cannot do it deterministically (it hardcodes a non-Retina ppi on the
headless path and otherwise inherits the host monitor's scale). The viable path
is a **tart fork / custom VF harness** that injects an explicit high
`pixelsPerInch`. Decision recorded in **ADR-0015**.

> **Why this matters for the grove.** The grove's driver is *test realism*
> (plan-k1 D1): apps under test should render at the 2× backing scale real Macs
> use. The whole downstream design (the core tension: 2× ⇒ `framebuffer_px =
> logical_pt × 2`, the 3840×2160-px → downsample-2:1 → vision's 1920×1080-px
> path) is gated on whether a 2× framebuffer is even reachable. This note is
> that gate.

---

## The one invariant that decides this

A macOS guest renders at the resolution **and backing scale** of the
**host-side `VZMacGraphicsDisplayConfiguration`** that VF hands it. Two axes,
both host-owned:

- **Mode sizes** (which logical resolutions appear in
  `CGDisplayCopyAllDisplayModes`) follow the config's **point dimensions**.
- **Backing scale** (1× LoDPI vs 2× Retina) follows the config's
  **pixel-to-point ratio**, which tart derives from `pixelsPerInch` (px path)
  or from the host `NSScreen.backingScaleFactor` (pt path).

The guest's WindowServer then *selects* a mode from that host-defined list and
VF sizes the [[Host-side framebuffer]] to it ([[Guest-controlled resolution]]).
Selection is guest-side; **the menu is host-side**. Every guest-side mechanism
below fails on this invariant: you cannot select a 2× mode that the host config
never put on the menu, and no guest-side write reaches back into the VF config.

---

## Mechanism 1 — `/Library/Displays/Contents/Resources/Overrides/` HiDPI override plist

**The canonical "force HiDPI on a display that doesn't advertise it" trick for
physical external monitors.** Write a
`DisplayVendorID-<hex>/DisplayProductID-<hex>` plist with a `scale-resolutions`
array whose entries carry a HiDPI marker; enable
`com.apple.windowserver DisplayResolutionEnabled`; reboot.

**Primary sources**
- `xzhih/one-key-hidpi` — `hidpi.sh`
  (https://github.com/xzhih/one-key-hidpi/blob/master/hidpi.sh): target dir
  `targetDir="/Library/Displays/Contents/Resources/Overrides"`, file at
  `DisplayVendorID-${Vid}/DisplayProductID-${Pid}`. IDs derived from
  **IORegistry** (Apple Silicon: `DisplayAttributes` →
  `LegacyManufacturerID`/`ProductID`), not from CoreGraphics. Activation:
  `defaults write … DisplayResolutionEnabled -bool YES` + **reboot**; the tool
  contains **no `killall WindowServer`**.
- The HiDPI byte encoding of a `scale-resolutions` `<data>` entry:
  `<px_w be32> <px_h be32> 00000001 00200000` — the trailing 8 bytes flag the
  entry as HiDPI. Corroborated by the Comsysto/Grünewaldt writeup
  (https://medium.com/comsystoreply/force-hidpi-resolutions-for-dell-u2515h-monitor-5304e5506214):
  "the two last blocks are fixed values to flag the resolution as HiDPI."
- **Direct negative result for a *virtual* display:** BetterDisplay discussion
  #1747 — "there will be no HiDPI scaling options no matter what you do
  (including editing display's PLIST and disabling SIP of VM)." The reported fix
  was at the hypervisor layer, not the plist.
- **No source found** decoding the bits of `0x00200000`; **no Apple doc** for
  `scale-resolutions`; **no primary source** that a `killall WindowServer`
  alone (vs reboot) activates an override.

**Spike result: REFUTED (measured).** See Verification below — installing the
override (both candidate vendor keyings) + `DisplayResolutionEnabled` + a full
guest reboot produced **zero** change to the advertised mode list. The VF
virtual panel is **identity-less** (CG vendor/model `0/0`; no `IODisplayPrefsKey`
or `LegacyManufacturerID` in its IORegistry node), so the EDID-keyed override
never binds — and even when bound it cannot create a backing scale the host
config did not.

## Mechanism 2 — private CoreGraphics Services / SkyLight HiDPI APIs

Undocumented `CGS*`/`SLS*` calls that some tools use to select or inject modes.

**Primary sources**
- `NUIKit/CGSInternal` — `CGSDisplays.h`
  (https://github.com/NUIKit/CGSInternal/blob/master/CGSDisplays.h): the real
  family is `CGSGetNumberOfDisplayModes`, `CGSGetDisplayModeDescription…`,
  `CGSConfigureDisplayMode(config, display, modeNum)`. Note
  `CGSConfigureDisplayMode` takes a **mode *index***, not a fabricated struct —
  it can only *select an already-enumerated mode*.
- The HiDPI marker in `CGSDisplayModeDescription` is a trailing **`CGFloat
  scale`** (`2.0` ⇒ HiDPI), per the header and the maintained Rust tool `knoll`
  (https://docs.rs/knoll/) ("If 2.0, the mode is scaled.").
- Symbols migrated **CoreGraphics → SkyLight.framework** at macOS 10.13
  (`CGS*` → `SLS*`); modern apply entry point `SLConfigureDisplayWithDisplayMode`.
- **Modern Apple Silicon blocks struct injection:** `sammcj/force-hidpi`
  + smcleod.net (2026-03) report `SLConfigureDisplayWithDisplayMode` "returns
  error 1000 when the mode is not in the display's own mode list" and
  "validates modes against the same DCP-derived mode list as WindowServer."
  Current tools therefore fabricate HiDPI by creating a **`CGVirtualDisplay`**
  at 2× and mirroring — not by injecting a CGS mode.
- **`CGSConfigureDisplayEnabledForHiDPI`: no primary source found** — absent
  from `CGSDisplays.h` and every authoritative symbol list. Treat as
  confabulated unless an `nm`/`dlsym` dump surfaces it.

**Spike result: REFUTED by construction (not separately measured).** This family
*selects* an advertised mode; it does not *create* one. Since the premise is
that the VF guest advertises only 1× modes (measured), and Apple-Silicon
validation rejects out-of-list modes, there is no guest-side injection primitive.
A `CGVirtualDisplay` would create a *second, fake* display — not make the *app
under test's real display* Retina — so it does not serve the realism driver.

## Mechanism 3 — host-side `VZMacGraphicsDisplayConfiguration` ppi (the viable path)

**Primary sources**
- Apple docs
  (https://developer.apple.com/documentation/virtualization/vzmacgraphicsdisplayconfiguration):
  initializer `init(widthInPixels:heightInPixels:pixelsPerInch:)`;
  `pixelsPerInch` documented only as "pixel density." **No documented Retina
  threshold** — the "~220 ppi" figure is community lore, no Apple source.
- Positive report that high ppi yields a Retina guest: Eclectic Light Co.
  (Howard Oakley)
  (https://eclecticlight.co/2022/08/01/virtualisation-on-apple-silicon-macs-7-improving-the-virtual-display/):
  ppi 80/109 → guest did **not** enable HiDPI; ppi = host's own 218 → HiDPI
  auto-selected. Implies the trigger is **host-relative**.
- `s-u/macosvm` (https://github.com/s-u/macosvm, `VMInstance.m`): defaults to
  **`2560x1600 @ 200 dpi`** and exposes `dpi` in JSON — i.e. a working VF
  harness that sets a high ppi directly, *independent of any host screen*. This
  is the existence proof that the host-side path works headless.
- **tart, verified against `main`** (`Sources/tart/Platform/Darwin.swift`):
  ```swift
  if (vmConfig.display.unit ?? .point) == .point, let hostMainScreen = NSScreen.main {
    result.displays = [VZMacGraphicsDisplayConfiguration(for: hostMainScreen,
                                                         sizeInPoints: vmScreenSize)]
  } else {
    result.displays = [VZMacGraphicsDisplayConfiguration(
      widthInPixels: …, heightInPixels: …, pixelsPerInch: 72)]   // ← non-Retina
  }
  ```
  tart's `--display` is `WIDTHxHEIGHT[pt|px]` — **no ppi field**. So tart's only
  Retina path is the `pt` branch, which **inherits the host monitor's backing
  scale** via the `for:` initializer; the `px`/headless branch hardcodes
  **`pixelsPerInch: 72`** (1×).

**Spike result: characterized, viable, but tart cannot do it deterministically
(measured).** On a **1× host** (`NSScreen.main.backingScaleFactor == 1.0`),
`tart set --display 1920x1080pt` expanded the advertised list to 12 modes up to
1920×1080 but **all at scale 1.0** — no Retina mode (Verification below). The
`pt` path's HiDPI behaviour is thus **coupled to the host's physical monitor**:
non-deterministic across dev/CI hosts and absent on any 1×/headless host. A
deterministic 2× requires injecting an explicit high `pixelsPerInch`, which tart
does not surface → **fork tart** (add a ppi field / Retina-display branch) or run
a **macosvm-style custom VF harness**.

---

## Verification (2026-06-25, `spike-hidpi-feasibility-k2`)

Method mirrors ADR-0014's spike: host-compiled CoreGraphics probe
(`provisioner/helpers/probe-hidpi-identity.swift`) `/upload`+`/exec`'d into a
fresh `testanyware-golden-macos-tahoe` clone over the agent's HTTP surface;
framebuffer read via `testanyware screen size` (the negotiated RFB ServerInit).
No golden regeneration. Guest: macOS 26.5 (25F71); agent runs as `admin` with
**passwordless sudo**; **SIP enabled**.

**The VF virtual display is identity-less.** `CGDisplayVendorNumber == 0`,
`CGDisplayModelNumber == 0`, `CGDisplaySerialNumber == 305419896` (`0x12345678`,
a placeholder). IORegistry `DisplayAttributes.ProductAttributes`:
`ManufacturerID="APP"`, `ProductID=0`, `ProductName="Apple Virtual"`, native
1024×768. **No `IODisplayPrefsKey`, no `LegacyManufacturerID`.**

**Baseline (golden's 1024×768 px config):** 4 modes, **every one scale 1.0**
(`retinaModeCount=0`): 800×600, 1024×768 (default+native), 512×384, 640×480.
`screen size` = 1024×768.

**Mechanism 1 (override plist) — REFUTED.** Wrote the HiDPI override to
`/Library/Displays/.../Overrides/DisplayVendorID-610/DisplayProductID-0` **and**
`DisplayVendorID-0/DisplayProductID-0` (covering both the EDID `APP`→`0x610` and
the CG `0` keyings), `defaults write … DisplayResolutionEnabled -bool true`, then
a full guest **reboot**. `/Library/Displays` was created and written cleanly with
`sudo` under SIP enabled, and both files + the pref **survived the reboot** — but
the post-reboot mode list was **byte-identical** (4 modes, all scale 1.0,
`retinaModeCount=0`). `screen size` = 1024×768. Zero effect.

**Mechanism 3 (tart `pt` path) — host-coupled, no Retina on a 1× host.** Host
`NSScreen.main.backingScaleFactor == 1.0` (frame 5120×2160 — a LoDPI ultrawide).
`tart set --display 1920x1080pt` + `tart run --no-graphics --vnc-experimental`:
the guest advertised **12 modes up to 1920×1080, all scale 1.0**
(`retinaModeCount=0`), and `screen size` = 1024×768 (the guest restored its saved
mode). This cleanly isolates the two axes: the **point dimensions** of the config
set the advertised *sizes*; the **host backing scale** sets their *scale*. With a
1× host, every mode is 1×.

**Q2 (framebuffer under a 2× mode) — answered by the contract.** No 2× mode was
selectable here, so this was not directly measured at 2×. But `screen size`
reports the **px** framebuffer (ADR-0014: pre-switch 1024×768 px → post-switch
1920×1080 px, all where px==pt), and `VZMacGraphicsDisplayConfiguration`
width/height are **pixels**. So under a host-injected 2× config of logical
1920×1080, `screen size` would report **3840×2160 px** — i.e. the framebuffer is
reported in **px**, which is exactly the precondition the grove's downscale
design (plan-k1 D2: 3840×2160 → downsample 2:1 → vision's 1920×1080 px) needs.
The downscale design **applies**.

**Verdict:** **guest-side REFUTED, host-side viable via a tart fork / custom VF
harness.** See ADR-0015.
