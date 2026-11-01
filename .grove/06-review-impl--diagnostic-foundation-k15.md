# diagnostic-foundation-k15

**Reviews:** diagnostic-foundation-k11

## Goal
Adversarially review the diagnostic controller, its retained self-checks and
route prerequisite handoff before the causal inquiries rely on them.

## Context
- Read the producer commit for `diagnostic-foundation-k11`, this grove's root
  brief, and `docs/experiments/exec-reliability-diagnostic-foundation.md`.
- Implementation is under `experiments/exec-reliability/`; Taskfile entry points
  are `diagnostic:self-check` and `diagnostic:preflight`.
- Final controller controls are under `evidence/self-check-final/`; the earlier
  `self-check/` and read-only `preflight-20261101-01/` retain their own source
  digests. The separate resource audit is linked in the report.

## Done when
- Review finite declarations, identity pinning before signals/reaping, PID-only
  CLI interruption versus group deadlines, grace/kill/reap bounds, exception
  paths, raw stream retention and file/hash consistency. Check what a blocked
  filesystem or a normally exited leader with surviving helpers actually means
  for the report's limits; do not infer arbitrary descendant containment.
- Check that failed acquisition still reaches exact owned disposal, cleanup and
  observation failures cannot suppress independent disposal, failed inspection
  stays uncertain, and no unrelated resource is targeted. Review the residual
  remedy and the audit separately from the original uncertain receipt.
- Verify that failure controls can reject a broken instrument, source hashes
  describe the run measured, and `ok` distinguishes instrument self-check success,
  raw host exit and guest outcome. Check each final control, not just totals.
- Verify public help/source support for the per-OS installation and teardown
  recipes. Challenge any unavailable route treated as usable, any stale Local
  Network observation treated as fresh health, and any hidden preparation
  shifted into a causal inquiry. No production repair or live VM disposal is
  established by these local mocks.
- Produce findings without fixes. Insert reconciliation ahead of the first live
  consumer only if actionable findings warrant it; otherwise retire. Preserve
  the live causal inquiry chain and the historical 0/10 unavailable trial.

## Notes
Do not invoke `agent:macos:test`: k3 still owns scoping its unsafe process-name
cleanup. The installed app occupies the default socket. Do not run a routing
suite that would replace/quit it, or live guest probes, to obtain this review.
The controller self-check uses owned local subprocesses and fake peers only.
