# complete-worker-scheduler-k134

## Goal

Convert persistent worker startup readiness from its private blocking loop to bounded common-driver turns. Preserve startup admission, identity/frame validation, immutable readiness cutoff, sticky failure and existing bootstrap retirement receipts.

## Context

The entire original aggregate charter and historical handoffs were transferred byte-for-byte (except the heading) to complete-worker-scheduler-k137 before narrowing this leaf. That flat sibling owns all remaining concurrency, teardown overlap, controls, historical failures and final bounded-inspection-k28 review. k28/k17 remain open. Guest acceptance remains 0/10 unavailable.

Read the k101 bootstrap report, k105 review and k106 reconciliation before changing startup; read readiness-review-k136/report.md for common-driver advisory readiness behavior. This slice changes only the startup readiness owner; it does not establish shared production scheduling.

## Done when

- Startup readiness advances in bounded nonblocking callbacks through the existing common driver; no private select/wait loop remains on this controller startup path.
- Each callback reads at most the 4097-byte startup frame limit, preserving exact session/PID/PGID/SID validation, partial/overflow/EOF rejection and no success at or after the original cutoff.
- Startup deadline is frozen once; repeat start and failure/retirement remain sticky. Readiness registrations stop using closed channels.
- Regression controls demonstrate bounded yielding, fragmented readiness, immutable cutoff, late identity and terminal/closed readiness behavior; retain rejecting controls against the parent and meaningful mutants.
- Freeze/hash all measurement subjects before/after; preserve prior evidence bytes; retain focused/full/static checks with per-item parent comparisons and independent fixture identity audit. Every failure remains explicit.
- Schedule a review-impl sibling of this producer before complete-worker-scheduler-k137. Keep aggregate acceptance open.

## Decisions (running log)

- 2026-11-15: chose flat siblings because startup is an independently verifiable runtime increment while concurrency and teardown overlap share existing k28 context. No new node/context boundary is warranted. Whole aggregate/history retained in k137 before narrowing.
- Independent adversarial challenge is deferred to the scheduled review-impl leaf; routine executable checks run here.
- RED controls reject the parent in four new cases. Initial candidate retains one expired-observation receipt failure; readiness_finished is now recorded at the callback's expiry decision so driver unwinding cannot replace that observation. All initial inputs/results remain intact.
- Initial parent-full-1 is excluded from acceptance because evidence instruments were edited while its checker was still running. Retain its drift receipt and run fresh interleaved parent/candidate pairs after freezing the final instruments. This is a measurement error, not a runtime repair or historical-failure allowance.
- Clean interleaved samples so far: parent-full-2 and parent-full-3 each pass 903/907 with exactly the four new controls failing; candidate-full-1 passes 907/907. Static checks pass, subject hashes stay fixed, and each full sample independently audits 13 fixture absences. The excluded initial run and historical runtime failure remain retained.
- Final candidate-full-2 passes 907/907 and all static checks. Both candidate per-item records agree; parent pairs differ only in the four new startup controls. All four single-site mutants are rejected. Prior preservation after-check finds 254,292 unchanged paths. Final evidence audit is the remaining acceptance check for this slice; aggregate scheduling acceptance remains with k137.
- Final audit passes: exact source/artifact/archived-instrument digests, four rejecting mutants, 907 per-item outcomes across both pairs, 254,292 unchanged prior paths and 93 independent fixture absence audits (13 from the excluded sample). report.md records scope, every failure and reproduction. Cut the independent startup review before k137 as the last work; retirement and one sealed jj change follow. No aggregate/node acceptance or guest acceptance is claimed.
