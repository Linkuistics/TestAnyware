# worker-deadline-cleanup-k157

**Integrates:** worker-deadline-cleanup-k156


## Goal

Triage the committed review of worker-deadline-cleanup-k154 and complete the
warranted repairs and verification against that producer's existing acceptance.



## Context

Read the findings from worker-deadline-cleanup-k156's commit, then inspect the
producer's committed artifact and done-when list. Use the retained k154 evidence
and the k17 brief to establish each finding's consequence for acceptance.

## Done when

- Each finding has a source-backed disposition under the integration procedure.
- Accepted issues have bounded repairs and proportionate verification; any
  accepted trade-off or unresolved acceptance gap remains explicit.
- The producer's evidence claims and the k17 handoff agree with the resulting
  evidence, preserving the original acceptance and its live remainder.

## Notes

Read the review findings at their committed source rather than treating this
leaf as a prescribed fix list. This is the next sibling under k17 so the review's
evidence coordinates remain adjacent to their consumer. Keep the historical
run 42 gap cause unknown unless new evidence establishes it. Do not rerun the
k152 soak merely to verify a review finding. k18 continues to own stalled
storage; no live guest, app or ten-clone work follows from this handoff.

## Decisions (running log)

- R1 is a real issue. The k154 control inventory held 248 children, but the
  receipts name 486 direct children of the control. Example: PID 84367 in run
  `pause-survey-nostall-final-20261116T222343-AC58EFB2` is absent from it.
  `evidence/worker-deadline-cleanup-k157/child_inventory.py` now lists each
  child from the retained receipts. It reads the k154 evidence and changes
  none of it. `task diagnostic:child-inventory` runs it with ruff and pyright.
- R2 is a real issue. The k154 audit credited the runtime with each child that
  was "reaped before audit". The k157 inventory names a reaper only from a wait
  status, an owner record or a removal record. PID 83687 now shows the fixture
  as its reaper. Totals: fixture 308, runtime 157, control 4, launchd 15 and
  unknown 2.
- Visible limit: the two unknown reapers are PIDs 78438 and 78439 in run
  `pause-survey-stall-after-20261116T221759-D57F2FF0`. Receipts name no
  signaller for 258 children. The inventory keeps both as unknown.
- Visible limit: the receipts cannot prove that they name every child. Eight
  runs end with no child of the control in `ps`, so each child there was
  reaped. A child without a receipt has an unknown reaper.
- The repair reads retained files only. It reruns no soak and no test, and
  `worker_capture.py` is unchanged.
- The doc section of k154, the k154 producer file and the k17 brief now cite the
  k157 inventory. The k154 acceptance stands with that inventory as evidence.
  The k17 whole contract stays not met, because five historical cases did not
  recur and have no demonstrated cause.
- k17 close: on 2026-11-16 the human accepted two visible limits and chose to
  close k17. The limits are the five cases that did not recur and the run 42
  gap. The k17 brief
  and the doc section of k154 record that decision.
