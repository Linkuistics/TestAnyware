# startup-turns-k138

**Reviews:** complete-worker-scheduler-k134

## Goal

Independently try to break the bounded startup-readiness owner and its evidence. Report findings against the artifact and contract, without implementing fixes.

## Context

Read the retired k134 task, worker_capture.py Worker.start, turns.py, the startup controls in test_bootstrap.py and evidence/startup-turns-k134, including the report, frozen parents/candidates/mutants, excluded measurement and executable evidence instruments. The governing startup contract includes the k101 bootstrap report, k105 review, k106 reconciliation, readiness-review-k136 and docs/specs/diagnostic-process-inspection.md. Establish observations from the artifact rather than inheriting its conclusion.

This review is of the startup slice. The original aggregate charter and every historical handoff remain in complete-worker-scheduler-k137. Worker.start is still synchronous. Production scheduler concurrency, teardown overlap and the final full-runtime review of bounded-inspection-k28 remain separate live requirements; do not accept those from this corpus or discard historical failures because a later sample passes. k28/k17 remain open, k18 owns storage and guest acceptance remains 0/10 unavailable.

## Done when

- Challenge callback work/read limits, fragmented/overflow/EOF frames, exact session/PID/PGID/SID validation, immutable cutoff and equality rejection, expiry receipt ordering and late identity handling. Distinguish real OS delay from injected-clock boundary controls.
- Challenge sticky failure/close/repeated start and readiness interests across lifecycle transitions, including closure before the first startup callback and stale descriptor registrations. Check any signal/reap path reached from startup failure against original ownership/cutoffs rather than only the success path.
- Assess whether the tests and mutants detect independent failure classes or merely mirror implementation details. Check what the competing-turn control establishes about the synchronous production owner.
- Verify measurement provenance and comparisons independently: source/config/spec/tool/binary before/after hashes, exact archived instrument versions, per-item parent/candidate differences, single-site mutants, excluded initial run, every retained failure and prior-evidence preservation. Check the independent checker identity/reap/absence receipts and their stated scope; do not equate a self-audit with independent evidence.
- Record each material finding with reproducible evidence and its practical consequence. Separate startup defects from original aggregate work still owned by k137. Retain new review evidence separately without editing historical corpus bytes. Follow the review skill for any integration handoff before the live k137 sibling.

## Notes

Graph coverage for worker_capture.py/test_bootstrap.py/turns.py was checked at generation 2026-11-15T02:19:52Z; worker metadata_changed, test metadata_match, turns not_tracked. Use exact-source fallback and current coverage for review claims. No privilege acquisition, guest operation, installed-app modification or unrelated-process cleanup is authorized. Fixture cleanup must retain exact identity validation.
