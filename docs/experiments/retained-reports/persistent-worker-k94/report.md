> Historical report. Paths and commands refer to the [archived checkout](../../README.md).

# Persistent inspection worker lifecycle (k94)

Controller execution now starts one admitted raw-fork worker before resource acquisition and keeps that exact direct child for sequential table requests. Its own session and descriptor/state reset reuse the existing helper isolation. Both initial admission and admission immediately after pipe/mask preparation must pass; readiness must match session/PID/PGID/SID and arrive within one second.

A separate bounded command pipe carries one session/request-token command at a time. The existing separate capped control/result channels retain gated helper creation publication and strict result validation. A matching completion marker replaces EOF for successful requests, allowing the child to remain alive. Completion omission, duplication or token mismatch fails the session. The legacy standalone adapter remains available for its prior fault controls; controller execution passes its worker explicitly through inspection state.

Worker/request failure is sticky: it never starts a replacement or retries a request. Controller inspection failure still prevents later queries and workloads while public cleanup, disposal and absence inspection are attempted. Worker closure runs in a finally around safety teardown, including when a safety adapter raises unexpectedly. Healthy closure fixes one two-second grace and four-second outer allowance; a failed request keeps its earlier deadline. An already expired request closes command authority without granting new signal/reap time. Strong exact-child ownership and an explicit residual remedy remain when the runtime cannot reap; the test fixture independently reconciles that exact child outside runtime authority.

## Controls and frozen records

The sixteen focused cases cover three successful requests through one direct child; five controller requests across acquisition, workload and all safety calls; initial and immediate-prefork admission rejection; silent, wrong-identity and late readiness; real worker SIGKILL with no replacement; startup rejection before any acquisition intent; safety-adapter exception closure; a worker ignoring shutdown until the sole fixed KILL; missing/duplicate/wrong completion; and sticky expired-request rejection with independent reconciliation.

Each sample retains declared commands/bounds, before/after SHA-256 inventories, raw stdout/stderr, exact JUnit cases, per-case evidence, checker PID/PGID/reap status and artifact hashes. The runner freezes all input Python modules, pyproject/lock, root and optional frozen Taskfile, itself, Python, ps and fixture executables. Focused checks have a twenty-second outer bound; full tests have 120 seconds, static tools thirty seconds. Checker timeout handling has TERM then KILL/reap allowances of two seconds each. Final source copies are retained in `inputs-final-2` and `inputs-baseline-2`; the earlier final draft and the expired/residual RED drafts also remain.

| Sample | Passed | Failed | Meaning |
| --- | ---: | ---: | --- |
| red-1 | 0 | 3 | First service controls fail before Worker exists |
| red-controller-1 | 3 | 3 | Worker controls pass; controller integration absent |
| green-controller-1 | 6 | 0 | Initial real integration controls |
| full-1 | 481 | 6 | Old adapter signatures, skipped closure and one deadline-sensitive legacy reap failure |
| green-2 | 15 | 0 | Expanded lifecycle/fault controls |
| full-2 | 495 | 1 | Adapter assertion still expected Python-thread reason instead of injected native-thread rejection |
| red-expired-1 | 15 | 1 | Exhausted request did not invalidate the service |
| full-3 | 497 | 0 | Intermediate runtime and all static checks pass |
| red-residual-1 | 15 | 1 | Unreaped lifecycle lacked explicit residual remedy |
| red-final-2 | 0 | 16 | Previous runtime with final controls |
| green-final-2 | 16 | 0 | Final frozen runtime and controls |
| full-4 | 492 | 5 | Legacy timing controls fail while auditor static checks overlap; all lifecycle and static checks pass |
| full-5 | 497 | 0 | Unchanged final suite with no concurrent checker; Ruff, formatting and strict Pyright pass |

Earlier `green-1`, `red-final` and `green-final` remain as measured drafts. No failed sample was overwritten or recast as runtime success. Early drafts retain subject hashes and raw failures; the final and targeted RED copies retain reproducible source bytes. The full suite runs in the actual repository layout to preserve existing Taskfile-relative tests; its source hashes must match the frozen final copy.

The late partial-identity worker-reap failure in full-1 and the five legacy timing failures in full-4 remain unresolved timing-sensitive observations. Full-4 overlapped auditor static checks; full-5 ran unchanged with no concurrent checker and passed. This isolates one scheduling difference but does not prove causation or repair. Later passing runs do not establish its mechanism or repair. Full-1 also exposed a real missing-finally path: four adapter TypeErrors skipped closure. The subsequent process-table check found PIDs 21105, 21109, 21113 and 21116 absent; the auditor retains a later raw check of those same PIDs. It does not infer helper death merely from worker reap.

## Reconciliation and scope

`audit-3/verdict.json` reconciles exact final case identities and outcomes, unchanged sample inventories/artifacts, source equality, no checker timeout/unreaped checker, and raw independent absence of eleven processes in each final controller reuse run: one worker, five helpers and five workload leaders. Direct lifecycle controls also assert exact-child WNOHANG/ECHILD after closure. The expired-request fixture records its independent exact-child reconciliation. Audits use no signals against unrelated processes.

The first auditor rejected jj copy labels for frozen inputs; its failure and input remain in `audit-1`. The second audit passed the runtime/evidence checks. Tool-check failures exposed missing collection annotations and one long string in the auditor; their raw outputs remain. The final runner/auditor pass Ruff, formatting and strict Pyright (`tools-4-verdict.json`).

Prior evidence preservation uses the jj content tree: all 98,328 baseline tracked evidence paths remain unchanged. The audit retains the baseline revision, sorted inventory digest and raw diff. New frozen inputs may be labelled copies; no modified, removed or renamed prior evidence is accepted. A one-command `snapshot.max-new-file-size=25000000` override records large audit/frame artifacts without changing persistent configuration.

This is the persistent lifecycle slice. It still has private polling waits and cannot preempt synchronous raw fork/exit observation. The shared earliest-deadline scheduler, complete run/workload/epoch identity, concurrent worker retirement and teardown, remaining aggregate controls and final bounded-inspection review remain in complete-worker-scheduler-k96. k28, k17 and root remain open. Ten-clone acceptance remains 0/10 unavailable. No guests, installed applications, golden evidence, privilege or unrelated-process cleanup were used.

Reproduce focused final controls with `mise exec -- task diagnostic:persistent-worker-check INPUTS=<absolute inputs-final-2 path> OUT=<fresh absolute directory> MODE=controls`. Use the actual `experiments/exec-reliability` path and `MODE=full` for full validation. Run the standalone auditor with `mise exec -- task diagnostic:resource-audit AUDITOR=<absolute audit.py path>` after changing its output directory to a fresh name.
