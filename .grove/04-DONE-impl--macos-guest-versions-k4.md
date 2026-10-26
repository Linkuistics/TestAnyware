# macos-guest-versions-k4

## Goal

Remove every macOS guest version older than 27 from TestAnyware. Golden creation
for macOS then builds only from Apple's IPSW, for macOS 27 or a later major.

## Context

- The running log of `plan-k1` holds the scope that the human chose and the
  sweep of the consumers.
- Code: the version selector (`base_source`) and golden creation (`golden.rs`,
  `ipsw_base.rs`) in `testanyware-vm`. The help text of `vm create-golden` in
  the CLI. The schema `docs/reference/cli-schemas/vm-create-golden.json`. The
  name test `platform_from_name` in `qemu.rs`.
- Docs: `README.md`, `CONTEXT.md` (the term **Base VM**), `docs/user/`,
  `docs/reference/cli-commands.md`, `docs/architecture/vm-lifecycle.md`,
  `docs/components/`, and their copies under `website/docs/`.
- Tests and checks that name `tahoe`: check 3 of `scripts/queue-live.sh`,
  `tests/app-vm-queue.rs`, `tests/app-version-skew.rs`, the default version of
  `tests/recovery-live.rs`, and the fixtures in `tart.rs`.

## Done when

- `vm create-golden --platform macos --version <v>` accepts macOS 27 or a later
  major only. `tahoe`, `sequoia`, `sonoma` and a major below 27 fail in the fast
  checks, before any clone or download. The error is a usage error that names
  macOS 27, and `task cli:test` shows it.
- Linux golden creation and its Cirrus Labs image stay as they are.
- No document offers an older macOS guest. A sweep with a positive control
  shows it.
- Each test and live check that used `tahoe` uses macOS 27 instead, and it
  passes.
- A statement can name Tahoe but describe behaviour that macOS 27 shares. Such
  a statement stays, with corrected words. Examples are the drop-shadow inset of
  a window and the Command key as `XK_Alt_L`. Check each one on macOS 27 before
  you change its words.
- Evidence of past measurements on a Tahoe clone stays as it is. This covers
  ADRs, research notes, plans and recorded runs.
- A new sweep of the consumers under `~/Development` finds no consumer that the
  removal breaks. If it finds one, tell the human its name.
- This leaf applies the ADR test of `ADR-FORMAT.md` to the removal, and it
  writes an ADR when the test passes.

## Notes

- Keep the golden `testanyware-golden-macos-tahoe` on this Mac. It is the data
  of the human. Tell the human that this command frees about 29 GB:
  `tart delete testanyware-golden-macos-tahoe`.
- Hosts keep macOS 14 or later. Remove nothing that serves an older host.

## Decisions (running log)

**The version selector is one pure function, and the error reuses `USAGE_ERROR`.**
`macos_major` in `base_source.rs` replaces `base_source`, `BaseSource` and
`vanilla_image`. It accepts an all-digit value of 27 or more. Everything else
fails with `VmError::UnsupportedMacosVersion`, code `USAGE_ERROR`, exit 2, and a
message that names macOS 27. No new error code means the list of codes in the
CLI surface does not grow.

The check runs first in `run_vm_create_golden_macos`. So it also fails under
`--dry-run`, and before the directory check, the fast checks and the wait for a
slot. The contract test
`vm_create_golden_macos_accepts_macos_27_or_later_only` shows it, and it fails
when the minimum drops to 0.

**The fake `tart` of the routing tests lists `testanyware-base-macos-27`.** A
golden creation of macOS 27 asks `tart list` for the base VM. Without it, the
pipeline would fetch Apple's catalogue after its slot. The fake lists the base
and cannot clone it, so the pipeline still stops at its first clone.

**`task queue:live` passed on the installed release 2.6.0, not on the
development bundle.** The first run rebuilt `TestAnyware Dev.app`. It signed the
bundle again. So macOS asked for the Local Network permission, and the start of
check 1 failed with the remedy. Only a person grants that.

The change of this leaf lives in the Host CLI. The app side of check 3 is the
same code in both bundles. So the run with `APP=/Applications/TestAnyware.app` and the Host CLI of this tree
passed all three checks, with `--version 27` in check 3.

**Three statements that named Tahoe, checked on a macOS 27 guest (26 October
2026).** The guest started from `testanyware-golden-macos-27` through the app of
2.6.0, and a small AppKit window ran in it.

- The Command key is `XK_Alt_L`. `input key a --modifiers cmd` selected every
  item of a Finder window, and `--modifiers alt` (`XK_Meta_L`) selected only
  "App Store", as type-ahead does. macOS 27 shares the statement. It stays, with
  the word "macOS".
- The AX window origin includes a drop-shadow inset of about 40 px. macOS 27 does
  not share it. For the probe window and for a Finder window, the AX `positionY`
  equals the visible top edge (508 and 164). `input click --window Probe 16 16`
  with the default inset landed at y=484 and missed the close button. With
  `TESTANYWARE_WINDOW_TOP_INSET=0` it landed at y=524 and closed the window. So
  the default of 40 px misplaces `--window` clicks on macOS 27. This leaf does not
  change the default: that changes what a client of the CLI sees. The docs state
  the behaviour as measured, with the override as the remedy, and the final
  message tells the human.
- Text fields in an `NSStackView` do not resolve on Tahoe. macOS 27 lists both
  fields in `agent snapshot`, and `agent set-value` reaches each one by `--label`
  and by `--index` (1-based). It does not share the statement, so the
  troubleshooting entry and the bullet of the agent guide go.

**Statements that stay as they are.** Recorded measurements: the `macOS 26.x`
stuck count in the spec admission-queue, `macOS 15.5` in the spec testanyware-app,
and the Terminal anchors read on Tahoe in `recovery.rs`. The csreq note in
`finalize.rs` also stays: it records what Tahoe required, and the pipeline still
inserts the field. The fixture `macOS Sequoia 15.1` in `golden.rs` is test data of
an IPSW catalogue parser, not a guest that any document offers.

**The ADR test fails for the removal, so this leaf writes no ADR.** The test
needs all three of: hard to reverse, surprising without context, and a real
trade-off with a rejected alternative. The removal is easy to reverse: one
revert restores the Cirrus path, and the goldens that exist stay on disk. The
choice is a scope decision of the human. The user guide and the commit record it.

ADR-0007 and ADR-0008 describe how golden creation provisions a guest. Neither
names an older guest as a design point, so the set needs no rework.

**The sweep of the consumers under `~/Development` finds no consumer that the
removal breaks.** Only `create-golden` with a version of 26 or older breaks. No
script outside TestAnyware passes one. The scripts and skills that call
`create-golden` pass `--platform` only.

Starting a clone of the existing `testanyware-golden-macos-tahoe` still works.
The removal touches golden creation, not `vm start`. Two things need telling to
the human. First, `UIAnyware/ci/README.md` line 110 names a
`testanyware-golden-macos-tahoe` image that `create-golden --platform macos`
does not build. It has not built it since 27 became the default. Second, the
published copy of the docs under `Delivery/` still describes the Tahoe golden
until the next publish.
