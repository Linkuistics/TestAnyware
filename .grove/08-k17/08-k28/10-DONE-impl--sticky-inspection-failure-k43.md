# sticky-inspection-failure-k43


## Goal
Make local inspection failure sticky across one execution while still attempting
every owned resource's independent cleanup, disposal and public absence command.

## Context
Read the ancestor briefs and diagnostic-process-inspection spec. The synchronous
controller currently queries again during teardown after an invalid observation;
later successful queries can mark disposal certain. This is a useful independent
behavior boundary before the complete worker/scheduler replacement in k44.

## Done when
- An unknown final observation latches its first failed call and observation for
  the run. No subsequent call submits another local process-table query.
- Later workload calls stay unreached. All acquired resources' reverse teardown
  commands still execute; local observations remain unavailable and disposal
  certainty fails. Valid live observations remain distinct from inspector failure.
- Real-controller fault controls demonstrate failure at acquisition, workload
  and teardown, marker-producing disposal, no query retry, independent exact-PID
  reaping audits, fresh-run isolation and healthy behavior. Retain failed samples
  and frozen self-check receipts in new directories; update the foundation report.
- Insert a specific review-impl sticky-inspection-failure before k44 as the last
  producer act. Name this handle in **Reviews:**; no empty integration.

## Notes
This slice does not deliver the worker or remove synchronous helper waits.
All remaining k42/ancestor requirements stay live in k44 without narrowing.

## Decisions (running log)
- Use an explicitly passed, execution-owned inspection state. Latch only an
  unknown observation, retaining its first call and raw observation. A valid live
  table remains a termination failure, not an inspector failure. No global state
  or public declaration API change is needed.
- Suppressed local inspection must not prevent independent reverse safety
  commands, TERM/KILL or bounded leader reaping. Their unavailable observations
  keep disposal certainty failed. A fresh execution resets the latch.
- Frozen prior-controller replays retain five and then seven failures (70 other
  controls pass in each). The final self-check passes 116 tests, Ruff lint/format
  and strict Pyright with unchanged inputs. New evidence is under k43-inputs-01,
  k43-inputs-02, k43-previous-01, k43-previous-02 and k43-final-01.
- k43-audit-01 verifies per-test comparisons and all indexed artifacts without
  changing prior evidence. All 362 recorded real PIDs are absent at the later
  observation; 94 resource markers and 122 argv-derived scratch directories are
  accounted for and absent after owned cleanup. This does not prove escaped-child
  absence, production scheduling or privilege behavior.
- The scheduled review will contest latch completeness, preserved safety actions,
  failure provenance and the evidence limits before k44 builds on this boundary.
  No competing in-session reviewer is needed. The complete worker/scheduler and
  final full-subsystem review remain live, unchanged, on k44 and its ancestors.
