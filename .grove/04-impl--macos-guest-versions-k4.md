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
