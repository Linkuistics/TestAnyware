# evidence-storage-k158

## Goal

Slice the evidence-storage design into implementation leaves that keep
teardown runnable when evidence storage stalls or fails.

## Context

Read the spec `docs/specs/diagnostic-evidence-storage.md` and its visual
document `docs/design/diagnostic-evidence-storage/`. Read the decisions log of
design leaf `diagnostic-disposal-k18`. Read F3 of review
`diagnostic-foundation-k15` and the review-design leaf that precedes this
leaf. That review can change the design. Integrate its findings into the spec
before you cut leaves.

The design changes the controller, adds an evidence writer and adds nine
controls. It needs no live VM. Ten-clone acceptance remains 0/10,
unavailable. Leaf `diagnostic-routes-k19` waits for this work.

## Done when

- Each implementation leaf has one testable outcome and names the spec
  sections that it implements.
- The order puts the controls first. Each control is seen to fail on the build
  without the writer before the repair lands.
- The slices cover the writer process, the frame channel, the outbox and
  pipe capture for calls. They also cover the phase bounds, the forward stop,
  the storage section of the run outcome and the audit-hook control.
- The slices keep the external CLI surface and the `execute()` callers
  compatible.
- The leaves sit before `diagnostic-routes-k19`. A review leaf for the plan
  follows the plan in the same session.

## Notes

The leaf keys and slugs are for the planning session to choose.
