> Historical report. Paths and commands refer to the [archived checkout](../../README.md).

# Late helper identity retention — k71

The interim per-query worker now retains valid helper identities received after
the query cutoff as cleanup evidence. The query remains failed. This closes the
late-publication slice of complete-worker-scheduler-k71; it does not establish a
persistent worker or the shared scheduler. Complete-worker-scheduler-k75 owns the
full remaining k28 contract and its aggregate review.

## Behavior and boundary

Previously, `_created` required the observation itself to precede the query
deadline, and shutdown never consumed the control pipe. An identity already
read during a delayed parent read, queued when the parent resumed after select,
or published during shutdown could therefore be lost. Observing worker exit did
not make that identity available either.

`worker_capture.py` now validates creation against the original dispatch and
query deadline independently of observation time. One parser handles normal
controls and cleanup controls. It preserves the exact token, positive PID,
worker parent/group, schema and creation-time checks. Every distinct valid
helper identity retains a separate obligation; duplicate/conflicting controls
continue to invalidate the observation.

After failure, shutdown parses buffered controls even if the worker was already
reaped. It can make one nonblocking control read per turn under the original
shutdown deadline and cumulative 4096-byte cap, with one byte allowed only to
detect overflow. The per-turn read is also below the existing 64KiB bound.
Worker KILL/reap work precedes the read. It never waits for EOF past the original
deadline or renews the reserve.

Late readiness permits parsing but does not widen worker signal scope. Late
terminal/reap messages are never decoded, late fork-error messages never grant
no-child authority, and late EOF never changes the timely channel-completion
record. The helper ledger remains evidence only: this slice adds no helper
signals and does not infer helper death from worker death or worker group
signals. Unpublished interrupted creation remains unknown.

## Frozen verification

Run through the existing Taskfile using `mise exec -- task`:

| Condition | Cases | Outcome | Lint / format / strict types |
| --- | ---: | --- | --- |
| [k71-red-01](../k71-red-01/verdict.json) | 335 | Six expected failures, 329 passes | All passed |
| [k71-green-01](../k71-green-01/verdict.json) | 342 | All passed | All passed |

The six RED failures are exactly the valid and distinct-identity publications
for each of `read`, `select` and `shutdown`. Each failed because the expected
identity set was absent. The independent audit compares every named testcase:
those six changed to passing, every other RED testcase stayed passing, and
seven further invalid-field controls were added and passed. Every prior 323
testcase was therefore exercised in both retained full-suite conditions.

The 19 new controls use real workers, helpers and pipes. Parent read/select
adapters delay observation across the cutoff. The shutdown adapter holds only
the publication, lets the real helper execute and be reaped, and delivers its
identity and terminal bytes through the actual pipes after worker TERM.
Positive cases retain their identity obligations despite the late terminal
reap. Negative cases cover wrong parent, token, Boolean PID, stale and
deadline-equal creation times, mismatched deadline, extra fields, incomplete
line and control overflow. Distinct-identity controls add the explicitly
synthetic PID 987654322; no process with that identity is created or signalled.
Select controls also check that late readiness leaves PID-only signal scope.

The runtime/test/config sources used for each condition are retained in
[k71-inputs-red-01](../k71-inputs-red-01) and
[k71-inputs-green-01](../k71-inputs-green-01), preserving repository-relative
paths. The self-check freezes all root diagnostic Python files, the project
manifest/lock, Taskfile, selected Python executable and `/bin/ps`, and records
before/after hashes, checker process receipts, raw streams, JUnit and artifact
hashes. The GREEN pytest run completed in 44.00 seconds within its 60-second
budget; none of the four checkers timed out and all were reaped. No measured
runtime subject changed during either run.

## Independent fixture and evidence audit

The frozen [auditor](../k71-audit-01/audit.py) runs through
`diagnostic:resource-audit`; its [verdict](../k71-audit-01/verdict.json) records:

- 2,441 declared subjects with matching before/after digests, including both
  retained conditions, both source snapshots, the auditor and selected tools.
- All 71,616 prior evidence files match the manifest frozen before this slice
  in [k71-inputs-01](../k71-inputs-01/prior-evidence.json), checked both before
  and after the audit.
- All 19 late-publication receipts independently checked against their helper
  witnesses, failure state, retained identity sets and original deadlines.
- All 1,273 retained positive real fixture PID selectors absent from a separate
  `/bin/ps` query, with an empty stdout/stderr and exit 1. Synthetic and
  nonpositive selectors are excluded explicitly. Live, absent, mixed and invalid
  selector controls first validate this oracle with outcomes
  `[false, true, false, false]`.
- 623 recorded/sibling test scratch directories checked. Two remaining mock
  marker files were removed only after exact fixture process absence, after
  matching their retained acquisition plan, run identity and bytes `owned`.
  Their before hashes and absent-after result are retained in the verdict.

This later exact-PID observation establishes absence at audit time only. It
does not acknowledge historical helper reap, resolve unpublished unknown
creation, or authenticate a helper in the production runtime. The six earlier
failed captures and all their evidence remain retained. No privilege, guest,
installed-app change or unrelated process signalling was used.

## Remaining work and review

k28 and k17 remain open. k75 retains persistent pre-acquisition startup, full
identity epochs, the immutable shared scheduler, every outstanding real/control
requirement and the final `Reviews: bounded-inspection-k28` review before k21.
The separate review of this k71 slice must examine parser authority, retained
identity ownership and the bounded shutdown read path before k75 relies on it.
Ten-clone acceptance remains 0/10 unavailable; storage isolation remains k18.
