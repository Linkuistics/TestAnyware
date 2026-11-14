# complete-worker-scheduler-k94

## Goal

Start one persistent inspection worker before resource acquisition and reuse that exact direct child for sequential controller table requests. A startup or request failure is sticky: no replacement or retry; public safety teardown is still attempted. Close the worker within fixed bounds and retain lifecycle evidence.

## Context

Read the k28 brief chain, docs/specs/diagnostic-process-inspection.md, and complete-worker-scheduler-k96, which carries the entire original aggregate charter and historical decisions. This leaf delivers a coherent lifecycle slice, not the shared scheduler or complete aggregate contract.

## Done when

- Persistent raw-fork worker is admitted immediately before creation, resets inherited state/FDs and owns helpers in its own session; readiness is validated within at most one second before resource acquisition.
- Sequential framed requests reuse one worker; each request retains the existing gated helper publication, strict terminal validation, independent channels and byte caps. No EOF is required for successful persistent requests. Exact worker/session/request identity is retained.
- Startup/request failure stays sticky without replacement or retry; later workloads stop, safety cleanup/dispose/absence still run. Healthy close and failed startup/request retain fixed shutdown bounds, exact-child ownership and residuals.
- Retain frozen RED/GREEN real controls for reuse, startup ordering/failure, sticky failure and close. Run full diagnostic tests, Ruff/format and strict Pyright, preserving every previous evidence byte.
- Schedule a review-impl of this slice. Keep k28/k17 open; k96 retains the full remainder including shared scheduling, complete identity epochs, concurrent teardown/shutdown, all remaining controls and final aggregate review.

## Decisions (running log)

- 2026-11-15: The per-query protocol currently ends at EOF. Add an explicit matching request-completion marker so a healthy worker remains alive between requests. Keep legacy standalone capture for existing bounded adapter controls; controller execution always supplies the persistent worker explicitly. The shared earliest-deadline scheduler and overlap of worker retirement with teardown remain k96 work.

- 2026-11-15: Controller ownership is explicit through `_InspectionState.worker`; legacy standalone capture remains for adapter controls. Successful requests retain the child; a validated token completion marker replaces EOF as their boundary. Startup admission is repeated after pipe/mask preparation immediately before raw fork, and resource acquisition follows timely readiness.
- 2026-11-15: Retained full-1 exposed four old adapter signatures and a teardown exception path that bypassed closure. Safety teardown now nests a worker-close `finally`; a real raising-adapter control verifies exact-child reap. Full-1 also retains a late partial-identity deadline/reap failure whose intermittent mechanism is not established by the later passing sample. Full-2 retained one stale admission-reason assertion; its adapter now rejects the helper-side native admission explicitly. No failed sample was replaced.
- 2026-11-15: Expired requests fail the whole session before dispatch, close command authority without extending an expired ownership allowance, and cannot be retried. The fixture independently reconciles that exact child after EOF, outside runtime deadline authority. Healthy close fixes one two-second grace/four-second outer allowance; request failures keep the earlier request deadline. Shared scheduling and concurrent retirement remain k96.

- 2026-11-15: Retain full-4 (492 pass/5 legacy timing failures) and its overlap with auditor static checks. Run unchanged final inputs with no concurrent checker to isolate that factor: full-5 has 497 pass plus Ruff/format/strict Pyright. This does not establish the intermittent failures' mechanism or repair; the report and following independent review must preserve that limit. Final frozen baseline/runtime controls are red-final-2 (16 fail) and green-final-2 (16 pass), reconciled by exact case identity.

## Artifacts and verification

Runtime and controls: `experiments/exec-reliability/{controller,inspection,worker_capture,test_controller,test_inspection,test_worker_capture}.py`; reusable `diagnostic:persistent-worker-check` in root Taskfile. Retained report, all failed samples, final frozen RED/GREEN, full-5 and audit-3 are under `experiments/exec-reliability/evidence/persistent-worker-k94/`. The final audit reconciles exact case identities, immutable subjects/artifacts, independent process absence/exact-child evidence and all 98,328 prior tracked evidence paths unchanged. Full-5: 497 tests plus Ruff/format/strict Pyright pass; runner/auditor tools-4 pass. This certifies the stated lifecycle slice only; the intermittent prior failures and k96 aggregate remainder stay explicit in the report.
