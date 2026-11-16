# storage-writer-k173

**Integrates:** storage-writer-k172

## Goal

Triage the committed review of the evidence writer and settle the actionable
findings before the controller client depends on its protocol.

## Context

Read the findings from the commit for `storage-writer-k172`, which reviews
producer `storage-writer-k164`. The contract is
`docs/specs/diagnostic-evidence-storage.md`. Apply the integration kind's
acceptance and material-risk threshold to each finding against the source.

## Done when

- Each finding has a recorded classification and supporting evidence.
- Any repair needed for the existing writer acceptance is implemented and
  verified, including a control that fails without the repaired rule.
- The diagnostic checks pass through the repository's Taskfile entry points,
  and the client slice has a settled frame and completion contract to consume.
- The integration records any accepted trade-off or remaining limit without
  expanding the agreed storage scope.

## Notes

This sibling is placed immediately after the review, before
`storage-client-k165`, so no intervening implementation moves the source
coordinates or changes the reviewed protocol. The findings remain in the
review's commit rather than becoming this task's charter.
