# macos-timeout-inspection-k194


## Goal

Triage the findings of review `macos-timeout-inspection-k192` against the
diagnostic produced by `macos-timeout-inspection-k4`, before remainder k193
consumes its tooling and conclusions.

## Context

- Read the findings from the review's committed artifact, rather than treating
  them as this task's obligations. Apply the integrate-review-impl threshold.
- The producer report is
  `docs/experiments/exec-reliability-macos-timeout-inspection.md`. The existing
  Taskfile diagnostic and retained k4 evidence identify the inspected artifact.
- `macos-timeout-inspection-k193` owns the remaining fresh-clone conditions.

## Done when

- Every finding is verified against the artifact and classified as an unclear
  contract, a real issue, a visible trade-off or noise.
- Accepted repairs and their appropriate verification are complete, or work
  meeting the threshold is placed explicitly using the Grove scope rules.
- The report and the handoff to k193 state the resulting evidence and limits
  consistently, while preserving the original failed samples.

## Notes

Keep the root bounds and clone accounting. This leaf is the review integration,
not a replacement charter for the remainder's causal inquiry.

## Decision log

All five findings of review k192 are real issues. Each one blocks the agreed
acceptance of k193 or a sound reading of run 02. This leaf repairs them.

1. The r3 online invocation differed from the original. The run 02 receipt
   shows `guest.sh online` with exit 1 and `$2: unbound variable`. The original
   `macos-03/clone-1/online.json` passed the pinned URL and hash with a guest
   timeout of 120 s. The plan now builds the call from `cases.json`. It also
   adds the original health check before the r3 sequence. A control pins the
   r3 order to the original clone-1 order.
2. The checker accepted wrong guest outcomes. Revision 3 requires a typed
   receipt with the planned command. It compares each r3 exit with the original
   exit. It checks health, transfer paths and sizes, the missing-artifact error
   and the pinned download bytes. Twenty wrong-outcome controls and two byte
   controls fail one call each.
3. A failed inspection could report absence. Revision 3 reports absence only
   in a valid observation of the plan's own payload with exit 1 and empty
   stderr. A valid r3 observation reports `not-listed`, because the fixture
   masks `ps` failure. The owned cleanup now checks `kill -0` after each signal.
   It ends with `cleanup confirmed` or `cleanup uncertain` and exits 1 on
   uncertainty. Local runs under sh and zsh with a TERM-ignoring child gave the
   expected `terminated`, `killed`, `gone`, `other` and `unknown` lines.
4. Any stalled flag established a reproduction. Revision 3 admits a stall only
   with valid local evidence. That evidence is a successful host call, EOF on
   both streams and a valid receipt with `timed_out` true and exit -1. It also needs a valid
   earlier intentional timeout and no provenance failure. Incompleteness alone does not block admission. Five
   controls cover each blocked admission.
5. The report and the k193 handoff carried stronger premises. Both now keep the
   timeout, the printed rows and the elapsed times as facts. Completion of `ps`
   or the shell, the action that ended PID 630 and the agent's wait phases stay
   unproved.

Verification: `mise exec -- task diagnostic:macos-timeout-inspection-tools`
passes with 52 controls. Revision 3 on run 02 exits 0 with `evidence
incomplete, stall reproduced` and `after-09-table` admitted. It fails
`r3-online` and `r3-cancel` and reports the run 02 cleanup as `unknown`. The
recheck is retained as `k4-timeout-macos-02-outer/check-r3.json`. No VM ran in
this leaf. The run 02 evidence and the original failed samples are unchanged.
