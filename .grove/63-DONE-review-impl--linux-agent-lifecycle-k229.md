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

## Decisions (running log)

- Reviewed producer commit c6084c28da7a29b67bf3a1785b9328bd55d188cb against
  the current source, the lifecycle spec, ADR-0023 and ADR-0024. No production
  or test code changed. The review is inspection only: the loaded
  grove-review-impl skill's references/review.md says a review does not run
  test, build, lint or format commands. Therefore this session did not rerun
  task agent:linux:test; post-fix execution belongs to integration.
- Graph tools were unavailable. The graph CLI list_projects failed with
  "CBM CLI could not start because a pre-coordination or unverified CBM
  generation is active; close all CBM sessions and commands, then retry".
  Used direct source inspection; no graph coverage or completeness claim.
- The three findings below concern the agreed publication bound, record
  validity and the tombstone safety control. They merit integration before
  k218 consumes the record. A flat sibling integration shares this brief and
  directly unblocks that reader; it needs no new context node.

## Findings

Source coordinates below refer to producer commit c6084c28.

### F1 — P1: publication work remains outside the capture deadline

At agents/linux/testanyware_agent/exec_owner.py:260 the deadline bounds only
the worker join. The owner then copies the entire accumulated bytearray at
agents/linux/testanyware_agent/exec_owner.py:264. The worker has no deadline
check or captured-size ceiling and continues reading after abandonment. More
work follows the finalisedAfterMs sample at
agents/linux/testanyware_agent/exec_owner.py:452: synchronous directory removal
at agents/linux/testanyware_agent/exec_owner.py:375, legacy UTF-8 conversion
at agents/linux/testanyware_agent/models.py:269, base64 and SHA256 at
agents/linux/testanyware_agent/models.py:104, and HTTP JSON encoding at
agents/linux/testanyware_agent/server.py:110.

A command producing sufficiently large stdout can therefore overrun the
2-second outcome-to-publication or KILL/drain bound even with a responsive
capture file; an incomplete prefix near the read deadline still needs copying,
hashing and encoding. In particular finalisedAfterMs and withinBounds exclude
the unbounded serialization work and can report success for a late reply.
This contradicts the spec's outcome-to-published-record bound and its schedule
of counting and hashing in the worker. The stopped-read fixture yields only
five bytes, while the 32 MiB control at
agents/linux/tests/test_exec_lifecycle.py:224 checks only the internal timer,
so neither detects this failure. Integration should establish a bounded
publication path and a control that measures it through serialization with
substantial captured data.

### F2 — P2: stat failure emits a record invalid under the agreed schema

At agents/linux/testanyware_agent/exec_owner.py:227 a failed stat becomes
size -1; agents/linux/testanyware_agent/exec_owner.py:269 converts that into
an incomplete read_error stream with bytes 0 and fileBytes 0. Spec validity
requires bytes < fileBytes for an incomplete stream. The local oracle at
agents/linux/tests/exec_validity.py:75 correctly rejects this record.

This is reachable by an attached command under the agent's own uid, for
example: rm -- "$(readlink /proc/$$/fd/1)"; printf x. The command unlinks its
capture path while its stdout descriptor remains open. At exit, the owner
cannot stat the path and publishes that contradictory stream. The read-error
test at agents/linux/tests/test_exec_lifecycle.py:287 only substitutes open
after a successful stat, so it misses this branch. The missing-file/unknown-size
case needs a representation consistent with the contract, plus a control;
integration may need to clarify the schema if no truthful existing variant
can represent an unavailable size.

### F3 — P2: the cooperative-exit control does not protect tombstone order

At agents/linux/tests/test_exec_lifecycle.py:159 the cooperative-leader
control checks the recorded SIGTERM status, kill time and withinBounds, but
never observes whether the leader is still unreaped at the actual SIGKILL.
The other timeout tests likewise do not assert the signal/reap ordering.

A broken owner that reaps a cooperative leader, caches its status and then
sends SIGKILL to the saved group ID can satisfy these assertions. With no
PID reuse during that short test, the empty-group probe still succeeds. In
production that order releases the identity pin before a nonzero group signal
and risks signalling a reused group. The current implementation does send
SIGKILL before reap at agents/linux/testanyware_agent/exec_owner.py:470 and
agents/linux/testanyware_agent/exec_owner.py:490; this finding concerns the
control's inability to reject a regression in the central safety contract.
Observe the actual signal operations and reap in the control, covering both
leader-exit orders and prohibiting nonzero signals after reap.

## Required-control audit

This is a source/evidence audit, not a fresh execution result. Rows follow
docs/specs/guest-command-lifecycle.md, "Test seams and required controls".

| Spec guarantee | Reviewed evidence and disposition |
|---|---|
| Group ends after timeout | test_exec_lifecycle.py:112 checks real fork/wait child PID plus start ticks, death, groupEmpty, bounds and outer elapsed. The k217 matrix records the repaired survivor defect at all five seams. |
| Grace and SIGKILL bounds | test_exec_lifecycle.py:141 and :159 cover ignoring and cooperative leaders; F3 identifies the missing actual tombstone-order assertion. Report correctly does not invent an old Linux failure for the macOS old-fail row. |
| Leader status unavailable at K + 1.4 s | test_exec_lifecycle.py:187 substitutes HiddenExitWaiter rather than merely a SIGKILLable leader. Checks unavailable/no/false, elapsed bound, unreaped zombie, eventual reap and unchanged published JSON. |
| Large or stalled capture | test_exec_lifecycle.py:224 and :262 cover exact 32 MiB and a stalled five-byte prefix. F1 shows the uncovered publication work; :287 also misses F2's stat failure. |
| Exact bytes, exit 23, signal, spawn errno | test_exec_lifecycle.py:296–342 checks owner bytes/outcomes and real RLIMIT_NPROC EAGAIN. test_exec_compatibility.py:65 checks HTTP records; spawn uses the handler under the process limit. Recorded exec-record fixtures retain typed outcomes. The k5 matrix preserves old CLI receipts; typed CLI reading belongs to k218. |
| No finished command reported timed out | test_exec_lifecycle.py:356 suppresses exit notification but retains real kernel peek and requires exit 7 at the deadline. The specified free-1 and 20-run agent:macos:test control belongs to macOS k219, not this Linux producer. |
| Responsive agent during command | Threaded transport belongs to k220; excluded from this review's findings. |
| Disconnect changes nothing, retention | k220; excluded by this leaf's notes. |
| Cancellation, idempotence, tombstone | k220 and Host CLI control; excluded here. F3 concerns the existing timeout owner's tombstone order. |
| CLI interruption sends cancellation | Host CLI k222. |
| Download reads one version | k220/k221. |
| Download never sends extra bytes | k220. |
| Host CLI rejects extra/short bodies | Host CLI k222. |
| Temporary file removed at SIGTERM | Host CLI k222. |
| Record validity and skew | exec_validity.py validates records and test_exec_fixtures.py checks 14 new fixtures; F2 is a producer validity gap. Cross-platform reader/skew acceptance belongs to k218. The legacy oracle reuses production projection, so the independent recorded-old comparison is the stronger compatibility evidence. |
| Legacy projection keeps old values | test_exec_compatibility.py:65 compares frozen old replies; :83 checks old CLI receipts. The recorded k217 matrix also executes HTTP and checkout CLI seams and the report states every legacy receipt matches k5 run-03. |
| App file-cancel route | Host CLI/app k222. |

The committed k217 run-01/check.exit is 0. Its old-path detector deliberately
returns 201 with seven historical defects absent (five fork/wait survivors and
two skipped-SIGTERM traces). These are expected new-owner results, not seven
new failures. The matrix checks the legacy surfaces, not the whole new record.
The producer describes the pinned Linux suite task and locked fixture recording;
no fresh suite execution or suite transcript is claimed by this review.
