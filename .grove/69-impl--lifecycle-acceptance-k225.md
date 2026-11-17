# lifecycle-acceptance-k225

## Goal

Five fresh macOS clones, then five fresh Linux clones, pass the root
acceptance inventory in sequence through the app. They run under the freeze
manifest of leaf 68.

## Context

- The freeze manifest, trial plan and checker of leaves 67 and 68.
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

A revised candidate needs a new freeze and a new trial. The historical result
of 0 out of 10 stays on record. Reach each guest by VM id through the app and
the Host CLI library (ADR-0004).
