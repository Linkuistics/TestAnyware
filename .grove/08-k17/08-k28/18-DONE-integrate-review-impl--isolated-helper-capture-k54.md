# isolated-helper-capture-k54

**Reviews:** isolated-helper-capture-k53

## Goal
Reconcile the findings of review k53 on the worker channel before complete-worker-scheduler-k52 builds on it.

## Context
The findings are in the Decisions log of k53, anchored to commit 0189003 (producer k51). Re-read the cited code before changing it, because positions may have moved. The cited files are in `experiments/exec-reliability/`.

## Done when
- Each k53 finding is fixed, or rejected with a reason in this leaf's decisions.
- The worker starts correctly when the controller has a standard stream closed, or the leaf records why it cannot.
- A malformed or hostile result frame never raises past `capture_helper`. The receipt records the failure and unknown creation.
- Controls cover oversized, malformed, duplicate-key and early-exit frames. Controls state which are real and which are adapters.
- Retained evidence keeps every earlier failed condition.

## Notes
Scope is this slice only. k52 keeps the persistent worker, gated publication and the scheduler. Do not credit adapter controls as live worker evidence.

## Decisions (running log)
- Reconciled k53's committed findings at `0577db579529a4b490fa404225aa6cf49a44ab63`, against producer `01890039`, using Tier 2 graph evidence and direct source. The final coverage check reports changed metadata for the three edited Python files and untracked metadata for the evidence auditor; their source/diffs were read directly. No graph-completeness claim is made.
- F1, real issue: `reset_after_fork` now preserves explicitly allowed standard descriptors. Seven real subprocess controls close every nonempty combination of 0/1/2 before invoking the live worker; the baseline fails four combinations and the repair passes all seven. Channels can legitimately occupy those descriptors, so replacing every standard descriptor with devnull was incorrect.
- F2, real issue in part: the baseline nested result raises `RecursionError` through `capture_helper`. A shared JSON loader now translates decoder recursion into `ValueError`, allowing the existing failure/unknown-creation/shutdown receipt path to complete. Readiness uses that loader too. The review's suggested `TypeError`/`AttributeError` escape is not supported by the current validation: output types are checked before `bytes.fromhex`, and admission is checked as a dict before `.get`; the bad-field-type adapter returns a failed receipt. That part is rejected as noise rather than broadening the catch to arbitrary exceptions.
- Additional real issue found by the controls: readiness used last-key-wins JSON decoding. The duplicate-readiness baseline was accepted; shared duplicate-key rejection now rejects it before granting readiness authority.
- F3, real proof gap: added twelve child-publication protocol adapters covering nested result/readiness, malformed JSON, duplicate result/readiness keys, token, timestamp and field-type errors, oversized frame/channel, partial frame and exit before intent. These replace child publication only: fork/reset, pipes, bounded reads, parent decoding, shutdown and exact reap remain real. Every repaired hostile condition retains a failed receipt, unknown creation and worker reap within its original deadline. These are not live malicious helpers or proof of k52's full protocol.
- F4, real proof gap: a real unreaped sleep session pins the queried group while the live `controller._group_snapshot` uses the unmodified worker, helper and process-table parser. The group is observed live, worker/helper reap is checked, and the fixture is killed and reaped in `finally`. Existing direct-engine adapters keep their narrower scope.
- F5, real documentation issue: inspection module/capture docstrings now describe the synchronous worker-local engine and its actual per-query caller.
- F6, visible accepted limitation: child-reported helper identity/reap remains conditional evidence. No worker reap or group signal proves helper absence. k52 remains live and owns independent helper reconciliation, gated publication, the persistent worker and shared scheduler; no ancestor closes here.
- Retained `evidence/k54-red-01`: 6 failed / 156 passed; lint also found import order, while format/types passed. The escaped nested-result condition retained the parent's exact worker owner state after shutdown. `evidence/k54-fixed-01`: lint, format, strict Pyright and all 162 tests pass. Both self-checks retain stable before/after input hashes, checker ownership and artifact digests; no old sample was replaced or reinterpreted.
- Independent frozen auditor `evidence/k54-audit-01/audit.py` passed via `diagnostic:resource-audit`: 949 retained exact PIDs absent, 270 exact fixture directories reconciled, before/after inputs unchanged. It verifies both new envelopes and the earlier k51 auditor's recorded evidence hashes, and challenges its absence predicate with live/mixed/invalid/missing selectors. Later observations do not repair earlier unknown creation or k51's lost close-error identity. All failed conditions remain retained.
- Doubt reviewer declined because executable RED controls disprove the central behavioural claims directly, then pass after the minimal repair; another fresh read of the same source would be a weaker instrument here. No redesign or new producer chain is needed. No app/guest/privilege action, deadline increase or acceptance result: 0/10 remains unavailable.
