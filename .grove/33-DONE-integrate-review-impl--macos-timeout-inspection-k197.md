# macos-timeout-inspection-k197

**Integrates:** macos-timeout-inspection-r3-k196

## Goal

Triage the k196 review against the k193 artifact and its contract, and apply
the findings that meet the acceptance and material-risk threshold before k195
consumes the report.

## Context

- Read the findings from the commit for `macos-timeout-inspection-r3-k196`.
- Producer: `macos-timeout-inspection-k193`; durable report:
  `docs/experiments/exec-reliability-macos-timeout-inspection.md`.
- Next consumer: `macos-exit-observation-instrumented-k195`.

## Done when

- Each finding is independently verified and classified, with the disposition
  recorded and justified against the agreed contract.
- Any accepted repair and its proportionate verification are complete. The
  report and recorded conclusions give k195 a consistent evidence basis.
- Retained raw evidence remains intact, and any unresolved causal question is
  explicit rather than presented as an established result.

## Notes

This is a finite integration of the reviewed artifact, using the existing root
context. Keep optional hardening outside completion obligations. Select no
production ownership or cancellation mechanism.

## Decisions (running log)

- The k196 review has one finding. It is a real issue. The k193 report said
  that each absence came from a valid own observation. The fixture runs
  `ps ... || true`, so a row that is not listed does not prove absence.
- The report also said that the fixture's cleanup ended PID 700. That cleanup
  ignores the result of `kill` and records nothing for each PID. The owned
  cleanup proves only that both PIDs were absent later.
- The repair qualifies the k193 result paragraph in the report. It changes no
  data, checker or raw evidence. The checker already reports `not-listed`.
- The k193 log is a finished record. A correction bullet is appended to it,
  and the original bullet stays unchanged.
- The review also noted two limits: the checker does not attest the order of
  calls, and a failed outer inventory read may not propagate. Both are optional
  hardening outside this leaf.
- The time of each exit and the action that ended PID 700 stay open for k195.
