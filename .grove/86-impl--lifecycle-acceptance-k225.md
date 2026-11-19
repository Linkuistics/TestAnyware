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
