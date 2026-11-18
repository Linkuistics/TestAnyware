# macos-agent-lifecycle-k234

**Integrates:** macos-agent-lifecycle-k233

## Goal

Triage the findings of review `macos-agent-lifecycle-k233` against the
implementation of `macos-agent-lifecycle-k219` and its agreed contract.
Apply the findings that block acceptance or identify a material risk, then
verify the resulting implementation through the repository tasks.

## Context

- Read the findings from the review's own commit, identified by its stable
  handle above. Do not treat their presence as a decision to accept them.
- Producer: `macos-agent-lifecycle-k219`, commit `d7e121df`.
- Contract: `docs/specs/guest-command-lifecycle.md`, ADR-0023 and ADR-0024.
- The existing owner evidence is in
  `experiments/exec-reliability/evidence/macos-agent-lifecycle-k219`.
- This is a flat sibling integration placed directly after the review, ahead
  of the first later live sibling, so no intervening implementation changes
  the source before triage. It directly unblocks the lifecycle owner's
  acceptance conditions and its later cancellation consumer, k221.

## Done when

- Each finding has a source-grounded disposition under the integration
  procedure's acceptance and material-risk threshold.
- Valid actionable findings are repaired and verified, genuine trade-offs
  are accepted visibly, and unsupported findings are rejected with evidence.
- Necessary verification uses the existing process, HTTP and Host CLI seams
  and the repository's mise/Taskfile workflow. Preserve failed evidence and
  keep each new measurement's subjects unchanged throughout the run.
- The resulting owner and record meet the agreed contract without claiming
  that lower-level checks replace the ten-clone acceptance trial.

## Notes

Cancellation, retention, downloads and shutdown still belong to k221. Read
the review's evidence limits before relying on its source-derived findings.
