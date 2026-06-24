# confirm-hidpi-pt-path-on-retina-host-k4

**Kind:** work (empirical spike / de-risk)

## Goal

Empirically confirm — **on a Retina host** — that shipped tart's host-scale `pt`
display path actually delivers a 2× HiDPI framebuffer, and pin the `vm start`
sequencing the opt-in (k6) needs. This is the **load-bearing de-risk**: ADR-0015
*derived but did not measure* the 3840×2160 framebuffer (the spike host was 1×).
This leaf is the doubt pass for that claim; k5/k6 build against its findings.

## Context

Read first: **ADR-0016** (the build design — start here), **ADR-0015** (feasibility
floor) + `docs/research/hidpi-enable-mechanisms.md` (Mechanism 3: the `pt` path
inherits the host monitor's backing scale), **ADR-0014** (the guest-side 1× switch
this must interact with), `CONTEXT.md` `[[HiDPI logical framebuffer]]` +
`[[Guest-controlled resolution]]`.

Key facts:
- shipped tart's `pt` path (`Darwin.swift`:
  `VZMacGraphicsDisplayConfiguration(for: hostMainScreen, sizeInPoints:)`) inherits
  the **host monitor's** backing scale — so it only yields 2× on a **Retina host**.
  `--display` is passed untouched today (`tart.rs` `resolve_display`/`set_display`),
  so `vm start --platform macos --display 1920x1080pt` is testable *as-is*.
- The macOS golden boots into its WindowServer-saved mode (1024×768 — ADR-0014
  finding 2), so reaching a 1920-logical mode may need a **guest-side mode switch**
  even under a 2× host config. Reuse the `.forSession` configuration-transaction
  pattern from `provisioner/helpers/{probe-display-modes,set-display-mode}.swift`
  and the ADR-0015 probe `provisioner/helpers/probe-hidpi-identity.swift`.
- Method to mirror: host-compiled CoreGraphics probe `/upload`+`/exec`'d into a
  fresh `testanyware-golden-macos-tahoe` clone over the agent HTTP surface;
  framebuffer read via `testanyware screen size` (the negotiated RFB ServerInit).
  No golden regeneration. See feedback memory `tart list state column` / VM SSH
  caveats.

**Precondition:** this needs a **Retina host**. First check
`NSScreen.main.backingScaleFactor` (or `system_profiler SPDisplaysDataType`) on the
dev host. If the host is 1×, record that as a finding (the whole minimal-opt-in path
is untestable here) and surface it — do not fake a result.

## Done when

- Confirmed (or refuted, with measurements) on a Retina host: `--display
  1920x1080pt` (the `@2x` translation) makes `screen size` report **3840×2160 px**,
  i.e. the guest renders logical 1920×1080 at 2× backing scale.
- The `vm start` sequencing question is **answered**: does the guest auto-select the
  Retina 1920-logical mode, or does it restore 1024×768 and require a guest-side
  switch to *select* the Retina mode? If a switch is needed, the exact CG mode
  selector (a mode where `pixelWidth==2*logical && width==logical`, scale 2.0) and
  transaction are characterized for k6 to build.
- Confirmed that ADR-0014's 1× `set-display-mode.swift` switch must be **suppressed**
  under `@2x` (it selects `pixelWidth==w && width==w`, which has no match at 2×).
- Findings recorded in ADR-0016's Verification (or a `docs/research/` note) — this
  resolves the one empirical unknown ADR-0016 flagged.

## Notes

Pure measurement + characterization; no production code. Its output is the
*sequencing contract* k6 implements and the *confirmation* that ADR-0016's
mechanism premise holds. If the host is 1× and no Retina host is available, this
leaf becomes "document the gap" and k6/k7 inherit the can't-fully-verify caveat.
