# linux-agent-lifecycle-k229

**Reviews:** linux-agent-lifecycle-k217

## Goal

Challenge the Linux lifecycle owner of linux-agent-lifecycle-k217 before the
Host CLI reader of leaf k218 relies on its record. Find each place where the
owner breaks the spec's tombstone order, a bound or a validity rule. Find each
control that would pass on a broken owner.

## Context

- The producer's commit and its task file, with the decisions log.
- Spec: `docs/specs/guest-command-lifecycle.md`, sections "Exec record",
  "Bounds", "Test seams and required controls" and candidate B.
- ADR-0023 (typed record) and ADR-0024 (one owner ends each command).
- Code: `agents/linux/testanyware_agent/exec_owner.py`, `models.py`,
  `system_endpoints.py` and `server.py`.
- Tests: `agents/linux/tests/`, run by `task agent:linux:test`.
- Evidence: the report section "Run against the lifecycle owner (leaf k217)"
  in `docs/experiments/exec-reliability-linux-timeout-lifecycle.md`.

## Done when

- Each finding names its file, its line and a concrete failure.
- The review checks each control against its row in the spec's control table.
- The review reruns `task agent:linux:test` and reports the result.
- When a finding is worth acting on, insert an `integrate-review-impl` leaf as
  `references/decompose.md` directs.

## Notes

Cancellation, the threaded server, record retention and the exact-length
download belong to leaf k220. Their absence is not a finding.
