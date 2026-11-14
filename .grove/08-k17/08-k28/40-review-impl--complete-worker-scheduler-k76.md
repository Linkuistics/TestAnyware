# complete-worker-scheduler-k76

**Reviews:** complete-worker-scheduler-k71

## Goal

Independently review cleanup-only late helper identity retention for lost obligations, accidental authority and changes to the original shutdown bound.

## Context

Find the producer commit by complete-worker-scheduler-k71; read its diff, ancestor briefs, docs/specs/diagnostic-process-inspection.md and experiments/exec-reliability/evidence/late-helper-identity-k71/report.md. The runtime boundary is experiments/exec-reliability/worker_capture.py, especially _created, consume_control and capture_helper's normal receipt and finally paths. Trace the helper publisher and local reap receipts in experiments/exec-reliability/inspection.py too. The new real transport controls are experiments/exec-reliability/test_worker_capture.py:1544 onward. Retained conditions are k71-red-01 and k71-green-01, source snapshots k71-inputs-red-01 and k71-inputs-green-01, the prior-evidence manifest k71-inputs-01, and independent frozen audit k71-audit-01. Read helper-fork-error-k72 and its k74 reconciliation before assessing inherited no-child behavior.

The session had no GROVE_PROVIDER_EVIDENCE; no Creator is invented or copied. Review routing may require an owner declaration. Full aggregate runtime work remains complete-worker-scheduler-k75; this review runs before it.

## Done when

- Trace every control parser entry and both directions of ownership effects: timely receipt, delayed read return, queued select result, buffered bytes after observed worker exit, TERM-triggered shutdown publication, EOF, malformed/partial/overflow and conflicting/distinct identity sequences. Verify exact token/PID/type/parent/group/schema and original dispatch/creation/deadline validation. Challenge whether any valid identity is discarded or any invalid one obtains authority.
- Check that failure remains sticky; late readiness cannot widen PID-only signals; late terminal/reap or fork-error messages and late EOF cannot repair success, remove an obligation or establish no-child. Follow the helper ledger separately from direct worker ownership and group signalling. Later exact-PID absence cannot act as historical reap acknowledgement or resolve unknown creation.
- Inventory changed waits, reads and signal calls. Verify fixed query/shutdown deadlines, no grace renewal, the cumulative 4096-byte cap plus overflow detection byte, per-turn read bound, nonblocking behavior without unbounded EOF draining, and safety signal/reap work before shutdown reads. Trace callers and error/finally exits, including worker already reaped and control invalid/overflow.
- Assess all 19 controls against actual code paths: valid/distinct/malformed/overflow over read/select/shutdown, seven invalid fields, real TERM publication and late reap suppression. The synthetic PID is evidence only; delayed transport is an adapter, not an induced kernel failure. Check the six named RED-to-GREEN transitions, unchanged passing cases, strict types/checker receipts and frozen input/artifact hashes. Independently assess the auditor's 71,616 prior-file check, 1,273 exact real selectors, 623 scratch paths and two plan-owned marker removals within their stated bounds.
- Record findings with source coordinates, concrete consequences and feasible controls. For actionable findings only, insert an integration sibling before the first subsequent live sibling (currently k75); no empty integration. Preserve retained samples and distinguish this slice from persistent bootstrap, full identities/epochs, the shared scheduler, remaining aggregate controls and the final k28 review. Keep k75, k28 and k17 open; ten-clone acceptance stays 0/10 unavailable and storage remains k18.


## Goal



## Context

## Done when

## Notes
