# lifecycle-delivery-k228

**Integrates:** lifecycle-delivery-k216

## Goal

Reconcile the planning review before the first implementation leaf starts.
Keep the delivery plan consistent with the agreed lifecycle contract.

## Context

- Read the findings from the committed review `lifecycle-delivery-k216`.
- The reviewed producer is `lifecycle-delivery-k213`. Its artifact is the
  root's "k213 delivery plan" and the implementation through handoff leaves
  k217 to k227. Use handles because insertion shifts their positions.
- The contract is `docs/specs/guest-command-lifecycle.md`, ADR-0023 to
  ADR-0026, k215's planning input and `.grove/_BRIEF.md`.

## Done when

- Verify and classify every finding against the producer and contract under
  the planning integration's acceptance and material-risk threshold.
- Apply the warranted planning changes and record each disposition with its
  evidence. Preserve the root acceptance inventory and continuation.
- Leave concrete, dependency-ordered implementation and validation work for
  every remaining root obligation.

## Notes

This is planning integration. It changes the plan rather than implementing
the product or rerunning acceptance. The review's commit owns its findings.
This task deliberately does not restate them as mandatory changes.

## Decisions (running log)

- Read the findings from review commit `7d0b3f57`. Verified each finding
  against the producer leaves at `7be5b751` and against
  `docs/specs/guest-command-lifecycle.md`. This integration changed only the
  plan. No product code, test or acceptance command ran.
- R1 is a real issue, and it blocks acceptance. Spec lines 677, 678, 683 and
  690 state "none, new behaviour" or name a recorded reference. The leaves
  demanded an old failure for every control. Leaves k217, k219, k220 and k221
  now label each control as old-fail, corrected-pass, new behaviour or
  compatibility. Each label follows the spec table row and names the old path
  where one exists. Two cases prove a row on the other platform: the
  SIGTERM-ignoring child on Linux, and a disconnect on Linux.
- R2 is a real issue and a material risk. The spec's "Agent shutdown" section
  had no owner. Leaves k220 and k221 now own bounded shutdown at SIGTERM.
  Each leaf adds a control during a command and a second control with a
  leader whose status stays unavailable.
- R3 is a real issue, and it blocks acceptance. Leaf k222 cited the
  controller's bound of 2 s, then 2 s, instead of the spec's "CLI
  interruption" steps. The steps are a new connection, one cancel with
  `waitMs: 0`, a 1.5 s wait, the default action and the same signal. Leaf
  k222 now requires these steps, the 404 path and both signals, and it tests
  the signal death within 2 s.
- R4 is a real issue and a material risk. Spec line 319 requires one
  accessibility lock, because AT-SPI is not safe across threads. Leaf k220
  now owns that lock boundary. It adds a control that holds the lock while a
  health request and a cancellation reply within 1 s.
- An extra change outside the findings: the leaves named siblings by stale
  positions after the k228 insertion. For example, "leaf 64" meant k220. All
  sibling references now use handles. References to retired leaves keep
  their positions, because those leaves do not move.
- Updated the root's "k213 delivery plan" for the k220 and k221 scope. Added
  the control labels and the handle rule. The order k217 to k227, the
  acceptance inventory, the 0/10 status and the continuation are unchanged.
  No new leaf or node was needed.
