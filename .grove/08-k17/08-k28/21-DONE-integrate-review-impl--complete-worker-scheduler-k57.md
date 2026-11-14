# complete-worker-scheduler-k57

**Integrates:** complete-worker-scheduler-k56

## Goal

Fix the two findings of the k56 review of the k52 receipt-timing slice. Fix the controls first, then close the decoder gaps. Keep old evidence bytes unchanged and retain each new check in a fresh condition.

## Context

Reviewed commit: ea652e2 (complete-worker-scheduler-k52). Runtime: `experiments/exec-reliability/worker_capture.py`, function `_decode` (lines 125-204). Child engine: `experiments/exec-reliability/inspection.py` (lines 129-136, 158-174, 216-217, 256-263). Tests: `experiments/exec-reliability/test_worker_capture.py`, function `test_review_worker_rejects_hostile_channel` (fault table at lines 370-387). Report: `docs/experiments/exec-reliability-diagnostic-foundation.md`, k52 section (about lines 1488-1530).

### Finding 1 (medium): two causal-order controls reject for the wrong reason

The faults `kill-before-create` and `reap-before-kill` are meant to prove that optional helper stamps must be in causal order. They do not prove it.

- Each fault also lowers `started_monotonic` to `now - 0.002` or `now - 0.003`. That puts the start before the parent dispatch time.
- Retained evidence shows the reason. In `evidence/k52-fixed-02/runs/`, `worker-hostile-kill-before-create-protocol-adapter-*.json` and `worker-hostile-reap-before-kill-protocol-adapter-*.json` both carry the error `worker query interval outside dispatch/receipt allowance`. The stamp-order message `worker helper timestamps out of order` never appears for them.
- The test asserts only `receipt.error is not None`. It does not assert the reason.
- Result: no control fails when the `kill < created` or `reaped < kill` branch of the loop at `worker_capture.py:194-199` is removed. Only `early-created` and `future-created` reach that loop.
- The outcome also depends on host timing. If the child needs more than 2 ms after dispatch, the start is no longer before dispatch and the fault takes a different path.
- The report sentence at about line 1488 says causal order is enforced. The sixteen-fault paragraph implies each fault is checked. Both overstate what the retained controls show.

### Finding 2 (low to medium): the decoder does not bind stamps to the other receipt fields

`_decode` checks the order of the optional stamps. It does not check that they agree with the fields they describe. In a genuine child receipt the pairs always agree: `inspection.py:173-174` sets `reaped` and `reaped_monotonic` together, `inspection.py:216-217` sets `pid` and `created_monotonic` together, and `inspection.py:259-263` sets `kill_errno` only after `kill_monotonic`. A corrupted or hostile publication can break each pair and still be accepted:

- `reaped: true` with `reaped_monotonic: null`, or `reaped: false` with a reap stamp.
- `pid` set with `created_monotonic: null`, or the reverse.
- `kill_errno` set with `kill_monotonic: null`.
- `capture_deadline_monotonic` equal to any value in `[started, deadline]`. The child always derives it as `deadline - min(0.05, max(0, deadline - started) / 4)`, so the parent can recompute it from two fields it already holds.

`controller.py:249-251` trusts `check.reaped` and `check.returncode` from this object. The worker is not an independent witness (see k56 scope), but a receipt that contradicts itself should be rejected, not retained as valid.

## Done when

- Give `kill-before-create` and `reap-before-kill` a valid start: not before dispatch, and not after the stamps. Each must fail when the stamp-order loop is removed. Retain a RED condition that shows it, then a fixed condition. Assert the rejection reason in the test, not only `error is not None`. Make no control depend on child scheduling delay.
- Assert the exact rejection reason for every synthetic timing fault, so a fault cannot pass through a different check.
- `_decode` rejects each coupling in Finding 2 and recomputes the capture cutoff. Add one hostile control per rule, each with a RED condition. Prove that the genuine early-exit, spawn-failure, killed-helper and normal receipts still pass.
- Fix the k52 report section. State which controls prove the stamp-order rule. Keep the statement that a child's helper identity and reap are not independently proved.
- The full retained runner passes lint, format, strict types and all tests in a fresh condition. Old evidence bytes stay unchanged. Preserve every k55 and ancestor requirement.

## Notes

The k56 review found these items sound. They are parent dispatch placement, receipt witness placement and the deadline checks before and after decode. They also include the finish and deadline overwrite and `published_times` retention. The wait and signal authority, the source and runner hashes, and the resource auditor are also sound. See the k56 decisions log.

## Decisions (running log)

- Both k56 findings are real repairs, derived from the reviewed decoder and retained errors. Keep the existing transport and helper lifecycle unchanged. Repair timing controls before tightening acceptance: sample start/finish after dispatch and reverse only helper stamps, with exact rejection reasons. Six new hostile controls cover both directions of reap/creation coupling, kill errno implication and the derived cutoff; genuine early exit and missing-executable receipts must remain valid publications.
- Preserve all earlier evidence bytes. Retain a frozen causal-loop-disabled RED condition, the original-decoder coupling RED condition, and a fresh full fixed condition with equal before/after digests. No retry, timing-bound increase or guest/privilege operation is part of this repair; k55 and all ancestor requirements remain live.
- The frozen order RED ran all 187 cases: exactly four helper-order controls and six new consistency controls failed because malformed publications were accepted. Restoring the original causal loop yielded exactly the six consistency failures, with 181 passes. Both conditions passed lint, formatting and strict types; their exact JUnit case sets are retained. Keep the loop and its existing rejection precedence, then append the three field relationships and exact derived cutoff check. This preserves the earlier timing errors while rejecting each new incoherent receipt for its named reason.
- The fixed condition passed all 187 tests plus lint, formatting and strict types. The first independent audit stopped before resource inspection: a nested-readiness publication witness was empty after the parent rejected readiness and shut down the child. Keep audit-01 unchanged. A new audit-02 condition permits an empty optional publication only for nested/duplicate-readiness, while requiring the actual failed worker outcome and every timing publication as before. No production source, test input or timing bound changes for this audit repair.
- Audit-02 validated the condition/mode receipts, but its separate resource enumeration still attempted to parse the same empty readiness witness. Preserve that failure as well; audit-03 applies the same narrowly scoped optional-witness rule in resource enumeration and hashes both earlier audit conditions. The test runner evidence and implementation remain untouched.
- Audit-03 passes lint/format and independent integrity/resource checks: all three 187-case sets match, each intended RED failure and final exact rejection is verified, genuine publications remain valid, 1,452 recorded exact process identities are absent, and 480 fixture directories are reconciled. Live/mixed/invalid/known-reaped selector controls pass. Before/after subjects match and every prior evidence digest matches the frozen baseline. k52 report now explicitly limits its original causal proof and records the new conditions and both failed audits. Worker self-report still does not authenticate helper identity or independently prove reap; k55 and its ancestors retain the full contract and final runtime review.
