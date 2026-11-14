# complete-worker-scheduler-k71

## Goal

Retain valid helper identities received after the interim query cutoff as cleanup-only obligations, without repairing the failed query or renewing its fixed shutdown deadline.

## Context

The full original charter and its decision history continue in complete-worker-scheduler-k75. Read worker_capture.py, test_worker_capture.py, the helper-fork-error-k72 review and k74 reconciliation before relying on existing no-child authority. The spec requires late identity refinement independently of worker death/group signals.

## Done when

- Valid matching identities already read at the cutoff or delivered during bounded shutdown are retained individually, even when worker exit was already observed. Strict token, positive PID, parent/group, timestamp and original deadline validation remains; malformed/wrong identities create no authority.
- Late messages cannot repair failure, grant signal authority, acknowledge a helper reap, confirm a late fork failure, or extend query/shutdown deadlines. Control byte/read bounds remain enforced; shutdown does not become an unbounded EOF drain.
- Retained RED/GREEN controls cover actual worker transport, post-cutoff read/dispatch, shutdown publication, distinct identities, malformed data, overflow and unchanged deadlines. Independently account for the real worker/helper fixtures. Run lint, format, strict types and the full diagnostic suite against frozen subjects; retain prior evidence unchanged.
- Cut a specific review-impl of this slice before the remaining producer. The aggregate review of bounded-inspection-k28 remains k75's final producer obligation.

## Notes

This is an independently verifiable flat sibling slice under k28; no continuation node is introduced. Complete-worker-scheduler-k75 owns every original aggregate requirement, acceptance control and final review. k28/k17 remain open. No guests, installed app changes, privilege or unrelated processes; ten-clone acceptance remains 0/10 unavailable; storage isolation remains k18.

## Decisions (running log)

- 2026-11-15: Separate identity validity from observation timeliness. A matching late helper publication refines cleanup evidence only; the observation remains failed and the original shutdown deadline stays fixed. A separate scheduled review supplies the fresh adversarial context required by Grove.
- 2026-11-15: Full RED condition retained at k71-red-01: six missing-obligation failures, 329 passes. Validate creation time inside its original dispatch/deadline interval independently of receipt time. Shutdown parses only control evidence under the existing 4096-byte cap and fixed reserve; late readiness never widens PID signals, late terminal/reap and late fork-failure messages never create success/no-child authority.
- 2026-11-15: GREEN k71-green-01 passes all 342 cases, lint, format and strict types. Independent frozen k71-audit-01 confirms all 19 late controls, six named RED-to-GREEN transitions, 71,616 unchanged prior files, 1,273 real fixture PID selectors absent and two exact plan-owned mock markers removed across 623 scratch directories. The durable artifact is experiments/exec-reliability/evidence/late-helper-identity-k71/report.md. Later fixture absence does not resolve historical unknown creation; the aggregate remains k75's work. A separate review will challenge this slice before that producer runs.
