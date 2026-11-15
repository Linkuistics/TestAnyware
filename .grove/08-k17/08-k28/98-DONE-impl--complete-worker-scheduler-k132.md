# complete-worker-scheduler-k132

## Goal

Wake bounded request and failure-retirement turns on pipe readiness while preserving safety-first earliest-deadline dispatch and immutable owner cutoffs.

## Context

The aggregate contract and its complete historical handoffs are retained in complete-worker-scheduler-k134. k28 and k17 remain open. Read the ancestor briefs and diagnostic-process-inspection spec. k129 removed the pre-registration command write; the aggregate opening assertion to the contrary is historical. k131 identified a zero-timeout transport poll followed by up to 5 ms sleep. This slice gives the common driver readiness interests without introducing new owner authority.

## Done when

- Driver waits for registered read/write readiness or its earliest timer; ready safety work precedes transport work, including readiness returned at or after a safety deadline. Preserve initial due dispatch, stable rotation, bounded idle fallback and terminal removal.
- Production request turns publish command-writer interests before dispatch and stream-reader interests afterward. Failure retirement publishes only its open cleanup reader. Existing per-turn caps, sticky failure and original cutoff checks remain authoritative.
- Exact-clock and real-pipe controls verify readiness routing, timeout/deadline behavior, safety priority, no terminal re-entry and production registrations. Reject relevant wake/priority/registration mutants.
- Freeze parent/candidate subjects and instruments; retain full and focused comparisons item by item, statics, timing failures and before/after hashes. Preserve prior evidence byte-for-byte. No guest or installed-app changes.
- Schedule independent review of this scheduler seam before the aggregate continuation. Concurrent production ownership and teardown overlap remain k134 work.

## Notes

Ten-clone guest acceptance is still 0/10 unavailable. Privileged real ps KILL/UID EPERM fixtures remain unavailable; acquire no privilege. Keep every historical failure in k134 and the report; clean reruns do not erase it.

## Decisions (running log)

- Split into flat siblings because the full concurrent scheduler exceeds one focused session and shares k28 context. The independently useful first slice removes polling delay; k134 retains the entire original charter and history.
- Readiness is an optional callable on a frozen Turn, sampled only while idle. Owners retain nonblocking I/O, immutable cutoffs and bounded reads/writes; the driver only wakes callbacks and never takes signal/reap authority.

- Frozen readiness controls pass 106 cases; the parent selection passes 10 old readiness-named cases and fails all nine new driver cases. All six wake/priority/registration mutants reject. The first candidate full suite passes 896 tests but strict Pyright rejects pending-list inference (five errors); an explicit `list[tuple[float, int, Turn]]` annotation passes a separately frozen static check. The original failed samples stay retained.
- Original parent-full-2 retains three failures: `test_review_exited_leader_does_not_hide_a_surviving_group_member[False]` returns -9 instead of 0; `[True]` observes leader exit at 349937.987481333 after KILL at 349937.980860666; `test_review_live_group_is_failure_even_with_expected_leader_exit[True]` returns -9 instead of 0. All checker/known-fixture audits complete. These failures join the aggregate's unresolved timing evidence; no later pass repairs them.

- Frozen-directory static checking additionally exposes a local `prepare` import resolution failure in the evidence auditor (eight Pyright errors), despite the direct tool check passing. Finish the immutable final sequence, retain those samples, then remove the auditor import dependency and verify both frozen parent and candidate statics; runtime bytes do not require another change.

- Final parent pair two reproduces the retained expired-request `residual_remedy is None` failure (test_worker_capture.py:2751); the exact recorded fixtures are absent. Carry the sample to k134 without claiming that later candidates explain it.

- The final candidate passes 896 runtime cases twice; the final focused run passes all 106. All six mutations reject against both candidate source conditions. Removing the auditor's local import dependency yields passing frozen-parent and frozen-candidate statics and a passing direct four-tool check. No runtime changed for that tool repair.
- The completed auditor verifies every shared/added case in four interleaved pairs, all frozen subject/artifact hashes, exact mutation sites, checker reaping/no timeout, 94 recorded fixture absence audits and 232,416 unchanged prior evidence paths. It exits nonzero deliberately: retained parent failures keep both comparison gates rejected. Current statics and final candidate runtime suites pass; aggregate acceptance remains false.
- The durable report and k28 brief carry the limits. Independent review complete-worker-scheduler-k135 is scheduled before complete-worker-scheduler-k134, which retains the entire original aggregate charter and histories. No ancestor closes.
