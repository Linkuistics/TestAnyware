# macos-timeout-inspection-k193


## Goal

Run the complete original r3 fixture condition of the k4 accounting as its
own condition. This leaf is the first slice of the k4 remainder.
`macos-exit-observation-instrumented-k195` owns the instrumented clone and
the cause statement.

## Context

- `docs/experiments/exec-reliability-macos-timeout-inspection.md` and the
  decision log of `macos-timeout-inspection-k4`. In run 02, `after-09-table`
  printed 460 rows that end with its own `ps` row, then timed out at 30 s.
  Only checker revision 2, changed after the run, counts that call. It is a
  post-hoc expansion, not a predeclared reproduction of the known-child
  observation, and it proves no shared cause. Completion of `ps` or the
  shell, the action that ended PID 630 and the agent's wait phases stay
  unproved until instrumentation measures them.
- The findings of review `macos-timeout-inspection-k192` and their repairs
  in integration `macos-timeout-inspection-k194`. Run 02's r3 samples are not
  the original condition: `r3-online` ran without its URL and hash, and the
  plan stopped at `r3-cancel`. The plan now passes the original online
  argument, checks health first and expects exit -15 from the cancel case.
  Checker revision 3 judges typed receipts and each role's expected outcome.
  It admits a stall only with valid local evidence and an earlier valid
  intentional timeout. The owned cleanup now reports `cleanup confirmed` or
  `cleanup uncertain`.
- The k3 mechanism and detector `diagnostic:macos-completion-old-path`.

## Done when

- The complete original r3 fixture sequence runs as its own condition on one
  fresh unchanged clone, with no issue payloads before it. Each original
  failed inspection stays distinct from the new samples.
- The report gives the r3 result and states what it adds to the k4
  candidates. A failed or timed-out inspection never proves absence.
- Owned children and the clone are accounted for and disposed even on
  failure, with residual identities and remedies under the common root
  contract.

## Notes

Select no production ownership or cancellation mechanism. No broad process
name cleanup. The k4 run 02 is the unchanged clone of this inquiry for the
issue payloads. The r3 clone is the unchanged clone for the r3 condition,
a stated new condition under the root brief. The instrumented clone
belongs to k195.

Placement: the r3 run needs only the frozen k4 tooling and the installed
tools. The instrumented clone needs a checkout-built agent and its own
controls. So each one is a separate session, and k195 follows this leaf.
