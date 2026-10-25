# queue-slots-config-k18

## Goal

`config.rhai` can lower the share of the two slots that the app may hold to one
or zero. A person can then keep a slot for a VM of their own.

## Context

- Spec sections "Configuration" and "What a start does when `macos_slots` is 0"
  (in "The wait and its end"), and the requirement "`config.rhai` lowers the
  share".
- ADR-0021: one Rhai file configures the app and the CLI. The shared evaluator is
  the crate `testanyware-config`.
- `queue-order-k16` fixed the share at 2 in the count. This leaf makes it a
  setting.

## Done when

- `macos_slots` is an integer 0, 1 or 2, and the default is 2. Another value or
  another type is `CONFIG_INVALID`. The shared evaluator checks it for the app
  and for the Host CLI on every host. Only the app uses the value.
- The count admits while own holders are below `macos_slots`. The `limit` of
  `VM_LIMIT_REACHED` is `macos_slots` when the share is full.
- With 1, a second start waits while a first guest runs. With 0, every macOS
  start fails at once with `VM_LIMIT_REACHED` and `limit` 0, with `--wait` or
  without it.
- The file that the app writes when it finds none names the setting in an
  example line that starts with `//`. The tests of that text and of the write
  step change with it. A test-mode run still writes nothing.
- `docs/reference/env-vars.md` lists `macos_slots` in its table of `config.rhai`.
  The rule of an absolute path names the path settings only.
- The tests run under `task config:test`, and the task lists any new target.

## Notes

- An older release reads `macos_slots` as an unknown key and fails with
  `CONFIG_INVALID`. The spec accepts that risk. The documents say so.

## Decisions (running log)

**The key is checked in the shared evaluator, as the spec says.** `macos_slots` is an
`Option<usize>` in `Config`. The evaluator reads it with Rhai's `as_int`, so `1.0`, `"1"`
and `true` fail with "must be an integer, but it is <type>", and 3 or -1 fail with "must
be 0, 1 or 2". Neither error has a line, as for every check after the script ends. The
Host CLI evaluates the same file on every host, so a wrong value stops each command there
too. The CLI never reads the number.

**The default is a constant of the config crate.** `DEFAULT_MACOS_SLOTS` is 2. The app's
`MACOS_SLOTS` constant is gone. `AppConfig::macos_slots()` gives the file's value, and 2 for
a missing file, a file that fails and a run with no file. `AdmissionQueue::for_paths` takes
the number, and `AppContext::new` passes it.

**A share of 0 skips the queue.** `decide()` already blocked every waiter at 0, but the
head would then wait out its whole deadline. `admit` now sends a start with `slots == 0` to
the one decision of a start without `--wait`, which always refuses it. It makes no wait, no
place in the queue and no `tart run`. The mutation that removes this line hangs the test on
the 600 s wait.

**`limit` is the share when the own holders fill it, else 2.** The spec table says
"`macos_slots` when the app's share is full, and 2 otherwise". `refusal()` now asks
`limit(state)`. A framework refusal beside a guest that the app cannot see, with the share
not full, reports 2. With the default share the value is unchanged.

**The written file names the key in a comment only.** The example line is
`//  macos_slots: 2,` with prose above it that gives the range, the effect of 0 and the
risk for an older release. The test that deletes the `//` of each example now also sees
`macos_slots` set. A separate test checks that the shipped text sets nothing, so an older
release still reads it.

**One accident, repaired.** `cargo fmt --all` reformatted 84 files that this leaf does not
touch, because the workspace is not fmt-clean. They were restored from the parent change
with `jj restore`, and the two reformatted hunks in `tests/common/mod.rs` were put back by
hand. The diff now holds only this leaf's 14 files. Do not run `cargo fmt --all` here.

**The tests.** The evaluator, the default text, the app's config, the queue and the real CLI
against the real app (share 1 waits and the framework never refuses; share 0 fails three
starts at once with `limit` 0 and leaves no clone) all run under `task config:test`, which
lists two new filtered targets. The queue tests of `app-vm-queue` also stay in
`task app:routing-test`.

**The documents of this scope.** `docs/reference/env-vars.md` lists `macos_slots`, says
that it is an integer and not a path, and warns that an older release fails on the key.
The spec and the ADRs already state all of this, so they did not change.

**No review leaf, and the in-session allowance stays unspent.** The new logic is one
guard and one comparison, and the tests cover each branch, with one mutation checked.
