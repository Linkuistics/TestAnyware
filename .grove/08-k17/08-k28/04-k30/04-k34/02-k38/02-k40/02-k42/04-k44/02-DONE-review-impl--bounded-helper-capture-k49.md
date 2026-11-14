# bounded-helper-capture-k49

**Reviews:** bounded-helper-capture-k47

## Goal
Audit the actual post-creation helper capture/reap path and its integration
before worker-scheduler-k48 changes the ownership boundary.

## Context
Read the producer's committed diff, the parent contracts and
`docs/specs/diagnostic-process-inspection.md`. The implemented boundary is
`experiments/exec-reliability/inspection.py` through the actual group query in
`experiments/exec-reliability/controller.py`, with controls in both test modules.
The measured report is `docs/experiments/exec-reliability-diagnostic-foundation.md`;
retained conditions are `experiments/exec-reliability/evidence/k47-check-01`,
`experiments/exec-reliability/evidence/k47-final-01` and the independent
`experiments/exec-reliability/evidence/k47-audit-01`.

## Done when
- Inspect all helper ownership, cleanup and exception paths, including Popen
  construction, destructor behavior, waitpid, pipe closure, helper KILL and
  retained residual owners. Test whether identity release always closes signal
  authority and whether unexpected failures could lose ownership or wait forever.
- Contest the fixed capture/reserved-reap deadlines, combined output cap,
  per-turn work, stderr/EOF handling and late-creation behavior against the
  actual executable path. Distinguish the bounded post-creation guarantee from
  synchronous creation and controller-local waiting still assigned to k48.
- Check sticky failure and safety continuation, group KILL ordering, strict
  table validation, sole final observation and nonblocking workload reap for
  regressions. Evaluate the real controls and the separately labelled simulated
  unreapability fixture; do not credit them as privileged denial/setuid-ps tests.
- Audit each condition's subjects, bounds, raw receipts and exact resource
  reconciliation. Preserve the initially failed lint condition. Check report
  claims against actual receipts and ensure the full parent worker/scheduler,
  concurrency and final bounded-inspection-k30 review obligations remain live.
- Record actionable findings with committed file/line evidence. Cut integration
  before worker-scheduler-k48 only if findings warrant it; no empty integration.

## Notes
No in-session reviewer ran. The launch supplied no current provider evidence;
the retirement procedure therefore leaves the Creator declaration absent.

## Decisions (running log)

Review of producer k47, commit 2d25827 (change yslowlwkmylz). Inspection only: no test, lint, build or format command ran. Lines are in `experiments/exec-reliability/inspection.py` at that commit.

### Findings

- Finding 1 (medium, correctness): an exception from stream closure skips the reap loop.
  Lines 255-257 call `stream.close()` without a guard, inside the `finally` block. An `OSError` there leaves lines 258-269 unrun.
  The child is already killed but never receives a reap attempt, and `finished_monotonic` stays 0.0. The exception then reaches `_group_snapshot` (`controller.py:312`), which records no `observation["helper"]` because line 245 of the call has not run. The raw receipt is lost.
  The owner stays in `_helper_owners` (line 205), so ownership is not lost, but the cleanup contract in the task is not met. No test injects a close failure.
- Finding 2 (medium, bound): the receipt can finish after its deadline, and no final reap follows the last sleep.
  Line 258 tests the deadline before the first reap. Line 261 sleeps up to 5 ms and the loop then exits without another `_reap_helper` call.
  A helper that was killed and exits during the last sleep is reported as unreaped with a residual remedy (lines 262-268). Nothing sweeps `_helper_owners` later, so the zombie lives until process exit.
  Receipt evidence: `experiments/exec-reliability/evidence/k47-final-01/runs/helper-simulated-reap-029c89c607084d44b8e12d6fa84574d1.json` has `finished_monotonic` 263046.779055791 and `deadline_monotonic` 263046.778218625, so it ended 0.84 ms late.
  The report claim "finished within the deadline" is proven only for the reaped controls (`test_review_helper_capture_bounds_real_pipes_and_reaps`). It does not hold on the unreaped path.
- Finding 3 (medium, coverage and behaviour): a late Popen return is untested, and the helper is then never signalled.
  Line 248 sends KILL only while `time.monotonic() < deadline`. Popen is synchronous (line 197) with no time bound.
  If it returns after the deadline, the capture loop is skipped, the KILL is skipped and the pipes are closed (lines 255-257). A live helper, for example a hung `ps`, then keeps running with a strong owner and no signal.
  The PID is still pinned by the unreaped child, so a KILL would be safe. The code comment says only that a late return "cannot extend the deadline" (lines 173-174).
  No test patches Popen to return late. The task already assigns synchronous creation to k48, so the fix may be a documented residual plus a control. The leaf must say which.
- Finding 4 (low, residual accuracy): a wait error is reported as a retained owner with a stale PID.
  `_reap_helper` (lines 158-161) stores `reap_errno` and returns early on every later call. For `ECHILD` the child is gone, yet line 167 never removes the owner and the remedy text claims a retained live child.
  Admission rejects ignored SIGCHLD, so this is a narrow path. No test covers it.
- Finding 5 (low, dead code): `except subprocess.TimeoutExpired` at `controller.py:310` cannot fire.
  `_group_snapshot` no longer calls `subprocess.run`. The branch also writes `stdout=error.stdout or b""`, which overwrites a capture if it ever ran.
- Finding 6 (low, tests): helper failure controls in `test_controller.py` replace `controller.capture_helper` with adapters that return `HelperCapture(... reaped=True)`.
  Those adapters check the controller reaction only. Real pipe, cap and reap behaviour is covered separately in `test_inspection.py`. The two sets are consistent, but the adapter receipts must not be cited as helper evidence.
  The simulated unreapability control (`helper-simulated-reap-*.json`) is labelled "waitpid always returns (0, 0); real child and KILL" and the report does not claim privileged denial. The report states this limit correctly.

### Checked and sound

- Receipts match the report: the five real helper receipts in `k47-final-01/runs` give elapsed 0.015942, 0.014994, 0.456604, 0.027784 and 0.020463 s. Both overflow cases hold 262144 bytes. The exit codes are 0, 0, -9, -9, -9. The 500 ms deadline puts the capture cutoff at 450 ms, as claimed.
- Hang case: KILL at 263046.5099805 follows the capture cutoff 263046.509491916 and the reap ends 6.1 ms later, inside the 50 ms reserve.
- Combined cap: the `available` arithmetic (line 226) and the 16384-byte read size (line 228) give exactly 256 KiB across both streams. Each turn reads once per ready stream.
- Identity release: after `waitpid` succeeds line 167 drops the owner, and the KILL guard at line 248 requires `reaped` false and `reap_errno` None. A wait error therefore closes signal authority.
- Condition integrity: `k47-final-01` passes 130 tests with lint, format and strict types. `k47-check-01` keeps its failed lint result (E501 at `inspection.py:264` and an import-order error), and `k47-audit-01` reconciles both.
- Parent obligations stay live: k48 keeps the worker/scheduler, synchronous creation, shutdown and concurrency work, and k30 keeps the final bounded-inspection review.

### Outcome

Findings 1-3 warrant integration. An integrate-review-impl leaf `bounded-helper-capture-k50` was cut before worker-scheduler-k48. Findings 4-6 go to the same leaf as optional fixes with a stated reason if rejected.
