# lifecycle-delivery-k216

**Reviews:** lifecycle-delivery-k213

## Goal

Challenge the delivery plan of lifecycle-delivery-k213 before leaf 61 starts.
Find each requirement that has no owner. Find each order that leaves the
product broken. Find each leaf that cannot fit one session.

## Context

- The plan is the leaves 61 to 71 and the root brief section "k213 delivery
  plan".
- The design is `docs/specs/guest-command-lifecycle.md`. Read its sections
  "Implementation candidates", "Test seams and required controls", "Bounds"
  and "Validation route".
- The planning input is in lifecycle-evidence-k215, under its planning
  section.
- The root brief holds the acceptance inventory, the byte oracle, the ceilings
  and the required continuation.

## Done when

- Check that each part of candidates A to D has exactly one owning leaf.
- Check that each row of the controls table has an owning leaf. Check that
  each new row names its old failing path or states that none exists.
- Check that the record reader lands directly after the first record writer.
  Check that the cancel types land with `file cancel`, their only reader.
- Check that each implementation leaf leaves every existing test green and the
  Host CLI text output unchanged.
- Check that each implementation leaf fits one session. Name each leaf that
  needs a split.
- Check that the preparation, acceptance and handoff leaves keep every root
  case, byte oracle, ceiling and bound. Check that the freeze comes before any
  measurement.
- Check that the release leaf carries every Host CLI and agent change.
- Record each finding with its evidence in this leaf.
- If a finding exists, insert an `integrate-review-planning` leaf directly
  after this leaf.

## Notes

Review the plan, not the design. Lifecycle-evidence-k214 and k215 settled the
design. Raise a design defect only when a leaf cannot meet the spec as
written.
