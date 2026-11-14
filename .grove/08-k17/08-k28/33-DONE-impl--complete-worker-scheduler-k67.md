# complete-worker-scheduler-k67

## Goal

Gate helper exec on complete worker-side identity publication and retain independently published helper identities in the controller until a matching terminal reap acknowledgement. Ship this behavior through the existing per-query worker path.

## Context

Inherits worker-scheduler-k48 and docs/specs/diagnostic-process-inspection.md. The full original aggregate charter is preserved verbatim in sibling complete-worker-scheduler-k69. Read the raw-fork k62 artifact and reviews k65/k66/k68 before relying on that boundary.

## Done when

- A raw helper child resets inherited state, then waits on a private token gate with an immutable deadline. EOF, timeout or wrong token never executes argv. Its worker publishes helper PID, PPID, PGID, per-query token and creation time before releasing that gate, without controller ACK.
- The controller validates an independently published helper identity against the worker and dispatch interval, rejects duplicate/conflicting identities, and cross-checks a terminal receipt against it. A known helper remains a distinct retained obligation if a worker/result is lost; worker reap or group KILL never resolves it. A matching reaped terminal result resolves only that helper.
- Real controls demonstrate publication before exec, closed/expired gates, publication failure, successful helper retirement and interruption after known publication. Preserve existing bounded local tests and source-frozen RED/GREEN evidence. No guest/app changes or privilege acquisition.
- Schedule a separate review-impl leaf for this artifact/contract, before the full aggregate continuation. All aggregate ancestors and k69 remain open.

## Decisions (running log)

- 2026-11-15: This is a flat sibling slice because gated creation is independently verifiable in the existing worker path and shares k48 context. k69 carries the entire original aggregate charter unchanged. Gate publication is not persistent startup, the shared scheduler, complete request/epoch validation, or explicit fork-error/no-child messaging.
- 2026-11-15: The controller ledger records reconciliation obligations only; it gains no helper signal authority from a publication. Only the worker's unreaped direct child pins its PID. Independent adversarial challenge belongs to the scheduled review leaf; executable real controls verify this producer's named behavior in-session.
- 2026-11-15: The gate uses a private 32-byte token and the already-fixed helper capture cutoff; complete control-channel publication precedes the single release write. Control bytes are capped cumulatively, including consumed records. The controller ledger is independent of worker ownership and has no PID signal capability; even ECHILD preserves the published reconciliation obligation until a matching reaped receipt.
- 2026-11-15: Frozen k67-red-01 passed the existing 269 tests and failed the five new publication/ledger tests. k67-green-01 preserved its failures (three strict-type diagnostics and two residual-remedy expectation failures; 274 tests passed). These are retained evidence, not acceptance. Existing remedies are preserved when sufficient; ECHILD now adds the independent obligation remedy.
- 2026-11-15: Final frozen k67-green-03 passes all 288 cases plus lint, formatting and strict types; final nonbinary inputs match retained copies. k67-audit-01 preserves the two intentional blocked-disposal mock markers as a failed resource condition. k67-audit-02 passes after exact plan/bytes matching and removal of those two markers, with 2,290 retained exact PIDs absent and 14 identity receipts checked. No later observation acknowledges a helper reap or repairs unknown creation. All failed conditions and auditor preparation/lint failures remain retained.

## Artifact and verification

See experiments/exec-reliability/evidence/helper-publication-k67/report.md for the runtime contract, real and simulated control boundaries, retained RED/GREEN outcomes, exact-identity/resource audit and remaining limitations. The final subject digest check passes immediately before cutting the review; k69 begins with the original aggregate charter byte-for-byte apart from its heading.
