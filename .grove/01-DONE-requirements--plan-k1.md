# plan-k1

## Goal

Establish the requirements for TestAnyware issue #14, preserve the owner's
acceptance conditions, agree the test seams, and hand the remaining causal
investigation and design to a bounded next session.

## Context

- Commissioning input: https://github.com/Linkuistics/TestAnyware/issues/14.
- The issue records the owner's choice of upstream TestAnyware fixes before
  Ravel-Strategies uses it for executable tutorial verification.
- The issue contains the original receipts, unchanged minimal inputs, exact
  comparison budgets and pointers to retained Ravel-Strategies evidence.

## Done when

- The root brief states the goal, constraints, acceptance evidence and open
  engineering questions without treating source leads as established causes.
- The human has agreed the test seams.
- The next useful session is stated concretely, with no speculative complete
  implementation tree.

## Notes

This is a fresh bootstrap leaf. It establishes what must be built; it does not
claim that the old failures have been reproduced or corrected in this checkout.

## Decisions (running log)

2026-11-01 — Use issue #14 as the commissioning input. It explicitly asks for a
new Grove and fixes the upstream-first scope. Preserve TestAnyware integration;
do not turn durable jobs, leases, pooling, distributed placement, Grove
orchestration or a replacement backend into prerequisites.

2026-11-01 — Carry the issue's acceptance bar unchanged. An unexpected timeout
or failed sample cannot be retried away. The prior required trial is 0/10 and
unavailable, not a passing result. The new fixed trial requires five fresh
macOS clones and five fresh Linux clones, the complete per-clone inventory,
unchanged comparison ceilings, exact byte checks and independent disposal.
No Windows failure was measured; shared protocol changes still need supported
agent compatibility checks.

2026-11-01 — Keep the causal questions separate: macOS ordinary completion,
macOS post-timeout inspection, Linux timeout classification, Linux owned-child
cleanup, Linux request availability after disconnect, and successful truncated
download. Current source is evidence about this checkout, not proof of the
installed macOS binary's cause. The lifecycle and transfer contracts must be
chosen from measured evidence before implementation.

2026-11-01 — The commissioning issue settles the product scope and acceptance
requirements. The remaining causal/API choices are engineering work, not
questions to re-interview the owner about now. The full grilling threshold is
not met: the required human check here is agreement on the proposed test seams.
The unresolved causes make this a multi-session workstream, so retain the
Grove and hand decomposition to a fresh planning leaf after that agreement.

2026-11-01 — The owner answered "Agree with these test boundaries": public
Host CLI plus the full ten-clone acceptance trial, supported by deterministic
agent process/file-handler regressions and existing HTTP/client/app integration
tests. Record these agreed seams in the root brief. Exact cancellation APIs and
file-consistency semantics remain design work after causal investigation.

2026-11-01 — No separate requirements review is needed for this transcription
of the commissioned contract and the owner's seam agreement. The next planning
leaf must retain implementation review and reconciliation, and commission
design review when the new lifecycle/transfer contract exists. No in-session
reviewer was used.

2026-11-01 — Added `execution-reliability-k2` as the next planning leaf. It owns
decomposition of the separate causal investigations and the subsequent design
handoff, while retaining implementation review and the full frozen acceptance
trial. This session changes requirements/process notes only; no production
code, installed tool, guest or golden was changed, and no repair is claimed.
