# Review of declared bootstrap bounds (complete-worker-scheduler-k105)

This review covers the k101 producer, commit `8dc220bf`. It is inspection only. It ran no test, build, lint, format or mutation command. It ran read-only hash, byte-comparison and JUnit-parsing commands. It changed no prior evidence.

## Outcome

The review found no defect in the bounded k101 safety behaviour. Plan and direct Worker validate before any side effect. Failed bootstrap uses the selected bounds. Signals stay PID-only before readiness. Healthy and request-failure close are unchanged.

The review found five test-strength and receipt-clarity findings. Each is low severity. No finding changes the claim that the six frozen mutations fail their intended cases.

## Coverage disclosure

The code graph predates commit `8dc220bf`. The review read the committed source and the retained evidence directly. Every cited line number refers to `HEAD`.

## Findings

### F1. The `overrun` receipts do not separate expiry from lateness

Source: `worker_capture.py:357` (readiness loop), `worker_capture.py:383-385` and `worker_capture.py:502-506` (`readiness_overrun`), `worker_capture.py:452` and `worker_capture.py:476` (shutdown `overrun`).

The readiness loop exits only when `time.monotonic() >= deadline`. The failure handler then samples `readiness_finished`. That sample is later than the deadline. `readiness_overrun` is therefore true for every ordinary readiness timeout, even when no observation was delayed.

The shutdown loop has the same shape. It exits only at the fixed deadline. The code then closes channels and samples `shutdown_finished`. Every residual-ownership outcome therefore records `overrun` true.

A reader cannot tell "the deadline expired on schedule" from "the observation returned late". The specification asks receipts to record "overruns". The report claim that a delayed observation "records failed timing" is true, but the same record also appears without a delay.

No test reads `readiness_overrun`, `readiness_outcome`, `startup_deadline_monotonic` or `readiness_finished_monotonic`. The `deferred-reap` case does not assert `overrun`.

### F2. The late-readiness branch has no control

Source: `worker_capture.py:376-378`.

After a valid identity frame, the code rejects readiness when `readiness_finished >= deadline`. This branch guards a `select` or `read` that returns after the deadline. It is the only defence against delayed observation of a valid frame. It decides the equality case.

The `ready` mode delivers readiness after 0.02 seconds against a 0.15-second bound. No case delivers a valid frame at or after the deadline. No mutation removes or weakens this branch. The specific doubt "readiness is late" is therefore unproven for this branch.

### F3. The `overrun` field is asserted on one side only

Source: `test_bootstrap.py:117-119` and `worker_capture.py:476-486`.

The only `overrun` assertion is `is True` in the injected `overrun` mode. The `term` and `kill` modes never assert that `overrun` is false. The outcome strings `reaped`, `residual_ownership` and `not_created` are never asserted either.

The frozen mutation hides the overrun (`overrun = False`) and fails one case. A mutation that sets `overrun = True` always would survive. A mutation that swaps the outcome labels would survive.

### F4. Timing windows are wider than the declared bounds

Source: `test_bootstrap.py:113`, `test_bootstrap.py:123`.

The test accepts a KILL at any time from 0.065 to 0.16 seconds after shutdown start. The declared TERM grace is 0.07 seconds. A KILL sent at 0.15 seconds passes.

The test accepts a failed startup of up to 0.75 seconds against a 0.15-second readiness bound. A readiness allowance of 0.5 seconds passes.

The receipt `term_deadline_monotonic` is the value `self.kill_at` (`worker_capture.py:438-442`). The assertion `phase["term_deadline_monotonic"] == approx(+0.07)` therefore tests the schedule against itself. Only the loose signal-time window tests the real signal time.

No assertion compares `startup_deadline_monotonic` with `startup_started + 0.15`. The frozen `readiness` mutation uses 1.0 seconds, which is outside the 0.75-second window. A smaller wrong value would survive.

### F5. The earlier-cutoff and signal-history claims have no bootstrap evidence

Source: `worker_capture.py:348`, `worker_capture.py:414-428`, `worker_capture.py:794`, `worker_capture.py:915`.

The report says failed bootstrap preserves "an earlier owner cutoff". Before readiness the owner deadline is infinity (`worker_capture.py:348`). The other writers (`worker_capture.py:794`, `worker_capture.py:915`) run on request paths that need a started worker. `failed_bootstrap` requires `not started`. The `min(...)` at line 424 and the signal-history loop at lines 417-422 cannot see a finite deadline or a prior signal in a failed bootstrap.

These behaviours matter for worker-failure and orderly close. They are not demonstrated by this slice. The six mutations do not touch them. This is a scope gap for k104, not a bootstrap acceptance.

## Observations kept out of the findings

- `failed_bootstrap` depends on `self.failure` being set by `except (OSError, ValueError)` at `worker_capture.py:383`. An exception of another type would leave `failure` unset. The controller's `finally: worker.close()` (`controller.py:748-749`) would then use the healthy schedule. The review did not find a reachable exception of that type. This is plausible and unproven.
- The `kill` and `deferred-reap` modes rely on a ten-millisecond-scale loop. The slack is wide, but the assertions tolerate it (see F4).
- A delayed `waitpid` that returns a real reap after expiry is untested. The code would record `reaped` with `overrun` true. That outcome matches the contract.
- Admission rejection, fork failure (`not_created`) and an invalid identity frame have no bootstrap-bound control. The 24 invalid-declaration cases cover only the bound values.
- The frozen input directories hold `__pycache__` and `.pytest_cache` files. Git does not track them (0 tracked paths). They do not affect any hash in the verdicts.

## Verification performed

| Claim | Result |
| --- | --- |
| Plan validates before evidence, hashing, pipes or fork | Holds by source: `controller.py:88-90` runs first, then platform and run id. |
| Direct Worker validates before side effects | Holds: `worker_capture.py:315-317` precedes the first clock read, pipe and fork. |
| Selected bounds drive readiness and failed-bootstrap TERM and KILL/reap | Holds by source at lines 319, 427 and 432. |
| Signals stay PID-only before readiness | Holds: `_signal` uses `os.kill` while `owner.ready` is false. The test asserts scope `pid`. |
| Healthy and request-failure close unchanged | Holds: the diff against `879a801` adds only `failed_bootstrap` conditionals. The `else` branches keep `4`, `2` and the half-allowance cap. |
| Six mutations differ from `inputs-green2` by one line | Holds by `diff` of every `*.py` file for each mutant. |
| Failing cases match the report | Holds from JUnit: 24, 1, 4, 2, 4 and 1 cases. The names match the intended sites. |
| Artifact hashes | Holds: 5,256 digests across 11 conditions match their files. No subject drift in any verdict. |
| Final inputs equal the committed tree | Holds: the `*.py`, `pyproject.toml`, `uv.lock` and `Taskfile.yml` bytes equal `inputs-final`. |
| Case counts | Holds: 30 RED failures, 30 GREEN passes, 622 full passes. `green1` failed one case, as reported. |
| Link-aware preservation audit | The summary in `condition-audit.json` and the report agree. The review did not recompute the 123,249-path audit. |

## Scope against k104

These findings concern the bootstrap slice only. Worker-failure and orderly-close phase declarations, the shared scheduler, overlap with controller teardown and the final bounded-inspection-k28 review stay in complete-worker-scheduler-k104. F5 hands the earlier-cutoff and signal-history evidence to that leaf. k28 and k17 remain open. Ten-clone acceptance remains 0/10, unavailable.

## Routing

All five findings are actionable test or receipt changes. F1 may also need a small runtime change to separate expiry from lateness. The review cut one integrate-review-impl leaf before k104.
