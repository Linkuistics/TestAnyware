# macos-timeout-inspection-k193


## Goal

Finish the k4 accounting of the macOS post-timeout inspection stall. Run the
complete original r3 fixture condition. Separate a blocked guest `ps` from an
exit that the agent did not observe.

## Context

- `docs/experiments/exec-reliability-macos-timeout-inspection.md` and the
  decision log of `macos-timeout-inspection-k4`. Run 02 reproduced a 30 s
  guest timeout on `after-09-table` with complete `ps` output.
- The findings of review `macos-timeout-inspection-k192`, if any.
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
