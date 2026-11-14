# complete-worker-scheduler-k55

## Goal

Reject nonpositive helper PIDs in receipts delivered by the actual per-query worker transport. These values are process-group selectors rather than exact helper identities when passed to process APIs.

## Context

Before this increment, the decoder checked PID integer type and creation-timestamp coupling but accepted zero and negative PIDs. The original complete runtime contract is preserved unchanged in the flat live sibling complete-worker-scheduler-k58; k48 and all ancestors remain open. Current worker-reported positive PIDs still lack authenticated publication and must not be treated as independent proof of helper ownership or reap.

## Done when

- Reject zero and negative helper PIDs before accepting a worker receipt; preserve optional missing identity on explicit creation failure and existing integer/type/coupling checks.
- Extend the real-fork hostile-channel controls with otherwise valid timestamps and matching PID/creation pairs. Require the exact PID rejection reason, conservative unknown-creation residual, bounded worker shutdown, exact worker reap and discarded output. Retain the expected RED failures and final green full diagnostic suite, lint, format and strict types with source hashes frozen before/after.
- Preserve historical evidence byte-for-byte, update the report, independently audit newly recorded owned process identities outside the faulted worker, and schedule a separate review-impl of this delivered boundary before k58.

## Notes

No persistent worker, gated launch, ownership authentication, shared scheduler or aggregate control requirement is discharged by this slice. No guests or installed-app actions; ten-clone acceptance remains 0/10 unavailable.

## Decisions (running log)

- 2026-11-14: Keep the current leaf as a small independently useful protocol hardening increment; flat sibling k58 retains the complete original contract. A gated-launch increment crosses launch, control ownership and shutdown together and is too large to claim here. No extra nested remainder node is created.
- 2026-11-14: Nonpositive PID rejection is version-invariant protocol logic and needs no new framework/library API. Executable hostile transport controls challenge this behavioral claim; the scheduled review supplies the separate fresh-context challenge.
- 2026-11-14: Retained `k55-red-01` reproduces exactly three accepted nonpositive-PID publications (189 other tests pass); boolean and string PIDs retain their existing type rejection. The two-line decoder check produces `k55-fixed-01`: 192 tests pass with lint, formatting and strict types. Matching creation stamps and parent/child timing witnesses isolate the new check from earlier guards.
- 2026-11-14: `k55-audit-01` stops on Ruff E501 before running its audit; retain that script and failure record. Corrected frozen `k55-audit-02` verifies both case sets, rejection reasons, PID payloads, source/evidence hashes, absent recorded PIDs (978) and scratch reconciliation (330 directories). Synthetic nonpositive selectors are explicitly excluded from process inspection. Source copies for RED were made after launch and match the runner's before/after digests; final copies precede launch. Report details and limitations are in `docs/experiments/exec-reliability-diagnostic-foundation.md`.
