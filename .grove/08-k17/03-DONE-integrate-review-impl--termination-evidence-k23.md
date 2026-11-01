# termination-evidence-k23

**Integrates:** termination-evidence-k22

## Goal

Triage the findings of review termination-evidence-k22 against producer
termination-evidence-k20, then apply the real ones before retained-evidence-k21.

## Context

Read the findings from the review's own commit. Read k17's node brief and the
producer's commit and task file.

## Done when

- Each finding has a recorded disposition, with evidence for each rejection.
- Accepted fixes have new frozen conditions, retained failures and audits.
- The handoff report and k21's inputs agree with the result.

## Notes

No live guest, app quit, unsafe macOS process suite or acceptance claim.

## Decisions (running log)

- Verified R1 and R4–R9 against producer 6b9766f6 and review 0ee42bc4.
  R1 is a real false-success classification and is repaired here, with residual
  group identities retained without changing PID-only signal scope. R4 is a
  malformed-path audit and overstated prose, not an unchecked real directory.
- R2 and R3 are real instrument limitations. A fixed sampling reserve cannot
  guarantee a ps completion time, and subprocess.run's timeout can wait without
  a bound after kill. Their repair requires an independently supervised
  inspection design, beyond this integration's repair charter. A concrete
  producer/review continuation will precede k21; no causal or acceptance work
  may rely on the current unconditional disposal promise.
- Repair R5–R8 with executable early-reap, isolated-overrun, failed-fork-cleanup
  and self-signal controls. Repair R9 by retaining an unreaped checker outcome
  and stopping further checkers on an exhausted KILL/reap allowance. These
  controls use real local children except the explicit unreapable-wait seam.
- Tier 2 graph generation 2026-11-01T04:50:10Z records matching coverage with no
  reported gaps for the four sources and historical audit. Source confirms the
  real process operations; the trace omits nested helper calls. No exhaustive
  graph claim is made. Preserve all original evidence unchanged.

## Implementation plan

1. Add focused R1/R7/R9 regressions and retain a frozen red condition.
2. Repair classifications, fixture ownership and checker failure reporting.
3. Add mutation controls for early reap, overrun/cap removal and controller
   self-signal; retain the full check and every failed condition.
4. Correct the audit with structured argv paths and a shared absence oracle,
   audit old and new exact resources, update the handoff and continuation inputs.
5. Cut the bounded-inspection producer ahead of k21, retire and seal this leaf.
- k23-red-01 retains three reproduced failures: both expected leader exits
  falsely succeed with live members, and an exhausted checker wait loses its
  verdict. k23-green-01 passes 42 tests but retains lint/type failures; those
  annotations/formatting are corrected rather than replacing the condition.
- k23-red-02 retains R7's missing out-of-band child publication against the
  old fork fixture. Its fallback signals only the still-pinned new group. The
  old child PID is unavailable because stdout was deliberately discarded;
  audit its recorded parent/group and retain that historical identity limit.
- k23-green-02 passes Ruff lint/format, strict Pyright and 54 tests. Four real
  early-reap mutations are rejected before group inspection reaches the OS.
  Isolated delayed reap succeeds in termination but fails the bound, and
  removing the success gate exposes false success. Removing the deadline cap
  is separately detected. Self-signalling the inner controller loses the
  independently observed survival proof; its known child is ended first and
  positively checked afterward. No broader containment assertion is made.
- k23-final-01 passes all four checks and 54 tests with the final Taskfile and
  retained unreapable-checker fixture output. Source subjects match before/after.
- R4's corrected audit classifies k20's old arrays as 67/82/97 real paths plus
  12/15/18 joined commands respectively. The old receipts are preserved; their
  claimed exact-directory totals are withdrawn. The same absence predicate
  now rejects live, mixed and usage-error responses and accepts a just-reaped
  helper's valid missing PID.
- Preserve failed audit conditions 01–03. Audit 01 lacked raw control responses;
  audit 02 shows the assumed PID exceeded macOS's range. Audit 03 exposed a
  reused historical PID belonging to an unrelated eza process and three
  unarchived preflight scratch directories. No unrelated process was signalled.
  Audit 04 archives surviving exact fixture outputs, accounts for recorded reaps
  before classifying any reused PID, and passes. Red R7's missing child PID
  stays unavailable; its recorded group is absent in the later observation.
- Final integrity verification recomputes 15,049 artifact/audit-input items and
  3,466 distinct subjects, including the current delivered source hashes. It
  passes with equal before/after digests. All original k20 condition paths have
  an empty jj diff; failed new conditions remain retained rather than replaced.
- Decline an in-session reviewer because executable red and mutation controls
  directly contest the repaired behaviors; inspection architecture is sent to
  a producer/review chain rather than asserted here. No new ADR is warranted
  by these bounded repairs. No live VM, image, archive or app was acquired.
- Inserted bounded-inspection-k24 before retained-evidence-k21 as the final
  work act. It owns the externalized R2/R3 design, schedules its concrete
  reviewed implementation before k21, and preserves k18's storage ownership.
  k17 has live work and does not close in this retirement.
- Retirement leaves k24 and k21 live, so no parent cascade closes. Current
  provider evidence is absent; no Creator line is invented. All new retained
  artifacts are included with a one-command jj snapshot size override, without
  changing repository or machine configuration.
