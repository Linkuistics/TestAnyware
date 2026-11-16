# storage-client-k175

**Integrates:** storage-client-k174

## Goal

Triage the controller storage client review against the producer's contract,
apply findings that meet the acceptance and material-risk threshold, and leave
a verified client for `storage-cutover-k166`.

## Context

Read the findings from the commit for `storage-client-k174`, then rederive
them against the current source. The reviewed producer is
`storage-client-k165`. The review also records control seams for the later
cutover/publication slices; retain their existing ownership.

## Done when

- Every review finding has a source-backed classification under the integration
  procedure, with the reasoning recorded in this task.
- Accepted repairs have focused regression evidence and the repository's
  appropriate required checks. The full suite passes with the expected markers
  for the controls that still belong to later slices.
- The client is ready for cutover without widening this repair into the
  remaining execution/publication work.

## Notes

This is a sibling repair under `evidence-writer-k162` because it shares that
node's contract and has a finite handoff to the next consumer.
