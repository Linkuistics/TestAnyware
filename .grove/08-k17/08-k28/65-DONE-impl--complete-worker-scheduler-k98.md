# complete-worker-scheduler-k98

## Goal

Give persistent worker close one fixed orderly-close grace, then TERM and bounded KILL/reap escalation. Preserve an earlier retirement deadline and preceding signal attempts. Full aggregate scope and the original charter/log are preserved verbatim in complete-worker-scheduler-k101 under bounded-inspection-k28.

## Context

Read k94 lifecycle and its k97 review, k96 identity envelope and k99/k100 reconciliation, plus experiments/exec-reliability/evidence/request-identity-k96/report.md before relying on those boundaries. This slice changes only Worker.close; synchronous close remains until k101 supplies shared scheduling and shutdown overlap.

## Done when

- Healthy orderly exit receives no shutdown signal. A stuck healthy worker receives one fixed close grace then TERM; a TERM-ignoring worker receives KILL with TERM grace <=2s and KILL/reap <=2s, within the fixed original allowance.
- Failure receives no healthy-close grace. Earlier TERM/KILL attempts and finite deadlines are not renewed, duplicated or widened; repeated close is inert. Expired authority stays closed with explicit residual ownership.
- Retain frozen real-process RED/GREEN controls for orderly, TERM-responsive, TERM-ignoring, failure, already-retiring and cutoff paths, independent exact-child reap audit, full diagnostic suite, lint/format/strict typing and evidence report. Preserve prior evidence.
- Schedule independent review-impl of this slice before k101. k101 retains all aggregate requirements, shared scheduler/overlap and final k28 review. No guests/apps/privilege/unrelated processes; ten-clone remains 0/10 unavailable, storage k18 separate. Keep k28/k17 open.

## Decisions (running log)

- 2026-11-15: Flat siblings share k28 context. k101 preserves the entire incoming aggregate charter and history; k98 is independently verifiable worker-close behavior. Healthy close currently skips TERM and jumps to KILL after 2s. Fix only this local boundary, leaving shared scheduling/concurrent teardown to k101.
- 2026-11-15: Healthy close gets a fixed 50ms orderly EOF grace inside the existing four-second allowance. TERM starts immediately for failure or an already-retiring owner without a prior TERM; previous TERM/KILL attempts anchor escalation and are never repeated. TERM-to-KILL <=2s; original finite owner deadline always wins. Record attempts including denied signals; do not alter cutoff guards or helper/workload ownership.

- 2026-11-15: RED retained three expected behavior failures (TERM-responsive, TERM-before-KILL, earlier retirement reap); GREEN retained 17/17 focused controls. Full frozen suite: 588 passed; Ruff, format and strict Pyright pass including the new runner. All subject hashes stable; live Python matches full-inputs. Evidence/report and 51 exact-child audit receipts retained in experiments/exec-reliability/evidence/worker-close-k98/. Parent evidence content-tree comparison found 117652 prior paths unchanged. Six oversized receipts included using a command-local jj snapshot limit. Independent review remains the next step; full scheduler/overlap and aggregate acceptance remain k101.
