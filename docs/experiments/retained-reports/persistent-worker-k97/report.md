> Historical report. Paths and commands refer to the [archived checkout](../../README.md).

# Review of the k94 persistent inspection worker (k97)

## Result

The review found no actionable contract violation in the k94 slice. It cut no integrate step.
The review was inspection-only. It ran no test, build, lint or format command.
It executed no counterexample or mutation, so it froze no new subject.

## Scope and coverage limits

The review read `controller.py`, `inspection.py` and `worker_capture.py` in full.
It read the k94 diff, the k94 report, the retained pytest XML and the audit-3 verdict.
It read the tests that name the safety-adapter path, the persistent worker death path and the late helper identity path.
It did not read every test body in the three test files.
It used direct file reads. It made no claim from the code graph.

## Verified by recomputation

- The SHA-256 of `controller.py`, `inspection.py`, `worker_capture.py` and `Taskfile.yml` on disk equals `subjects_after` in `audit-3/verdict.json`. The audited code is the reviewed code.
- The k94 commit modifies seven paths: the root Taskfile and the three runtime sources with their three test files. It modifies and deletes no path under any earlier evidence tree.
- Every `os.waitpid` call names one exact PID. No call waits on `-1`, so no wait can reap another owner's child.

## Answers to the five questions

1. Startup. `Worker.start()` checks admission twice, once before and once after pipe creation. It blocks all signals across `os.fork()`. The child resets state and closes every other descriptor. The parent accepts readiness only for the exact identity `{session, pid, pgid=pid, sid=pid}` before its one-second deadline. `execute()` acquires no resource unless `worker.start()` returns true.
2. Framing. One command line is the only input. The worker rejects queued input, a wrong session and a bad deadline. The controller reads a four-byte header, a bounded body and one extra byte to detect a trailing frame. It rejects a second completion, a control line after completion, a wrong token and a late identity. Each request owns its own buffers, so one response cannot contaminate another. The controller cannot order the completion line against the result frame because they use separate pipes. It accepts either order and needs both before the deadline.
3. Failure paths. `Worker.start()` returns early once the worker started, closed or failed, so it never retries or replaces. `close()` is idempotent. The worker stores a helper identity as an obligation. Only a matching helper reap receipt retires it. Worker reap does not.
4. Controller. `worker.close()` runs in a `finally` that wraps every resource cleanup, disposal and inspection. The worker signals only its own group. It never signals a workload group. The workload signal code at `controller.py:465` is separate from the worker code.
5. Evidence. The audit-3 identities match the source. The report states the full-1 and full-4 failures as unresolved and does not claim a repair.

## Reconciliation of the retained failures

I read the failing test names and messages from the retained pytest XML.

| Run | Failures | Observed message | Reading |
| --- | --- | --- | --- |
| full-1 | 6 | Four `TypeError` from a test double without a `worker` keyword. | Test-side signature defect. It skipped worker closure, which left an owner in `_owners`. |
| full-1 | | One `assert not {pid: _Owner(...)}` in `test_review_worker_rejects_fork_before_creation`. | Consequence of the skipped closure. |
| full-1 | | One `assert (False)` in `test_late_created_identity_rejects_invalid_fields[partial]`. | Unresolved timing observation. |
| full-2 | 1 | `'native_threads' == 'python_threads'`. | Test expected the wrong reason string. |
| full-4 | 5 | One `TypeError` and four `KeyError: 'after_term'`. | Unresolved timing observations. |
| full-5 | 0 | None. | Unchanged final suite, no concurrent checker. |

The four `after_term` failures come from a fixture in `test_worker_capture.py` near line 1782.
The fixture child must write a witness file after it receives `SIGTERM`.
The parent sends `SIGKILL` after `reserve / 4`, which is at most 12.5 ms (`worker_capture.py:909`).
A loaded host could let `SIGKILL` arrive before the witness write.

This is a plausible mechanism. The review did not establish it and makes no causation claim.
The k94 report already states that full-5 does not prove a correction.

## Observations, not findings

- **O1, coverage gap.** `_safe_call` catches only `OSError` (`controller.py:624`).
  The k94 control `test_persistent_worker_closes_when_safety_adapter_raises` replaces `_safe_call` as a whole.
  It proves that the worker closes. It does not prove that the cleanup of later resources still runs.
  The baseline loop had the same behaviour, so k94 did not regress it.
  The review found no path where `capture_helper` raises a non-`OSError`, non-`ValueError` exception.
  Recommendation: k96 should decide whether to widen `_safe_call`.
- **O2, completion order.** See question 2. The ordering is unobservable across two pipes and fail-closed on trailing bytes.
- **O3, TERM grace.** The spec describes a grace of up to two seconds. The request-failure cleanup allows `reserve / 4`.
  The worker uses `SIG_DFL` for `SIGTERM`, so a grace has no effect in production.
  The shared scheduler remains k96 work.
- **O4, blocking close.** `Worker.close()` polls with `time.sleep(0.001)` for up to four seconds.
  It runs after safety work, so it does not delay safety. The k94 report discloses this private wait. It is k96 work.
- **O5, latent `Infinity`.** `write_json` does not set `allow_nan=False`. Evidence stays finite today because `close()` always sets a finite deadline before `evidence()` runs.

## Separation

- Actionable k94 contract violations: none.
- Owned by k96: shared earliest-deadline scheduling, overlapping shutdown and teardown, the private polling waits and the synchronous fork.
- Unestablished timing mechanisms: the full-1 partial-identity failure and the full-4 `after_term` failures.
