# complete-worker-scheduler-k140

## Goal

Expose one caller-owned scheduling boundary across helper query, request retirement and worker close, so callers can compose actual worker transport with competing owners without patching the module driver.

## Context

The aggregate remains in complete-worker-scheduler-k143 with the entire original charter and history copied verbatim. Read evidence/request-turns-k126/report.md and evidence/expired-request-review-k142/report.md for the current transport and ownership controls. All k28/k17 obligations remain open.

## Done when

- capture_helper and Worker.close accept an optional single-owner driver, retaining default compatibility.
- A supplied driver owns every query, retirement and close registration, including exhausted allowance and finalization failure, with original priority/readiness/deadlines and sticky failure.
- Real standalone/persistent success and stalled-creation controls compose through the supplied driver with competing owners; bypassing any phase is detected. Expired and finalization paths have focused regression controls.
- Retain RED, frozen parent/candidate checks, rejecting phase-bypass mutants, per-item outcomes, independent fixture audit, before/after subject digests and prior evidence preservation.
- Cut a separate independent review of this scheduling boundary. Keep the complete scheduler and final aggregate review in k143; do not claim production overlap.

## Notes

This is a flat sibling slice because it shares the existing bounded-inspection scope. It can be demonstrated with actual worker transport independently of controller integration.

## Decisions (running log)

- 2026-11-15: Choose an optional synchronous single-owner driver over generator suspension. The existing bounded callbacks can be composed by the caller without adding generator-finalization authority or changing their lifecycle. Every query/retirement/close path must forward the same driver. Production controller ownership and teardown overlap remain in k143, carrying the original charter/history unchanged.

- 2026-11-15: Require an independent review-impl leaf because this caller-owned driver becomes the controller's scheduling authority boundary. Existing aggregate review remains required separately. Ordinary verification ran here; no in-session adversarial reviewer was used.

## Delivery

See experiments/exec-reliability/evidence/caller-scheduling-k140/report.md and comparison.json. Final frozen candidate: 931 full tests, six audit-tool tests and all static checks pass. Four one-site phase-bypass mutants reject. RED and both intermediate failures remain retained. Parent full retained a late-created identity future failure (923 other passes); candidate passing does not establish a repair. Complete controller ownership, startup, overlapping teardown and final aggregate review remain in complete-worker-scheduler-k143. No ancestor closes.
