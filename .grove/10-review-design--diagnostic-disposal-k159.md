# diagnostic-disposal-k159

**Reviews:** diagnostic-disposal-k18

## Goal

Review the evidence-storage design adversarially before the planning leaf
slices it.

## Context

The design is the spec `docs/specs/diagnostic-evidence-storage.md`, the visual
document `docs/design/diagnostic-evidence-storage/` and the k18 section of the
diagnostic-foundation report. The decisions log of `diagnostic-disposal-k18`
records the trade-offs. The design answers F3 of review
`diagnostic-foundation-k15`.

## Done when

## Notes

Test each claim against the controller code and the fork-admission rule.
Look for a filesystem call, a blocking wait or an acknowledgement on the
teardown path. Check that no storage bound adds to the cleanup or disposal
ceiling.
