# evidence-storage-k169

**Integrates:** evidence-storage-k161

## Goal

Triage the committed planning review before implementation starts. Verify each
finding against the spec, source and planned acceptance, then repair the plan
where the finding blocks acceptance or presents a material risk.

## Context

Read the findings from the commit retiring `evidence-storage-k161`. The
producer is `evidence-storage-k158`; its artifact is the `evidence-writer-k162`
node, currently `.grove/15-k162/`. The governing design is
`docs/specs/diagnostic-evidence-storage.md`.

## Done when

Each finding has an evidenced disposition. Accepted changes leave a coherent
sequence of independently verifiable slices, compatible callers and a defined
owner for every spec rule and required control. Any accepted tradeoff is
explicit. Implementation can proceed without an unresolved acceptance blocker
being silently passed to it.

## Notes

This flat sibling shares the producer's scope and root brief. It sits directly
before the implementation node so triage sees the reviewed artifact before
implementation changes it. Findings remain in the review's commit; this task
does not prescribe their disposition.

## Decisions (running log)

- F1 (high) is a real issue, and the plan is repaired. `execute()` writes no
  storage close (`controller.py:979-998`). The healthy retention test requires
  `verify()` to pass on a run directory from `execute()`
  (`test_retention.py:37-59`). So k164 builds the completion check in
  `retention.py` and does not call it from `verify()`. k166 switches `verify()`
  to the check when the writer owns storage. The healthy test then passes
  without change. The decision about run directories from before the writer
  moves from k164 to k166.
- F2 (high) is a real issue, and the plan is repaired. The intent gate stalls
  the writer on intent-2, so the close wait can block until k168 adds the
  deadline. The outbox overflow control depends on k168 in the same way. These
  two markers now stay until k168. A k166 slice test runs the intent gate with
  a resumed stall. Each control that keeps its marker must fail under its
  watchdog and leave no live writer or stopped process.
- F3 (medium) is a real issue, and the plan is repaired. The overflow control
  needs fixture output in the outbox. Streams reach the outbox only from k167,
  because `_call()` gives the evidence descriptors to the child
  (`controller.py:779-792`). The overflow marker therefore moves to k168.
- F4 (medium) is an unclear contract, and the plan is repaired. k163 now builds
  one fault fixture per control on the current storage paths. A missing writer
  or parameter is not a recorded failure. A recorded failure comes only from a
  behavioural assertion or a watchdog expiry. A control that reaches no failure
  is recorded and reported to the review. When a slice removes a marker, the
  fixture moves to the writer wrapper and the assertions stay the same.
- Marker ownership after the repairs: k166 removes storage error, writer death
  and readiness failure. k167 removes inherited state. k168 removes stalled
  writer, blocking call, outbox overflow, intent gate and no controller storage
  calls.
- New risk note in k164: map each record name to the file name read today
  (`result.json`, `plan.json`, `result-identity.json`). If they conflict, record
  the conflict before the cutover.
- No finding needs a redesign. So this session adds no producer or review
  chain. The slice order and the spec stay the same.
