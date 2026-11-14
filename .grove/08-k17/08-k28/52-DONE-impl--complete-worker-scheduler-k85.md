# complete-worker-scheduler-k85

## Goal

Close workload termination signal and exit-observation admission at the original immutable cleanup cutoff. A delayed controller turn must not signal the target after that cutoff or accept a late exit observation as a timely final-query prerequisite.

## Context

This independently useful slice belongs to bounded-inspection-k28 and docs/specs/diagnostic-process-inspection.md. The full original aggregate contract and all prior decisions now live, without omissions, in complete-worker-scheduler-k88. Flat siblings share k28's existing context; another node would add no scope.

## Done when

- Establish the original group termination bound before the first TERM or exit observation. Preserve the existing earlier PID-only cap, mandatory group KILL while authority remains open, and no scope widening.
- Guard actual termination signal dispatch and exit-observation admission against the fixed bound, including equality. Reject exit results returned after the observation cutoff; record an explicit unsuccessful result when a signal was skipped at expiry.
- Retain frozen RED/GREEN and deliberate removed-guard controls for delayed turns, equality, before-bound dispatch, late observer return, and both PID/group scopes. Exercise existing real local-process controls and lint/type/full tests; retain failed attempts unchanged and hash subjects before/after. Preserve all prior evidence byte-for-byte.
- Document precisely this boundary and its limitations. Cut a review-impl of this producer. Keep complete-worker-scheduler-k88 and k28/k17 open; no persistent worker/shared scheduler or aggregate acceptance claim.

## Decisions (running log)

- 2026-11-15: Keep a flat independently verifiable workload termination admission slice here; k88 retains the entire original aggregate contract, control inventory and final k28 review. The present per-query worker remains interim.
- 2026-11-15: The claim under test is that delayed termination turns cannot dispatch signals at/after the original cleanup cutoff and late observer results cannot authorize a final query. Executable RED controls are the in-session disproof instrument; the scheduled review-impl supplies the independent adversarial read. No duplicate helper review is needed.

- Baseline controls: immutable red-1 timed out because a fake clock moved backward; red-2 completed but real audit serialization masked two assertions. Both retained. Corrected red-3 has 10 expected failures and two timely passes, including actual late KILL in both real-child scopes. Runtime then changed to establish the outer K before TERM, guard signal and exit-observation admission at equality, and reject exit observations returned at/after the inspection cutoff. PID narrowing and fixed group grace are preserved. These sampled admission checks do not promise protection against OS preemption between a clock sample and a syscall.

- Verification: final frozen controls green-2 pass all 14; each signal/entry/return guard deletion and >= to > mutation fails exactly its intended cases. full-1 retains 6 failures (one frozen-checker Taskfile lookup, four empty PID fixture outputs, one unknown permission observation). A fresh frozen copy changes only self_check metadata lookup and copies Taskfile, preserving runtime/test bytes; full-2 passes all 448 plus lint, format and strict types. The intermittent real fixture failures remain unresolved under aggregate k88; this is no claim of their repair. All failed samples remain immutable.

- Final audit audit-1 passes all 14 condition reconciliations and 24 real child records; 99,292 prior evidence file entries and 102,254 audited subjects remain unchanged. The report states identity and real-time limits. No ADR changes: this implements an existing cutoff contract. A separate review-impl will challenge the committed slice before the aggregate continuation; no aggregate ancestor closes. The launch has no GROVE_PROVIDER_EVIDENCE, so the review must omit Creator rather than invent provenance.
