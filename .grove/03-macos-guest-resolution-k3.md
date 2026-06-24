# macos-guest-resolution-k3

**Kind:** planning

## Goal

Decide how (or whether) to make a macOS guest's RFB framebuffer come back
**1920×1080 px** under `vm start` with no `--display`, given the empirical
finding that the host-side `tart set --display 1920x1080px` does **not** achieve
it. Then grow the work leaf/leaves — or consciously close the macOS contract as
a documented limitation.

## Context

`implement-default-resolution-k2` (DONE) implemented the per-backend default and
verified it empirically (`screen size` against running goldens, 2026-06-24):

- **Linux (tart) ✅ 1920×1080**; **Windows (QEMU)** mechanically correct +
  unit-tested (not run — no Windows golden on the host).
- **macOS (tart) ❌ 1024×768.** The clone's `tart get` correctly shows
  `Display: 1920x1080px` and the guest is fully logged in, yet it renders
  1024×768. A macOS VF guest's framebuffer is **guest-controlled**: WindowServer
  restores the guest's *own saved mode* on login (1024×768, baked into the
  golden) and Virtualization.framework sizes the framebuffer to it. The host
  `tart set --display` is only a ceiling/hint, not a forced mode. See ADR-0013
  "Verification (2026-06-24)".

This reopens the **golden-baking scope** ADR-0013 deliberately set aside
(§Decision: "Scope is vm start only — not golden-image baking"). So this is a
design decision, not a mechanical fix.

## Open questions to grill

1. **Is 1920×1080 on macOS a hard requirement, or acceptable to defer / document
   as a known limitation?** The brief's "Done when" says every backend — but the
   macOS vision-testing priority vs the macOS-golden-rebuild cost is the user's
   call to make.
2. **Mechanism, if pursued:**
   - **(a) Bake into the macOS golden** — set the guest's WindowServer resolution
     preference to 1920×1080 during golden creation (`golden.rs` / `vm
     create-golden`), so the guest restores 1920×1080 on login. Requires
     regenerating/patching the kept-built macOS golden.
   - **(b) Guest-side set at `vm start`** — switch the guest display mode after
     boot (a `displayplacer`-style CoreGraphics call). Blocked on no exec
     channel: Remote Login (sshd:22) is **off** in the golden and the agent is
     UI/accessibility-only (no generic exec) — would need a new guest capability,
     likely larger than this grove.
   - **(c) Accept as a documented limitation** — macOS guests render at the
     golden's baked resolution; close the grove on Linux/Windows.
3. **If (a): what saved-mode mechanism does VF/macOS use** — which
   `com.apple.windowserver` plist / display identity, and does it survive
   clone + `tart set --display`? Needs guest introspection (recovery cycle, or a
   one-off golden boot with a temporary exec path).

## Done when

The macOS path is decided (golden-bake / guest-side / accept-limitation),
recorded in ADR-0013 or a new ADR, and either the work leaf/leaves are grown or
the grove's macOS contract is consciously closed.

## Notes

- Verification recipe (reuse from k2): `vm start --platform macos` (no
  `--display`) → `screen size --vm <id> --json` must report 1920×1080;
  `tart get <id>` shows the VM's configured `Display`; `screen capture -o …`
  to eyeball. **`vm stop` takes a positional id** (`vm stop <id>`, not `--id`).
  Golden clone+start is cheap (`vm-costs`).
- Any guest-side approach must first answer "how do I run a command in a booted
  macOS guest" — no SSH, agent is UI-only.
