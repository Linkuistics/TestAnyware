# linux-agent-cancellation-k236

**Integrates:** linux-agent-cancellation-k235


## Goal

Triage the committed findings of linux-agent-cancellation-k235 against the
Linux implementation and its agreed contract. Reconcile actionable findings
before the macOS cancellation and Host CLI control leaves consume this work.



## Context

- Read the findings from the review's own commit, not from a restatement here.
- Producer: linux-agent-cancellation-k220, commit `b41c7492`.
- Contract: `docs/specs/guest-command-lifecycle.md`, ADR-0024, ADR-0025 and
  the producer's completion criteria and notes.
- Retained producer evidence:
  `experiments/exec-reliability/evidence/linux-agent-cancellation-k220`.

## Done when

- Each finding has a source-verified disposition under the integration
  acceptance and material-risk threshold.
- Actionable findings are reconciled in the implementation or contract, and
  appropriate controls and verification establish the resulting behaviour.
- The Linux agent's existing cancellation, responsiveness, retention,
  shutdown, record and download contracts remain accounted for.
- Documents and retained evidence state the actual result and its limits.

## Notes

This is a flat sibling because it repairs the same Linux artifact under the
same brief. It sits next to its review so intervening implementation cannot
silently move the cited code. Preserve earlier failed evidence. Do not replace
a golden or change the globally installed Host CLI or app.
