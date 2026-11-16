# storage-client-k175

**Integrates:** storage-client-k174

## Goal

Triage the controller storage client review against the producer's contract,
apply findings that meet the acceptance and material-risk threshold, and leave
a verified client for `storage-cutover-k166`.

## Context

Read the findings from the commit for `storage-client-k174`, then rederive
them against the current source. The reviewed producer is
`storage-client-k165`. The review also records control seams for the later
cutover/publication slices; retain their existing ownership.

## Done when

- Every review finding has a source-backed classification under the integration
  procedure, with the reasoning recorded in this task.
- Accepted repairs have focused regression evidence and the repository's
  appropriate required checks. The full suite passes with the expected markers
  for the controls that still belong to later slices.
- The client is ready for cutover without widening this repair into the
  remaining execution/publication work.

## Notes

This is a sibling repair under `evidence-writer-k162` because it shares that
node's contract and has a finite handoff to the next consumer.

## Decisions (running log)

- **F1, a lost writer can still receive KILL: real issue, repaired.** A
  reproduction showed the defect. The test reaped a pending writer outside
  the client. The next turn failed readiness with `writer_lost`, and the
  same turn sent one SIGKILL to the lost PID. A `ChildProcessError` from
  the exact-PID wait means that another party reaped the writer. The PID
  can then belong to an unrelated process. The spec allows KILL only while
  the writer is unreaped, so the source was wrong and the spec stays.
- **The guard is the residual flag in `_kill`.** A residual writer is
  either lost or past its reap deadline. In both cases the client must not
  signal again. The guard covers the automatic KILL in `advance` and the
  public `kill()`. The lost path keeps `pid`, `reap_error` and the
  `writer_lost` failure, so the controller can still report the residual.
- **Two controls cover the pending and the ready states.** Each control
  reaps the writer outside the client and records every `os.kill` call.
  Each then calls `kill()` and runs three more turns. Each asserts no
  signal, the failure state, the residual flag, released pipes, abandoned
  counts and a turn below the 0.1 s limit. Both controls failed without
  the guard, with the message that the client signalled a PID it no
  longer owns.
- **The fixture keeps its reap assertion.** These two controls reap the
  writer themselves, so each removes its client from the fixture list.
  The fixture still fails on any other unreaped writer.
- **The other review sections need no change.** The contract checks and
  the control handoff table record no defect. The forward stop, the intent
  gate and publication stay with `storage-cutover-k166` and
  `storage-publication-k168`. The review saw no separate control for a
  late `ok` readiness acknowledgement. That remark is not a finding, and
  this leaf adds no control for it.
- **Verification.** The diagnostic suite gives 1056 passed and 9 expected
  failures. The nine storage controls stay strict expected failures for
  the later slices. Ruff check, ruff format and project pyright pass.
