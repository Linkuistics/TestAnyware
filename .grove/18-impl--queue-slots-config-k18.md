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
