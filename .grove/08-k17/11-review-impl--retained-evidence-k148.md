# retained-evidence-k148

**Reviews:** retained-evidence-k21
**Creator:** declared claude

## Goal

Review the retained-evidence implementation before k17 consumes its stream pins, result identity and verifier.

## Context

Review the commit that retires retained-evidence-k21.
Read that leaf's done-when list and its running decision log in full.
Read `experiments/exec-reliability/retention.py` and the `_call` and `execute` changes in `experiments/exec-reliability/controller.py`.
Read `experiments/exec-reliability/test_retention.py` and the late-writer modes in `experiments/exec-reliability/peer.py`.
Read the k21 section and the F5 and F6 rows of `docs/experiments/exec-reliability-diagnostic-foundation.md`.
Use the retained evidence in `experiments/exec-reliability/evidence/k21-red-02`, `k21-green-01` and `k21-audit-02`.

## Done when

- Each claim in the k21 section has a check against the code or the retained evidence.
- The review states whether each stream pin and the exact stdout oracle use the same bytes.
- The review states whether the identity and the verifier detect changed, missing and added files.
- The review states whether the late-writer control keeps both the termination failure and the later rejection.
- The review states whether the documented limits are complete and true.
- Each finding has a severity, a file and line, and a failure scenario.
- When findings need action, an `integrate-review-impl` leaf exists for them.
- No live guest, app or acceptance claim follows from this review.

## Notes

k17 stays open after this review. Unavailable setuid `ps` and UID-denial controls stay unavailable.
