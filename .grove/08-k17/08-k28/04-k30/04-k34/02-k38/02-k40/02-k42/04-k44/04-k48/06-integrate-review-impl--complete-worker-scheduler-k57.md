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
