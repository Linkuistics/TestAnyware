# macos-timeout-inspection-k193


## Goal

Finish the k4 accounting of the macOS post-timeout inspection stall. Run the
complete original r3 fixture condition. Separate a blocked guest `ps` from an
exit that the agent did not observe.

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
- One instrumented clone records, for each post-timeout guest command, when
  the shell and its children exit and when the agent observes the exit.
  Deterministic process controls force the suspected orderings. The observer
  and its teardown are bounded.
- The report states which candidate the evidence establishes, or states that
  the cause stays unproved. A failed or timed-out inspection never proves
  absence.
- Owned children and clones are accounted for and disposed even on failure,
  with residual identities and remedies under the common root contract.

## Notes

Select no production ownership or cancellation mechanism. No broad process
name cleanup. The k4 run 02 is the unchanged clone of this inquiry for the
issue payloads. State how the r3 clone and the instrumented clone fit the
clone limit of the root brief.
