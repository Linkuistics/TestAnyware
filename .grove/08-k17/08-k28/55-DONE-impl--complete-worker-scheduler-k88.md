# complete-worker-scheduler-k88

## Goal

Attempt the first exact-child nonblocking workload-leader reap even when the fixed reap deadline has arrived or passed, while retaining deadline failure and ownership truth.

## Context

The k90 reconciliation explicitly leaves this boundary here. The full original worker/scheduler charter and running history are preserved in complete-worker-scheduler-k91; k28 and k17 remain open.

## Done when

- Close target signal/probe authority before one exact positive-PID WNOHANG attempt, including equality and expired-deadline entry. A successful late reap records status and releases its strong owner, but remains outside process bounds.
- An unsuccessful first attempt at/after K neither sleeps nor polls again. Retain failed ownership/residual and errno where applicable. Before K, later polls remain bounded by the original K.
- Retain real-child and explicitly simulated no-completion/error controls, RED/GREEN results, frozen subject and checker hashes, independent exact-child cleanup audits and full diagnostic checks without changing prior evidence.
- Document measured limits and schedule a focused review-impl sibling before k91 for the changed authority/reap boundary. The complete persistent worker/shared scheduler charter remains k91's work.

## Notes

Flat placement follows the existing k28 aggregate: this slice is independently useful and verifiable; no new context boundary is needed. It discharges only the first-reap boundary, not the original aggregate.

## Decisions (running log)

- 2026-11-14: Flat sibling decomposition keeps the full original k52 contract above here. k52 delivers only worker receipt timing validation in the actual per-query transport. Its review must reconcile before this producer relies on that boundary. Persistent bootstrap, gated identity publication, shared scheduler and every outstanding parent control remain this leaf's work; timing validation alone discharges none of those aggregate requirements.

- 2026-11-14: k55 was narrowed to reject nonpositive helper PIDs in the actual worker transport, with retained RED/GREEN controls. This leaf carries the entire original complete runtime contract above, including all outstanding controls and final aggregate review; PID validation is not helper identity authentication. Read the k55 review and any ensuing reconciliation before relying on that boundary.

- 2026-11-14: k58 delivers only terminal returncode/reap/ownership consistency in the interim transport. This flat sibling retains the entire original runtime contract, including remaining error/admission validation, all controls and aggregate review. Receipt consistency does not authenticate ownership. Read the k58 review before relying on this boundary.

- 2026-11-14: k60 delivers admission schema, error/errno and ownership-state consistency in the interim transport, including the k61 ECHILD finding. This flat sibling retains every aggregate requirement above; receipt consistency does not independently authenticate any helper. Read k60 and its review before relying on this boundary.

- 2026-11-14: k62 now delivers raw helper fork/reset and explicit local owners, with its own real controls. This sibling retains the entire aggregate contract above. Read k62 and its review before relying on that boundary. Raw fork does not supply gated publication, independent helper identity, persistent worker bootstrap or the shared scheduler; all remain here.

- 2026-11-15: k64 ships only the native SIGCHLD SA_NOCLDWAIT admission repair and its retained real controls. This flat sibling preserves the complete original aggregate charter above. Read the k64 artifact and ensuing review before relying on that boundary. Persistent pre-resource bootstrap, gated identity publication, immutable shared scheduling and every outstanding fault/control and aggregate review obligation remain here; all aggregate ancestors remain open.

- 2026-11-15: Flat sibling split: k67 delivers only gated helper identity publication and controller retention through a matching reap receipt. This leaf retains every requirement above, including explicit fork-error/no-child messaging, persistent bootstrap, full identity epochs, shared scheduler, all outstanding controls and final aggregate review. Read k67 and its scheduled review before relying on that boundary; all aggregate ancestors stay open.

- 2026-11-15: k69 delivers only explicit helper fork-error publication and conservative no-child accounting. Read its review before relying on that distinction. All aggregate runtime requirements and controls above remain here; no aggregate ancestor is complete.

- 2026-11-15: k71 is narrowed to cleanup-only late helper identity retention. This flat sibling owns the complete aggregate contract above, all remaining controls, and the final k28 review. Read k71 and its scheduled review before relying on that boundary. No persistent worker or shared scheduler is established by late identity retention.

- 2026-11-15: k75 delivers only incremental result framing in the interim transport. This sibling retains the ENTIRE aggregate charter and every remaining control above. Read k75 and its scheduled review before relying on the transport boundary; no persistent worker/shared scheduler is established.

- 2026-11-15: Flat sibling split: k78 delivers only admission/deadline revalidation after preparation immediately before worker/helper fork. This leaf retains the ENTIRE aggregate charter above, every outstanding control and final aggregate review. Read k78 and its scheduled review before relying on this boundary. All aggregate ancestors remain open.

- 2026-11-15: Flat sibling split: k81 adds only separately recorded signal-0 probe intervals, retaining the validated table-query timestamps separately. Read k81 and its scheduled review before relying on that evidence boundary. This sibling retains the ENTIRE original aggregate charter above, all controls and final k28 review; k28/k17 remain open.

- 2026-11-15: k83 ships only fixed worker shutdown-deadline guards at signal/reap entry points. This flat sibling retains the ENTIRE aggregate charter above, every outstanding control and final k28 review. Read k83 and its scheduled review before relying on that boundary. k28/k17 remain open.

- 2026-11-15: Flat sibling split: k85 now delivers only workload termination signal/exit-observation deadline admission, with frozen RED/GREEN controls. This leaf owns the ENTIRE aggregate charter above, including active-workload waits, probe/reap boundaries, persistent worker, shared scheduler, every outstanding control and final k28 review. Read k85 and its scheduled review before relying on this narrow boundary. k28/k17 remain open.

- k85 bounded slice: workload `_terminate` now fixes outer K before first TERM, samples signal/exit-observation admission at equality, and rejects late observer returns. Retained evidence/report: experiments/exec-reliability/evidence/termination-cutoff-k85/report.md. This does not complete shared scheduling, persistent ownership, `_wait`, group snapshot/probe/reap admission or any aggregate controls. full-1 retains one unknown permission table and four empty-PID real fixture failures; full-2 passes unchanged runtime/tests after a frozen-checker metadata path fix. Their intermittent mechanism remains unresolved; preserve every failed attempt and do not claim repair from the passing replay.

- k90 integrates k89: signal admission now closes at K-R, and receipts use the admission clock sample. See experiments/exec-reliability/evidence/termination-cutoff-k90/report.md. The spec requirement to attempt the first exact-child nonblocking reap even after K is already expired remains this aggregate leaf’s work; k90 does not change `_reap_leader`. All charter items and final review above remain binding. Earlier intermittent fixture failures are still unresolved.

- 2026-11-15: Narrowed this leaf to the first expired-deadline reap and copied its entire original charter/history to flat sibling complete-worker-scheduler-k91. The current loop checks the clock before its first waitpid, skipping the required attempt. The fix will make that first attempt unconditional and keep later polling under the original deadline; late success remains a failed timing receipt.

- 2026-11-15: Frozen RED controls produced exactly seven expected failures (six equality/expired outcome cases plus the existing expired simulated-reap control), with nine neighboring cases passing. The new tests audit a real exited child; withheld completion and ECHILD are explicitly simulated. Implemented an unconditional first exact-child WNOHANG attempt; only later polls consult the original K, and receipt timing/owner release logic is unchanged.

- 2026-11-15: GREEN controls pass all 16 exact cases; full self-check passes all 460 tests, lint/format/strict types. A second controls sample verifies the formatted and explicitly typed retained runner. The separate byte/receipt auditor checks per-case identities, six real-child audits per condition and 103182 prior evidence files; no guest or installed application work is claimed. Runner/auditor checking is now reusable through diagnostic:leader-reap-tools and diagnostic:leader-reap-audit.

- 2026-11-15: The byte/receipt auditor rejects a scratch condition exposing only five of six child audits (exit 1 at the named six-record assertion); original samples are untouched. Final audit includes all four samples and exact case-set comparisons. The retained runner and task recipes preserve earlier measured bytes; current tools pass independent lint/format/types. Focused review must evaluate this artifact and contract before k91 relies on it; no parent closes and no aggregate acceptance is claimed.

- 2026-11-15: `audit-verified.json` passes all four conditions, exact identities, six child audits each, 103,182 unchanged prior evidence files and 104,786 enumerated input files. Before/after inventory SHA256 is `429155b8cb86ed2e07c973c4b738b8f2c00a13321d9f5fe5551802d1114c56a3`. Final live diagnostic Python bytes exactly match frozen GREEN sources; the k91 continuation preserves the entire original k88 charter verbatim except its handle. No new target probes, larger bound, retry or aggregate acceptance was used.
