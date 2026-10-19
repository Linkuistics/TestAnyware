# vm-list-sidecar-vms-k5

## Goal

Make `vm list` show every VM TestAnyware started, whatever its id, not only
the tart clones whose names start with `testanyware-`.

## Context

- Decision D7 in the retired `plan-k1`.
- `plan-k1` fact 9: running tart clones are found only through `tart list`,
  filtered by the `testanyware-` prefix. So Koine's `koine-verify-<pid>` VMs
  never appear, and neither would a kept VM with a custom id.
- The listing combines goldens with running clones enriched from their
  spec/meta sidecars. The existing `list` unit tests build sidecars in a
  temporary state directory.

## Done when

- A running VM with TestAnyware sidecars is listed and enriched as today,
  whatever its id. Prefix-matched tart clones without sidecars are still
  listed.
- Sidecars whose VM is no longer running are handled by a deliberate choice
  (listed as stale, or ignored), and the help text documents it.
- Unit tests at the existing `list` seam cover it. Live: a
  `vm start --id koine-verify-…` appears in `vm list`.

## Decisions (running log)

- **A running tart VM is listed when it has the `testanyware-` prefix (not
  `testanyware-golden-`) or a spec or meta sidecar.** The rule is the pure
  `tart_clone_ids` in `lifecycle.rs`, with the sidecar check injected. QEMU needed
  no change: it already finds clones by scanning `clones/<id>/`, whatever the id.
- **Sidecars whose VM is not running are ignored, not listed as stale.** The
  candidates come only from `tart list`'s running names, so a stale sidecar cannot
  surface. Listing it would show a VM that no longer exists, and `vm stop <id>`
  already removes it. The help text, `cli-commands.md` and the contract's
  identifier table say so.
- **`vm delete`'s "a clone is running" guard uses the same rule.** Before, a
  running `koine-verify-*` clone would not have blocked deleting its golden.
  `tart::parse_running_ids` became `parse_running_names` (no prefix filter), and the
  filter moved up to where the sidecars are visible.

## Outcome

Unit tests at the `list` seam: four for `tart_clone_ids` (custom id with sidecars,
prefixed without, running golden, stale sidecar) and one for `has_sidecars`. The
custom-id test fails under the old prefix-only rule. Live, through Terminal
(`task vm:list-custom-id`, new): `vm start --id koine-verify-lc22265` reached a
Healthy agent and was listed enriched, then gone after `vm stop` with no clone left.
