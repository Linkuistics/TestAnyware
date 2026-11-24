> Historical report. Paths and commands refer to the [archived checkout](../../README.md).

# Incremental result framing — k75

The interim per-query receiver rejects malformed result framing as soon as the
bytes establish the defect. It no longer waits for the control channel to close
before rejecting an oversized declaration, truncated result EOF or trailing
bytes. This completes the framing slice of `complete-worker-scheduler-k75`.
The complete persistent-worker and scheduler charter continues in
`complete-worker-scheduler-k78`; k28 and k17 remain open.

## Behavior

`worker_capture.py` first reads only the four-byte network-order length header.
It checks the 1MiB declaration cap immediately, then reads at most the declared
remainder plus one detection byte, still within the existing 32KiB result-read
budget per turn. The detection byte is never appended to the retained frame.
Control reads retain their independent cumulative cap and cleanup-only parser;
the combined normal-turn read budget remains at most 64KiB.

An incomplete header or body at result EOF fails immediately even with an open
control pipe. Complete terminal data is still decoded only after both channels
end cleanly. Fragmentation is allowed. Query and shutdown deadlines are not
renewed, and late terminal data cannot repair failure or acknowledge helper reap.

Early errors bypass the old post-EOF decoder, so cleanup also preserves creation
conflict when a failed query has both a fork-error publication and partial
control/terminal bytes. Such contradictory data cannot grant no-child authority.
The existing control-tail and short-frame regression cases exercise this path.

## Frozen conditions

All five retained conditions have matching before/after subject hashes, bounded
outer checker receipts, raw streams, JUnit and artifact digests. Every checker
was reaped; none timed out. Locked tooling runs through `mise exec -- task`.

| Condition | Cases | Pytest result | Static checks |
| --- | ---: | --- | --- |
| [red-01](red-01/verdict.json) | 6 | Five expected framing failures, one pass | Not run |
| [green-controls-01](green-controls-01/verdict.json) | 6 | All pass | Not run |
| [red-02](red-02/verdict.json) | 7 | Five expected framing failures, two passes | Not run |
| [green-full-01](green-full-01/verdict.json) | 357 | Five failures, 352 passes | Lint/format pass; strict types fail |
| [green-full-02](green-full-02/verdict.json) | 357 | All pass | Lint, format and strict types pass |

RED runs use the original receiver. The five defects are oversized header,
short header EOF, short body EOF, trailing bytes and cap overflow. These controls
fork real workers and use real nonblocking pipes; a test worker publishes the
malformed bytes and holds its control channel open for two seconds. No helper
is forked by these five fixtures. Parent-side `os.read` instrumentation records
actual requested/returned byte counts. Final GREEN reads exactly 4, 2, 5, 7 and
1,048,581 result bytes respectively, including the overflow detection byte.
Each first TERM occurs before the asserted 450ms cutoff within the original
900ms total allowance (observed 2.8–4.7ms); each worker is reaped. Unknown helper
creation remains a failed, conservative result despite fixture knowledge.

The final RED strengthens overflow to a full cap plus 64KiB and adds the second
positive control. Fragmented valid delivery uses a real `/usr/bin/true` helper
and obtains matching helper/worker reap evidence. The open-control control
delivers a complete real helper result but holds the channels open: capture
fails at its original deadline, never publishes terminal times, and retains the
helper obligation. Its test-only ledger removal is fixture teardown, not runtime
proof of helper reap; the independent auditor separately checks PID absence.

The first full run exposed three creation-accounting failures fixed by the
cleanup conflict guard above. Its two other failures were old assertions that
required an all-channel receipt timestamp for oversized/partial frames; early
rejection now legitimately precedes that timestamp. Those tests retain their
failure and ownership checks. The optional integer type error is corrected by
typing the existing `Struct.unpack` result. No failed sample was replaced.
Final pytest took 44.15 seconds within its 60-second budget.

Frozen source mapping is verified by [audit.py](audit.py): `inputs-red` contains
the original receiver and initial tests/runner; `inputs-green` the initial fix;
`inputs-red-final` the strengthened tests/runner (red-02 uses the original
receiver); `inputs-green-final` the first full run; and
`inputs-green-repaired` the final sources. The initial runner frozen in
`inputs-red` is also the runner used for green-controls-01.

An earlier direct pytest exploration is retained as [red.xml](red.xml). Its
temporary fixture receipts were already gone when recovery was attempted.
It has no frozen outer runner or retained fixture census and is excluded from
the five-condition comparison and exact-PID audit. No receipts were fabricated.

## Independent evidence and resource audit

`mise exec -- task diagnostic:resource-audit
AUDITOR=experiments/exec-reliability/evidence/result-framing-k75/audit.py`
passes lint/format checks and runs the frozen auditor. Its
[verdict](audit-01/verdict.json) records:

- 2,614 declared subjects with equal before/after hashes, including the auditor,
  condition artifacts, snapshots, live diagnostic sources and selected tools.
- All 78,030 earlier evidence files match [prior-evidence.json](prior-evidence.json),
  checked before and after the audit.
- Every named testcase reconciled with final GREEN: ten expected RED failures
  and the five first-full-run failures now pass; every other observed case also
  passes. The five final frame receipts and read witnesses are independently
  checked against their exact byte bounds and original deadlines.
- All 1,397 retained positive exact PID selectors absent in a separate `/bin/ps`
  observation (exit 1, empty stdout and stderr). Live, absent, mixed and invalid
  selector controls first establish oracle results `[false, true, false, false]`.
  Nonpositive and explicitly simulated identities are excluded.
- 670 recorded/sibling scratch directories checked. Two remaining mock markers
  were removed only after exact fixture PID absence and after matching each
  retained acquisition plan, run identity and exact bytes `owned`. The audit
  retains their hashes and absent-after observations.

Later PID absence establishes absence at audit time only. It does not establish
historical reap, authenticate a production helper, or resolve unpublished
creation. Failed observations and residual remedies remain failed. This audit
does not include the exploratory invocation's lost fixture receipts.

## Scope and next review

Graph coverage reported changed metadata for the edited runtime/test paths;
material conclusions therefore use direct source and retained executions rather
than completeness claims from the graph. Python >=3.12 remains the manifest
contract; the existing binary format follows the
[Python 3.12 Struct documentation](https://docs.python.org/3.12/library/struct.html#struct.Struct).
There are no new dependencies or architecture decisions requiring an ADR change.

A separate review of k75 precedes k78 and must challenge incremental framing,
early-error creation accounting, cleanup identity retention and evidence scope.
k78 preserves the entire prior charter and decision history, including
pre-resource persistent startup, full protocol identities/epochs, the immutable
shared scheduler, every outstanding real control and the final
`Reviews: bounded-inspection-k28` review before k21. There were no guests,
installed-app changes, privilege experiments or unrelated process signals.
Stalled storage remains k18; ten-clone acceptance remains **0/10, unavailable**.
