# Review of the k96 persistent request identity envelope (k99)

## Result

The review found no runtime defect in the bounded k96 contract.
It found two low-severity test-strength gaps. Both are actionable, so the review cut one integrate step.
The review was inspection-only. It ran no test, build, lint or format command.
It executed no counterexample or mutation, so it froze no new subject.

The frozen k96 records stay as they were.

## Scope and coverage limits

The review read the k96 diff, `report.md` of k96 and the k96 verdict files.
It read `worker_capture.py`, `controller.py` and the identity paths of `inspection.py`.
It read the damaged-envelope test, the sequential-reuse test, the controller epoch test and the retained receipts.
It read `docs/specs/diagnostic-process-inspection.md`. The k96 commit does not change that spec.

It did not read every test body in the three test files.
It did not digest dependency implementations beyond the named subjects. The k96 report discloses the same limit.
It used direct file reads. It made no claim from the code graph.

The privileged and ten-clone controls were unavailable. The review treats them as not executed.

## Identity checks and acceptance order

1. `_request_identity` accepts a dict with exactly nine keys. String fields are non-blank, at most 256 characters and free of NUL.
2. The integer fields must have exactly type `int`, so a boolean fails. The PIDs must be at least 1 and the epoch at least 0.
3. The deadline must be an `int` or a `float`, finite and above 0.
4. `_check_identity` compares the normalised value with the expected dict. A missing, extra or mistyped key fails.
5. The worker (`_service`) tests the command keys, then the session, its own PID, the request token and the deadline.
6. The worker then tests the run ID against the first request and rejects a reused token. Any failure exits the worker with status 70.
7. The controller side (`consume_control`) tests the identity before it reads the readiness, intent, created, fork-failure or completion message.
8. `_decode` tests the identity of the terminal frame before the token, the field set and the receipt body.

The worker tests the workload PID, PGID, epoch and phase only for structure. It echoes them. The parent is the only party that compares them.

## Helper-ledger consequences

- `_helper_obligations.pop(token, None)` runs only after the terminal receipt passes every check. This includes the fork-failure and created-identity cross-checks.
- An identity mismatch in the terminal frame fails the receipt before the pop. The obligation stays.
- A mismatch in a ready, intent, created or fork-failure message in the cleanup path sets `creation_conflict`. The receipt then reports an unknown helper creation and a residual remedy. It never claims "no child".
- Extra helper identities use the key `token:pid`. Nothing pops them. They arise only after a conflict, so they never complete.
- The request failure is sticky. `Worker.start()` returns false after a failure, and `capture_helper` reports the worker as unavailable.

## Controller paths

- The only persistent query is `_group_snapshot` at `controller.py:265`. It passes `RequestScope(pid, pid, signal_epoch, "final")`.
- The PGID equals the PID because the workload starts with `start_new_session=True`.
- The termination path passes `signal_epoch=len(events)`. A denied signal still appends an event, so the epoch counts it.
- The normal-exit path passes the default epoch 0. The controller attempts no signal on that path, so 0 matches the run.
- `Worker(run_id=plan.run_id)` binds the worker to the run. `_validate` forces the run ID to 32 lowercase hexadecimal characters. The 256-character and 4096-byte caps are therefore safe.
- No caller uses a worker without a scope. The default scope serves the one-shot compatibility path only.

## Findings

### F1. No test covers worker-side request replay or run pinning (low, test strength)

The worker rejects a reused token and a different run ID. The checks are `used_tokens` and the `run_id` pin in `_service`.
No test sends a second command to one worker with a reused token or a different run ID.
The sequential-reuse test uses one constant run ID and fresh tokens. Each damaged-envelope case uses one request per worker.
The leaf question on request reuse therefore has source evidence only.

A mutation that removes either check would pass all 82 focused cases.
The integrate step should add a hand-built second command through the worker command descriptor.

### F2. Damaged ready, intent, created and fork-failure cases assert too little (low, test strength)

The 77 damaged-envelope cases assert that the request failed and the worker closed.
Only the result and completion stages also assert that the obligation stays and the helper identity exists.
The retained receipts for the other 44 cases show the right state.
The helper identity is `None`, `unknown_helper_creation` is true, `creation_conflict` is true and a residual remedy exists.

No assertion guards that state. A regression could turn a rejected frame into a "no child" claim.
That regression would pass these cases.
The integrate step should assert the conservative state for those stages.

## Observations, not findings

- The worker run ID is trust-on-first-use. The startup readiness frame carries no run ID, because startup precedes any request.
- The epoch counts a `deadline_exhausted` event. No signal was sent for that event, but no persistent query can follow it. The k98 cutoff work settles the wording.
- After a damaged earlier frame, the cleanup path drops later valid created frames. The receipt then reports an unknown creation. The pre-k96 handling of a malformed intent did the same.
- The test buffers the damaged child frames until the real helper completes and the harness reaps it. The k96 report discloses this. A damaged frame therefore never races a live helper. Older tests cover the live-helper failure path.
- The shared scheduling, cutoff closure of epochs and overlapping teardown belong to k98.

## Reconciliation of the retained records

| Run | Result | Reading |
| --- | --- | --- |
| red-1 | 67 of 67 fail | The tests fail before the envelope exists. This matches the k96 report. |
| green-1 to green-4, green-controls-5 | Failures | The k96 report attributes these to fixture and type faults. The review makes no further causation claim. |
| green-controls-6 | 82 pass | Matches the report. |
| green-full-6 | 579 pass, ruff, format and pyright clean | Every check returned 0, did not time out and was reaped. |
| mutation-no-check-final | 71 fail, 11 pass | The 11 passes are 5 positive controls and 6 command-stage cases. The worker rejects those 6 itself. The mutation does not exercise the worker checks (see F1). |
| mutation-zero-epoch-final | 2 fail | The group-timeout and pid-interrupt controller tests fail. This shows the epoch assertion works. |

## Verified by recomputation

- `subjects_before` equals `subjects_after` in all four final verdicts. Each lists 20 digests.
- The frozen inputs in `green-inputs-6` are byte-identical to the committed sources.
- Each mutation differs from the final source by one edit. One edit makes `_check_identity` return at once. The other sets `signal_epoch=0` at `controller.py:525`.
- `prior-evidence-sha256.json` lists 119,509 files. The same number exist outside the k96 directory, and none is missing.
- A random re-hash of 1,500 files found no mismatch.
- The k96 commit adds files only in its own evidence directory. It changes the root Taskfile, the three runtime sources and two test files. It also renames the k98 leaf.
