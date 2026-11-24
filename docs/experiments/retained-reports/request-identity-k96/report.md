> Historical report. Paths and commands refer to the [archived checkout](../../README.md).

# Persistent request identity evidence (k96)

The persistent worker transport now carries a complete request identity on commands, readiness, intent, helper creation, explicit fork failure, completion and terminal results. The controller supplies its run ID, workload leader PID/PGID, phase and number of attempted termination signals. Matching requests can reuse the same worker. A malformed or mismatched response fails the worker session and cannot use a terminal receipt to retire a helper obligation.

This report covers the narrowed `complete-worker-scheduler-k96` contract. The complete aggregate charter remains in `complete-worker-scheduler-k98`. Shared earliest-deadline scheduling, closure of signal epochs at cutoff, overlapping worker shutdown and safety teardown, and the remaining aggregate controls are unfinished. The standalone one-shot transport is outside this slice. Neither k28 nor k17 is complete.

## Artifact and checks

`worker_capture.py` defines an immutable `RequestScope` and a strict nine-field envelope: `run_id`, `worker_token`, `worker_pid`, `request_token`, `workload_pid`, `workload_pgid`, `signal_epoch`, `phase` and `deadline_monotonic`. Exact field sets and types are checked before equality; booleans cannot pass as integer identities. The service pins the run and its actual worker PID, rejects reused request tokens, and echoes the envelope on each persistent message. The controller passes the actual workload identity and signal-attempt count, including denied attempts. Terminal identity validation precedes receipt decoding and helper-ledger retirement.

The 82 focused cases comprise 77 altered-message cases (seven stages, eleven alterations each), a positive sequential-reuse case, three controller cases and a matching explicit-fork-failure case. The alterations cover all nine fields, a missing identity and a boolean epoch. The seven stages are command, readiness, intent, creation, explicit fork failure, completion and terminal result. Each altered-message test requires an injection marker, so failure to exercise the intended transport cannot count as rejection evidence.

The altered child controls are buffered until the real helper has completed and been reaped. These tests therefore check identity rejection and ledger retention without deliberately stranding helpers. They separately check exact worker-child ownership/reaping and absence of known helper PIDs. Controller cases inspect real run/workload identity and epoch zero for normal exit versus the attempted-signal count after group timeout or PID interruption, and independently check workload, helper and worker absence. Injected `EAGAIN` is a protocol-classification control; it is not evidence of a real kernel or privileged failure.

## Retained results

| Record | Result | Interpretation |
| --- | --- | --- |
| `red-1` | 67/67 focused cases failed | Baseline lacks the new API. This establishes the pre-implementation failure, not behavioral rejection. |
| `green-1` | 59 passed, 11 failed | The command interceptor incorrectly skipped parent commands. The fixture was corrected. |
| `green-2` | 566 passed, 1 failed; seven typing errors | The frozen layout exposed an existing self-check test's shallow-root assumption. The test now uses an isolated checker root; production self-check code is unchanged. Conditional keyword typing and a redundant type check were corrected. |
| `green-3` | 578 passed, 1 failed; three lint errors | The new positive fork-error fixture had not started its worker and lacked the `errno` import. |
| `green-4` | 578 passed, 1 failed; three lint errors | The remaining missing `errno` import broke that fixture. |
| `green-controls-5` | 81 passed, 1 failed | The positive fixture asserted a nonexistent receipt field. Its assertion was corrected to the actual unknown-creation/conflict fields. |
| `green-controls-6` | 82 passed | Final focused controls; checker reaped, no timeout, subjects unchanged. |
| `green-full-6` | 579 passed; lint, format and strict typing passed | Final full suite; all four checkers reaped, no timeout, subjects unchanged. |
| `mutation-no-check-final` | 71 failed, 11 passed | Replacing `_check_identity` with a no-op makes controls fail across all seven message stages. Independent service checks still operate. |
| `mutation-zero-epoch-final` | 2 failed, 80 passed | Reporting epoch zero after signaling breaks exactly the group-timeout and PID-interrupt controller cases. |

All earlier attempts and preliminary mutation records remain unchanged. Preliminary mutations contain fixture failures and are not credited as clean mutation evidence. The final focused cases were compared by case identity with the focused subset of the final full suite; both contain the same 82 passing cases. Per-case results are in each record's `pytest.xml`; raw stdout, stderr, declarations, run receipts and verdicts are alongside it. A later pass does not establish a cause for any unrelated historical timing failure.

## Reproduction and provenance

From the repository root, the reusable task runs the frozen final source tree:

```sh
mise exec -- task diagnostic:request-identity-check INPUTS=evidence/request-identity-k96/green-inputs-6 OUT=evidence/request-identity-k96/reproduction-controls MODE=controls
mise exec -- task diagnostic:request-identity-check INPUTS=evidence/request-identity-k96/green-inputs-6 OUT=evidence/request-identity-k96/reproduction-full MODE=full
```

Use a new output directory for each run. The checker refuses an existing directory. The focused timeout is 60 seconds and the full-suite timeout is 120 seconds; lint, format and typing each have 30 seconds. On timeout the independent checker permits two seconds after TERM and then two seconds after KILL for exact checker-child reaping, retaining a residual remedy if needed. These are test-harness bounds, not claims about the unfinished production scheduler.

`run_condition.py` records SHA-256 digests before and after for all frozen top-level Python sources, manifest, lockfile and Taskfile, the root Taskfile, the runner, Python executable, `/bin/sh`, `/bin/sleep`, `/usr/bin/false`, `/usr/bin/true` and `/bin/ps`. It records output artifact digests in each verdict. Final frozen Python/manifests/Taskfile bytes were also compared with the live sources and matched. Dependency implementations beyond the named subjects are not individually digested.

`prior-evidence-sha256.json` records the 119,509 evidence files that predate this new evidence directory. `prior-evidence-verdict.json` reports a path-by-path SHA-256 comparison: 119,509 checked, zero changed or missing. This is preservation evidence for that inventory, not an assertion of aggregate runtime correctness.

No guest, installed application, unrelated process or privileged test was exercised. Ten-clone acceptance and privileged `ps`/UID controls remain unavailable under the existing constraints. The independent review must assess the committed artifact and its bounded claims before the aggregate continuation relies on them.
