# complete-worker-scheduler-k117

**Reviews:** complete-worker-scheduler-k114

## Goal

Independently challenge the incremental workload termination signal/exit seam
against k114's bounded contract and the applicable termination timing spec.

## Context

Read the k114 producer and
experiments/exec-reliability/evidence/termination-turns-k114/report.md,
controller.py `_Termination`/`_terminate`, and the direct/real controls in
test_controller.py. Read docs/specs/diagnostic-process-inspection.md and the
termination-cutoff-k90 report for the original admission/reserve rules.
Read incremental-review-k115 for the worker-turn boundary and the limits of
passing evidence. k116 retains the entire aggregate charter/history; this
review must not certify shared scheduling, teardown overlap or guest acceptance.

## Done when

Trace constructor/advance/compatibility-consumer behavior. Challenge fixed
T/K/reserve, at most one exit observation per turn, nonblocking turns, absolute
wakes, starved callers and equality/overrun cutoffs, mandatory group KILL after
leader exit, PID-only scope, sticky errors, terminal idempotence, the sole final
inspection and signal epochs. Inspect the moved renewed-K mutation and the
three retained named regressions for a meaningful behavioral challenge rather
than a broken injection. Challenge real fixture identity, independent exact
reap/absence receipts, checker bounds and preservation/hash audit claims.

Account for all three full-run outcomes and the retained lost-result transport
sample alongside the five interleaved baseline/candidate pairs. Distinguish
source/mutation-anchor changes from unresolved transport repeatability. State
which claims hold, findings with source/evidence and disposition questions,
and material limits. Attempt a named regression within the review skill's
permitted work. Do not fix findings; reconciliation is earned by findings.

## Notes

No guests, privilege, installed changes or unrelated processes. Preserve every
sample. k116 owns compatibility-loop removal, shared earliest-deadline
scheduling, actual worker/teardown overlap, all remaining controls and final
bounded-inspection-k28 review. k28/k17 remain open; acceptance is 0/10,
unavailable. No empty integration leaf.

## Findings

Scope: the k114 commit `ae674a1` against parent `a42ba0c`. I read the diff, `controller.py` `_Termination` and `_terminate`, the new controls in `test_controller.py`, the termination section of `docs/specs/diagnostic-process-inspection.md` and the k114 evidence directory. I compared the new owner with the old `_terminate` from `a42ba0c`.

I ran no test, build, lint or format command, because a review may not. Findings F1 and F2 rest on reading the source, so the integrating session must reproduce each with a red control first.

### Claims that hold

- The constructor freezes T, K and the reserve before it publishes a receipt or signals. `advance()` never sleeps and never queries the process table. The sole final inspection stays in `_terminate`.
- Boundary cutoffs use `>=` consistently. The controls reach equality at entry (`first_turn=11.9`) and on return (`delay=1.9`). A `>` mutation of either check fails by reading.
- Group KILL after leader exit is kept. PID-only exit suppresses KILL and shortens K to `min(G, T)+A`. Both are asserted at `exited=True`.
- Terminal turns return `None` and change nothing. The signal epoch equals `len(events)` and the receipt asserts this.
- Mutants: I read `condition-audit.json` and `pytest.stdout`. `renew-term` fails 13 of 20 controls, `early-group` fails 3 and `late-wake` fails 2. Each failure is an assertion on timing or signals, not a `NameError` or a bad injection.
- The moved K-renewal mutation targets `_Termination` after `self._observe_exit()`. The anchor occurs once. The 0.2 s sleep pushes renewed K to T+0.35 s against the 0.3 s cap, so the margin cannot invert. The first-full failure `assert 0 == 1` was the old anchor, not a behavioural result.
- Commit `ae674a1` only adds paths under `termination-turns-k114/`. `git diff --name-status` shows no prior evidence path changed.
- Checker bounds in `run_condition.py` match the report: 60 s, 120 s and 30 s, then TERM 2 s and KILL/reap 2 s. The source and artifact hashes match `final-checked-inputs`. Only the string literal split differs from `final-inputs`.
- The exact-child receipts hold. Both retained green-controls receipts show TERM then KILL on the exact PID or group, return code -9 and `after.state == "absent"`.

### F1 (medium): the cutoff turn blames the observer when the leader never exits

`advance()` calls `_observe_exit()` before it tests `now >= inspection_deadline` (`controller.py:521-541`). After KILL the wake is `min(now+0.005, inspection_deadline)`. The turn that wakes at the cutoff therefore enters `_observe_exit`, which sets `observer_ok = False` and writes "exit observation cutoff exhausted before entry" (`controller.py:474-480`). A leader that survives KILL gets this error.

The old loop at `a42ba0c` read `while leader_exit is None and time.monotonic() < inspection_deadline`. It stopped at the cutoff without an observation error. `termination_ok` is false either way. The receipt now names an observer failure that did not happen.

No control asserts this. `test_termination_turn_deadlines_and_wakes` only checks `advance() is None` at 11.9 and never reads the receipt.

Question: should the cutoff test run before the observation, with a control that a surviving leader leaves `observer_ok` true? Or is the new label an accepted change that needs a control and a note?

### F2 (medium): one exit observation per turn is asserted only on the first turn

The controls assert `observations == [10.0]` after turn one. After that they compare the list only after the owner finished (`before == after`). The real-child controls count nothing. No retained mutant targets this rule.

Named regression: in `advance()`, call `self._observe_exit()` again after the KILL branch. With `exited=False`, each later turn then makes two `_exited` calls. By reading, all 20 new controls still pass. A caller could then lose its bounded-turn property without a failing control.

Question: should the deadlines control assert `len(observations)` after every turn, and should the evidence retain this regression as a fourth mutant?

### F3 (low): observation errors keep only the last text and retry each turn

Lines 480, 489 and 495 assign `exit_observation_error` unconditionally. `observer_ok` stays false, but the first error text is lost. After an `OSError` the owner calls `_exited` again on every turn up to the cutoff. The final turn then overwrites the OS error with the cutoff text from F1.

Question: should the receipt keep the first error and count later ones, or should the owner stop observing after an `OSError`?

### F4 (medium): the five baseline/candidate pairs cannot support a causal claim

The four `review_worker_fork_failure_evidence` cases import only `worker_capture` and `inspection`. Their hashes are identical in baseline and candidate. So the ten isolated runs are ten samples of the same code, not an A/B of k114. Each run is a fresh process, so it also omits the full-suite load that produced the failure.

Retained history gives a base rate. Across every `pytest.xml` under `evidence/`, `test_review_worker_fork_failure_evidence[lost-result]` has 76 passes, one expected failure in `k69-red-01` and one unexplained failure in `green-full`. That is about 1 in 77. At that rate ten clean runs happen about 88 per cent of the time, so they neither support nor refute a link to k114. The later full runs `green-full-2` and `green-full-3` carry the same weakness.

The report states this honestly: it does not claim to explain or erase the failure. The receipt shows worker exit 70, a valid errno 35 report and "incomplete worker frame length". Source and mutation-anchor changes explain the other failure. They do not explain this one.

Question: should k116 run full-suite repeats on the parent and on the candidate with every sample retained? Should it also inspect the worker-exit and fork-failure-channel close ordering, which this slice never touched?

### F5 (low): `audit.py` guards less than the report claims

For mutant conditions the audit asserts only that some `termination_turn` case failed. It does not check the 13, 3 and 2 counts or the failing names. It does not count exact-child receipts, so a condition with none still passes. It does not check that baseline samples used the parent controller and candidate samples the final one. I verified all of these by hand and they hold.

Question: should the audit assert the retained counts, the receipt count per condition and the pair input hashes?

### F6 (low): the 176,131-path claim includes 14,289 ignored cache files

`preserve.py` walks the disk. Git tracks 161,842 of those paths. The other 14,289 are ignored `__pycache__`, `.pytest_cache` and `.ruff_cache` files under older `inputs-*` folders. They are absent from the commit and from any clone. The claim "all prior paths unchanged" is a property of this checkout, not of the commit. The `git diff` check in "Claims that hold" covers the tracked part.

Question: should the report say that 14,289 paths are ignored caches, or should `preserve.py` skip them?

### F7 (low): the real-child controls have a timing margin and a checker escape

`test_termination_turn_real_child` uses `term_grace=0.08` and asserts `turns[1]["returned"] < owner.term_deadline`. A scheduler stall above about 80 ms between turns fails it. Three full runs did not hit this.

The fixture child uses `start_new_session=True`. If the checker times out and `killpg` ends pytest, the `finally` block does not run. The child then survives outside the checker group until its `sleep(30)` ends. No checker timed out, so this stayed theoretical. The checker receipt has no remedy for it.

Question: should T widen, or should the control assert wake values with the fake clock only? Should the checker record the fixture PID before any timeout?

### Material limits

- I ran nothing, so F1 and F2 are unconfirmed at runtime.
- No guest, shared scheduler, teardown overlap or acceptance claim is supported. k116 owns those, k28 and k17 stay open, and acceptance stays 0/10 and unavailable.
- Real children never exit on TERM with a surviving group member. That case reaches the owner only through the older `_terminate` controls, for example `test_review_exited_leader_does_not_hide_a_surviving_group_member`.

## Decisions (running log)

- 2026-11-15: The review has seven findings, so an `integrate-review-impl` leaf (k118) is cut ahead of k116. It carries only this handle.
