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

## Decisions (running log)

- D1. R1 is a real issue. A `NOTE_EXIT` event makes `waitExit` return true
  without a status check, and the owner then calls a blocking `waitpid`. The
  spec forbids a blocking reap in the owner. The repair: `waitExit` returns
  true only when `waitid` with `WNOWAIT` reports the exit. After an event that
  the status does not yet show, the waiter polls the status until the
  deadline. An `ESRCH` registration also falls back to that poll. A narrow
  registration seam inside `KqueueLeaderWaiter` gives a deterministic control:
  it adds an immediate timer event in place of the process source.
- D2. R4 is a real issue with the same repair. Any failed registration makes
  the waiter poll the status every 10 ms until the deadline. The registration
  seam also gives this control, with an `ENOMEM` result and no source.
- D3. R2 is a real issue. The deadline arithmetic saturates at `UInt64.max`,
  and each `kevent` wait is at most 60 s. The request keeps today's accepted
  range, so an old Host CLI sees no new error. The control runs the owner with
  a timeout of 20000000000 s and with `Int.max`.
- D4. R3 is partly real. The stalled-read control accepted a time of 2300 ms
  against a spec bound of 2000 ms. It now asserts the bound of the spec. The rest of
  R3 is a trade-off of the contract, not a repairable defect. An output of N
  bytes costs O(N) to encode and to publish, so no read deadline bounds the
  publication of an unbounded output. The owner already measures the base64
  and legacy work inside `finalisedAfterMs`. A record cannot measure its own
  serialisation, and the spec makes the controller the authority for each
  phase. The spec "Limits" section now states this limit. The acceptance
  outputs are at most 20 bytes.
- D5. The Linux owner of k217 has a form of R2. `select.poll` raises
  `OverflowError` for a wait of about 2147484 s or more. That code is outside
  this leaf's charter. Leaf k220 next changes the same wait, so its notes now
  carry the defect and its control.
