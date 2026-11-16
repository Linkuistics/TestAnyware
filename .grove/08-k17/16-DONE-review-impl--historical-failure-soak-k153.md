# historical-failure-soak-k153

**Reviews:** historical-failure-soak-k152

## Goal

Review the k152 soak instrumentation, plan, result and verdicts before the k17
whole-contract decision uses them.

## Context

Review the commit that retires historical-failure-soak-k152. Read that leaf's
done-when list and its running decision log in full.

Read the stamps in `experiments/exec-reliability/peer.py` and the prebuffer
fixture stamps in `experiments/exec-reliability/test_worker_capture.py`. Read
`timeline.py`, `soak.py`, `control.py` and `analyse.py` in
`experiments/exec-reliability/evidence/historical-soak-k152`.

Read `plan.json`, `report.md`, `verdicts.json` and `resource-audit.json` in the
same directory. Use the retained runs in `soak/` and `control/`. Read the k152
section in `docs/experiments/exec-reliability-diagnostic-foundation.md`.

## Done when

- The review states whether each stamp records the event that its name claims.
- The review states whether the frozen control detects a missing or misordered field.
- The review states whether each recurrence predicate in `analyse.py` matches its historical symptom.
- The review states whether the soak ran its frozen plan once, without retry or extension.
- The review states whether each "did not recur" verdict follows from its exposure counts.
- The review states whether the run-42 mechanism follows from the retained timestamps.
- Each finding has a severity, a file and line, and a failure scenario.

## Notes

The soak audit missed the leaked worker child of run 42. The decision log and
`resource-audit.json` record the manual removal. The fixture repair belongs to
`worker-deadline-cleanup`. Do not rerun the soak to check a finding. k18 owns
stalled storage. No live guest, app or ten-clone acceptance claim follows.

## Review result

The review inspected producer commit `a07f22793534bd076fde11553638576a3293f60c`.
Five P2 findings need integration before k17 uses the instrumentation and causal claims.
P2 means a material gap in the agreed evidence contract. None requires another soak.
The retained observations remain useful, and the five historical causes remain unknown.

### R1 — P2: identity-written stamps precede delivery

Location: `experiments/exec-reliability/test_worker_capture.py:1992`, with delivery at line 2052.

The fixture stamps `identity_written_monotonic` before witness-file I/O and before `original_write` writes the identity to the control pipe.
For `delay == "shutdown"`, it returns with the bytes held until TERM.
Thus this stamp can precede delivery by the whole shutdown wait, or exist when delivery fails.
For prebuffer, a worker stalled during witness publication would leave an early stamp despite no pipe delivery.
The timeline could attribute that delay to the wrong phase.
This fails the requirement that each stamp records its named event.

Integration should distinguish write attempt from completed delivery, including the held-publication branch.
Keep the existing helper-end upper bound explicitly labelled as a bound.

### R2 — P2: child-spawn observation has an invalid cross-process ordering rule

Location: `experiments/exec-reliability/peer.py:79` and
`experiments/exec-reliability/evidence/historical-soak-k152/timeline.py:24`.

The parent stamps `child_spawned` after `Popen`, a declaration write and a rename.
Meanwhile, the child can write readiness and stamp `ready_written` independently.
If the parent stalls after spawning, readiness can legitimately precede its spawn stamp.
The checker calls that sequence a reversed event, although the actual process creation necessarily preceded readiness.
It also moves the apparent spawn time past any declaration-file delay.
The control reverses synthetic numbers but cannot validate this claimed ordering against the actual instrumentation.

Integration should represent the observable boundary accurately and enforce only orders established by that boundary.
Simply moving the post-spawn stamp nearer `Popen` does not establish an order between independently scheduled processes.

### R3 — P2: the frozen control never tests missing KILL evidence

Location: `experiments/exec-reliability/evidence/historical-soak-k152/timeline.py:30`
and `experiments/exec-reliability/evidence/historical-soak-k152/control.py:35`.

`kill` appears in `SCOPE_ORDER` but not `SCOPE_REQUIRED`.
The control deletes only required fields, and the checker skips order pairs with a missing endpoint.
Consequently, deleting a present KILL produces no missing-field error or KILL-order error.
The two stubborn control records contain KILL, so they test reversed KILL ordering but never missing KILL.
The reported 87 detections do not establish the producer's required missing-field coverage for this event.

This matters because `scope_findings` uses KILL presence to identify interrupted-live-group recurrence.
Lost KILL evidence can look like no KILL, with no timeline warning.
Integration needs conditional presence checks against retained signal evidence and corresponding deletion controls.
Legitimate graceful exits must continue to permit absent KILL.

### R4 — P2: helper recurrence detection omits historical fixture timeouts

Location: `experiments/exec-reliability/evidence/historical-soak-k152/analyse.py:39`
and the JUnit checks at lines 127–135.

The predicate only inspects helpers nested in `group_observations` and flags their non-null `kill_monotonic`.
The historical helper case also explicitly includes fixture `/bin/ps` timeouts, including `test_turns`.
For example, `experiments/exec-reliability/test_turns.py:261` invokes `/bin/ps` directly with a one-second timeout.
A `TimeoutExpired` there produces neither a nested runtime-helper receipt nor either JUnit message checked by the analyser.
The run would fail, but its helper recurrence list would remain empty.
The case-wide verdict could therefore say “did not recur” despite the named historical symptom recurring.

Integration should cover those retained failure shapes or narrow the predicate and verdict to the runtime group-helper subset.
The 1260 timed group helpers establish exposure for that subset, not coverage of every listed fixture timeout.

### R5 — P2: run 42 promotes a shared-pause inference into a proved host mechanism

Location: `experiments/exec-reliability/evidence/historical-soak-k152/report.md:66`,
also `verdicts.json:293` in that directory and
`docs/experiments/exec-reliability-diagnostic-foundation.md:2183`.

The parent last polled at `440955.859085291`, before deadline `440955.923423416`.
Its receipt finished at `440956.063748166` with no signals and an unreaped owner.
Those observations support deadline exhaustion and the fixture cleanup defect.
They do not identify what occupied the unobserved interval.

The sampler interval runs from `440954.903141` to `440956.064472625`, a 1.161331625-second gap for a one-second sampler.
Its 161 ms lateness overlaps the parent's observation gap.
However, a one-second sample cannot locate its delay inside that interval or establish that the whole host paused simultaneously.
The report's assertion that timestamps show a host-wide stall of about 160 ms exceeds that evidence.
The same assertion propagates into the verdict and foundation document.

Integration should retain the measurements and label a shared scheduling or host pause as an inference with unknown cause.
The leak attribution remains valid without that stronger mechanism claim.

## Acceptance assessment

### Event semantics and frozen control

Scope `spawn` is the receipt's start observation, rather than an exact child creation timestamp.
Peer `start` records entry to `main` after imports.
`handler_installed` follows installation, and `child_ready` follows the readiness-file write.
`pid_printed` follows the flushed print. The loop-exit stamps record the respective observation points.
TERM and KILL come from receipt signal observations. Leader exit is the runtime's observation, not the kernel's exact exit instant.
Scope helper start and end use helper creation and reap timestamps.
Prebuffer dispatch and parent wait stamps identify their stated observation points.
`helper_end_bound` correctly identifies an upper bound after reap, rather than an exact completion time.
R1 and R2 identify the boundaries whose names or enforced order misrepresent the event.

The frozen control retained six records, including two stubborn KILL events, and detected all 87 mutations it generated.
It verifies missing required fields and reversed present pairs within that generated set.
R3 prevents accepting this as complete missing-event coverage. R2 prevents accepting every enforced pair as an actual causal order.

### Recurrence predicates and exposure

The interrupted-live-group predicate matches its named interrupted live-exit shape when KILL evidence is present.
The leader-exit predicate measures observed exit after TERM against the retained 150 ms grace.
The prebuffer predicate recognises either the expired witness or the historical JUnit prebuffer failure message.
The empty-PID checks cover absent or late print observations and the historical empty-integer failure message.
Their matching observations support bounded nonrecurrence, subject to the instrumentation findings above.
R4 limits the helper predicate to part of its historical case.
Checker-ceiling-k77 is outside the soak claim. Its retained input-runtime arithmetic supports the existing explanation.

The retained totals are 46 interrupted live-exit records, 138 TERMed records and 92 prebuffer witnesses.
The observed maximum TERM-to-exit interval is 60.462 ms, below 150 ms.
The prebuffer helper-start-to-end upper bound reaches 10.234 ms.
The retained group observations contain 1296 real `/bin/ps` entries, of which 1260 contain creation and reap timestamps.
None has a KILL timestamp. The minimum timed capture window is 87.481 ms.
These totals support the reported bounded observations, rather than an explanation of the historical causes.
The helper case-wide verdict needs the qualification in R4.

### Frozen execution and run 42

The retained schedule contains unique indices 0–45 and no unstarted entries.
It records 20 targeted idle runs, 20 targeted loaded runs, three full idle runs and three full loaded runs.
The run took 811.485 seconds within the frozen 2400-second budget.
The before/after subject hashes match, including plan hash prefix `13558d5168b3d2a0`.
The retained execution follows one schedule without a recorded retry or extension.
Only run 42 failed, without a runner timeout. Its failure remains retained.

The run-42 owner receipt has 40 reap attempts, no signals and `reaped: false` for PID 4100.
The later owner-record failure, manual process identity and removal support the fixture-leak account.
The stamped-peer audit's worker-fork omission is disclosed. `worker-deadline-cleanup` already owns that repair.
R5 qualifies the claimed cause of the deadline crossing, without duplicating the cleanup task.
The whole k17 condition remains unmet. No guest, app, ten-clone or stalled-storage acceptance follows.

## Review method and decision

This was inspection only. No tests, builds, lint, format commands or workloads ran.
Read-only calculations checked retained JSON counts and intervals without importing or executing the evidence tools.
The graph project was `Users-antony-Development-AbstractingComputers-TestAnyware.gh-issue-14`, generation `2026-11-15T02:19:52Z`.
Coverage reported changed metadata for established files and untracked freshness for the new evidence files.
Direct source reads supplied the material evidence where graph snippets were stale or unavailable.
The review makes no repository-wide completeness claim.

The five findings concern agreed evidence requirements and material attribution limits, so they earn one integration task.
That task must triage these findings from this review's commit. It must preserve the frozen soak and its failures.
