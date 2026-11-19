# lifecycle-acceptance-k225

## Goal

Five fresh macOS clones, then five fresh Linux clones, pass the root
acceptance inventory in sequence through the app. They run under the freeze
manifest that leaf k244 reconciled from the k224 freeze.

## Context

- The freeze manifest
  `experiments/exec-reliability/evidence/acceptance-preparation-k244/manifest.json`,
  SHA-256 `1d021e2cac3d...`. The repaired trial refuses the k224 manifest.
  The section "Review k243 dispositions (k244)" of
  `docs/experiments/exec-reliability-acceptance-preparation.md` gives its
  changes and limits.
- The trial plan and checker of leaf k223, with the k244 repairs.
- Root brief: sections "Done when", "Acceptance inventory and bounds" and
  "Constraints".
- Spec: section "Validation route", steps 6 and 7.

## Done when

- Run the five macOS clones, then the five Linux clones, one at a time.
- Each clone passes every root case, the byte oracle and every ceiling.
- Each next clone shows the reset of the guest state.
- The ledger disposes of each owned VM within 45 s.
- Preserve every failed sample.
- Record the result as a count out of 10 with links to the evidence.
- If a clone fails, fail the trial and stop. Record the defect and its
  evidence. Plan the repair by the decompose rules.

## Notes

The private app must start after the last edit of the pinned configuration
file, because the app reads it only at start. Each clone hashes its golden
disk twice, which adds about 7 minutes to the trial.

A revised candidate needs a new freeze and a new trial. The historical result
of 0 out of 10 stays on record. Reach each guest by VM id through the app and
the Host CLI library (ADR-0004).

## Decisions (running log)

- 2026-11-19: all 30 pins of the k244 manifest had their frozen digests, and
  the manifest had its frozen SHA-256. No VM was running. The installed app,
  build 2.8.0, answered.
- 2026-11-19: the human authorised the app handover for this leaf only, on the
  terms of k224. The session quits `/Applications/TestAnyware.app`, opens the
  private `TestAnyware Dev.app`, runs the trial, quits only the private app by
  its bundle identifier, and reopens the installed app.
- 2026-11-19: trial `trial-01` ran from 09:22:55Z to 09:34:53Z. The result is
  **10/10, passed**: five macOS clones, then five Linux clones. The report is
  `docs/experiments/exec-reliability-acceptance-trial.md`, and the evidence is
  in `experiments/exec-reliability/evidence/lifecycle-acceptance-k225/`.
- 2026-11-19: the independent recheck passed each clone. A clone copy with one
  flipped bit failed it. Each golden disk and each of the 30 pins kept its
  frozen digest, and no trial VM remained.
- 2026-11-19: at 09:37:38Z the session quit only the private app and reopened
  the installed app. `doctor` showed build 2.8.0 again.
- 2026-11-19: no review leaf follows this leaf. It changed no code, and
  reviews k241 and k243 already checked the plan, the checker and the freeze.
- 2026-11-19: the root brief stays unchanged. Its text already says that
  acceptance stays 0/10 until k225 reports, and leaves k226 and k227 read this
  leaf's result.
