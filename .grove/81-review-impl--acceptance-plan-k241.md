# acceptance-plan-k241

**Reviews:** acceptance-plan-k223

## Goal

Try to break the acceptance trial plan and checker of acceptance-plan-k223.
Leaf acceptance-preparation-k224 freezes them next, and k225 runs ten clones
under them. Find each case where the plan does not run the root inventory.
Find each record or evidence that the checker judges wrongly. A wrong pass
and a false failure both count.

## Context

- Root brief: sections "Done when", "Constraints" and "Acceptance inventory
  and bounds".
- Spec `docs/specs/guest-command-lifecycle.md`: sections "Exec record",
  "Validity rules", "Host CLI semantics", "Bounds" and "Validation route".
- The contract of k223 is its `Done when` list and its notes.
- The code, in `experiments/exec-reliability`:
  - `acceptance_k223.py`: the plan, the manifest reader and the trial driver.
  - `acceptance_k223_cancel.py`: the explicit cancellation helper.
  - `acceptance_k223_check.py`: validity, bounds, case judgements and the
    corruption controls.
  - `test_acceptance_k223.py`: the controls.
  - The reused parts: `controller.py`, `routes_k19.py`, the cleanup of
    `linux_k204.py` and the retained fixtures of `timeout_k4.py` and
    `linux_k204.py`.
- `self_check.py`: k223 changed its pytest budget.
- `Taskfile.yml`: `diagnostic:acceptance-trial` and
  `diagnostic:acceptance-check`.
- The Host CLI reader of the record:
  `cli-rs/crates/testanyware-protocol/src/exec_record.rs` and
  `cli-rs/crates/testanyware-cli/src/commands/file.rs`.
- The report `docs/experiments/exec-reliability-acceptance-plan.md` and the
  evidence in `experiments/exec-reliability/evidence/acceptance-plan-k223/`.

## Specific doubts

- Compare the case order and the case count with the root inventory. Check
  each reading of "ten after-timeout operations and child checks".
- Compare the validity rules with the spec and with `exec_record.rs`. Find a
  valid record that fails, or an invalid record that passes. Try both leader
  orders and `leaderStatus` `unavailable`. Try an incomplete stream and each
  legacy projection.
- Find each ceiling or bound of the root brief or the spec that is not a
  checker limit. Find each limit with a wrong value or observation.
- Find a damaged copy that a corruption control accepts. Check that each
  control damages an independent copy and that the originals stay unchanged.
- Check the helper for races, unbounded waits and children that outlive it.
  Check that its waits are trigger times and not padding.
- The checker uses an inequality to show that health ran during the command.
  Check that the inequality is sound across host and guest clocks.
- The abrupt interruption runs a child check before its fallback
  cancellation. Check that an absent child proves the cancellation of the
  Host CLI. Check that a transient state cannot fail a correct clone.
- Check that the trial stops at the first failed clone. Check that it keeps
  every failed sample. Check that the reset record claims nothing more than the
  next baseline shows.
- k223 raised the self-check pytest budget from 120 s to 300 s. Check that
  this change cannot hide a stall or a defect.
- Check that the manifest contract is enough for k224 to freeze every
  subject, ceiling and bound.

## Done when

- Each finding names the code, the input and the observed or derived wrong
  result.
- The review reports findings. It changes no code.

## Notes

Review the producer commit of acceptance-plan-k223 against the current
source. Leave each code change to the integration step. Boot no guest, and
do not patch the global CLI or app.
