# admission-queue-k15

**Integrates:** admission-queue-k14

## Goal

Triage the committed finding of `admission-queue-k14` and settle the admission
design's deadline policy before planning consumes it.

## Context

- Read `admission-queue-k14` from its own commit. Its task file holds F1, the
  counterexample, supported rules and inspection limits.
- The reviewed producer is `admission-queue-k13`, commit `f90ba1dd`.
- The root brief and `plan-k1` hold the agreed requirements and test seams.
- `docs/specs/admission-queue.md`: the single observer and per-call bound in
  "The count", the events and expiry order of "The admission decision", "The
  wait and its end", the serial recovery batch, the protocol seam and risks.
- The CLI contract retains a bounded `--wait` and an immediate refusal without
  it. The queue, app spec, glossary and decision records must remain coherent.

## Done when

- F1 has a reasoned disposition grounded in the committed review and current
  design. Supported corrections are applied across the affected documents.
- The design states how expiry, the last admission decision and an immediate
  refusal proceed while an observation or recovery batch is busy, without
  freeing the slot of a live guest.
- The test seams cover slow successful calls and a deadline during an
  observation, including a slot that frees only after the deadline.
- The design is ready for `admission-increments-k3`, or the tree names further
  design work that must precede planning.

## Notes

This integrates an inspection-only review. F1 is a counterexample to the prose,
not an executed test. The review changed no production or test code and ran no
test, build, lint, format command or formal model. This session owns triage,
every fix and verification. Keep the human's requested focus on failures worse
than the design's stated risks; the human rejected a Quint model.
