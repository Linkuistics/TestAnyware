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
