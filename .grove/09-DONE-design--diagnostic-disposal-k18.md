# diagnostic-disposal-k18

## Goal

Establish how the independent outer controller can preserve owned-resource
teardown when evidence storage itself stalls or fails.

## Context

Read F3 of review `diagnostic-foundation-k15` in commit `4bb47c05`, integration
`diagnostic-foundation-k16`, and `diagnostic-evidence-k17`. The integration
catches raised OSError per action; synchronous evidence open/read/write/hash
can still block the only thread responsible for disposal. Such a stall voids
its disposal ceiling. This is an architecture question, not permission to
raise a timeout or silently exclude a required acceptance property.
Termination integration k23 also identifies a distinct blocking path: Python's
subprocess.run timeout kills ps then waits without a bound. k17's bounded-inspection
continuation owns that design and reviewed implementation before k21. Coordinate
the ownership handoff with it; this leaf still owns stalled evidence storage.

The selected inspection contract is
`docs/specs/diagnostic-process-inspection.md`. Consume its immutable observation
and residual values from a bounded in-memory channel. Evidence publication
must not be an acknowledgement needed for workload signalling, reaping or
disposal. Preserve the controller's sole signal ownership and worker failure
semantics; account for evidence descriptors used by safety commands as part of
storage isolation. The spec is not proof that either continuation has shipped.

Integration bounded-inspection-k27 requires one controller scheduler to serve
all workload and teardown waits, channels and worker cleanup timers. Storage
isolation must leave those timers runnable and keep their existing deadlines.
No evidence publication acknowledges helper exec or permits safety progress.
Bootstrap and worker-close phases have separate declared invocation bounds.
Consume the final-query failures and known/unknown helper residuals unchanged.

Planner bounded-inspection-k25 assigns the concrete implementation to
bounded-inspection-k28, before retained-evidence-k21. Read its delivered source,
control receipts, specific review-impl and any reconciliation before choosing
the storage boundary. Preserve that scheduler's worker timers while teardown
commands are open, its sole workload signal/reap authority, fixed failure/close
deadlines and immutable observation/residual values. Missing real permission or
setuid-ps evidence remains unavailable. k28 does not isolate blocked evidence
I/O; that remains this leaf's design and reviewed implementation continuation.

## Done when

- Specify the smallest ownership and I/O separation that keeps independent
  disposal runnable, with frozen identity handoff, concrete grace/kill/reap
  bounds, failure reporting and explicit controller-crash limits.
- Define deterministic stalled-evidence and storage-error controls that show
  safety work still occurs within its ceiling. Clarify which timing guarantees
  remain conditional on OS scheduling or uninterruptible operations.
- Deliver the spec/ADR only where each earns its place. Cut a concrete planning
  or implementation successor before `diagnostic-routes-k19`, then insert this
  artifact's `review-design` before that successor as the final producer act.
  Its repair must land and be reviewed before live measurement relies on it.

## Notes

No live VM needed for this design. No installed app changes. Preserve all old
failed conditions and the historical 0/10 unavailable trial.

## Decisions (running log)

**Storage boundary.** One separate evidence writer process owns every evidence
file operation. The controller starts it as a fresh interpreter with an exec.
A writer thread is not possible, because fork admission requires exactly one
Python thread and one native thread. Storage in the inspection worker would
join two failure domains. Asynchronous file calls and longer timeouts cannot
interrupt a blocked call on the only thread.

**Controller filesystem rule.** From storage readiness until publication ends,
the controller thread makes no filesystem call. The controller never resolves,
creates or reads the evidence root. The writer resolves it. An audit-hook
control checks the rule.

**Command descriptors.** Every call writes its standard output and error to
pipes that the scheduler drains. The expected-output match uses the bytes in
memory, not a readback. The validator lexically refuses a teardown executable
that is relative or under the evidence root.

**One acknowledgement gate.** Only the intent record before each acquisition
waits for an acknowledgement, for at most two seconds. An intent file is the
only trace of an acquisition after a controller crash. A missed gate prevents
that acquisition and stops forward progress. Teardown never waits for storage.

**Bounds.** Readiness ends within five seconds of the spawn, and a failed
writer is reaped within two seconds. Publication starts after worker close and
has a window W of at most ten seconds. Subjects-after digests wait until
S + W/2. At P the writer receives KILL, and the reap poll ends at P plus two
seconds. The outbox holds at most 64 MiB, and a frame holds at most 1 MiB.

**Records.** The spec `docs/specs/diagnostic-evidence-storage.md` is the
artifact. No ADR is written, because the diagnostic controller is an
investigation harness and the choice is not hard to reverse. The inspection
spec stays unchanged. Its statements remain true, and an edit would first need
a partial prose conversion of more than 60 findings. The new spec links to it.

**Supporting documents.** The visual document is
`docs/design/diagnostic-evidence-storage/`, served by
`mise exec -- task design:storage-preview` on port 8766. The foundation report
gains the section "Evidence storage design (k18)" after a `tc: on` marker. The
historical sections stay pending.

**Successor kind.** The successor is a planning leaf, not an implementation
leaf. A design session that cuts implementation leaves does the work of
planning. The planning leaf slices the writer, pipe capture, channel and
controls into implementation leaves before k19. A review-design leaf precedes
it.
