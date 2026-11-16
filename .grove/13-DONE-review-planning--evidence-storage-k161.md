# evidence-storage-k161

**Reviews:** evidence-storage-k158

## Goal

Review the plan that slices the evidence storage design into impl leaves.

## Context

The plan is the node `evidence-writer-k162`, now `.grove/15-k162/`. Read its
brief and its six task files.
The design is `docs/specs/diagnostic-evidence-storage.md`.

## Done when

- Each spec rule that needs code belongs to exactly one slice. Consider the
  channel contract, the phase bounds, the descriptor rules, the terminal
  outcome and the completion authority.
- Each slice leaves the suite green and `execute()` compatible with its callers.
- Each of the thirteen controls belongs to the slice that makes it pass.
- Each finding is in this file with the evidence for it.

## Notes

Specific doubts:

- The controls slice must fail each control on a build that has no writer and
  no writer command parameter. Check that each control can fail for the right
  reason on that build.
- The cutover slice runs with a writer whose close wait is not yet bounded.
  Check that this intermediate state cannot hang the suite.
- The completion authority changes in the writer slice, before `execute()`
  produces a close record. Check that `retention.py` and its tests stay green
  for run directories that `execute()` writes before the cutover.

## Review findings

Reviewed producer commit `e9e40c931597fca0a3e9fdb7aa2b42f968342643`.
The coordinates below describe that commit, before insertion renumbers the
node. This was an inspection-only review; no tests, builds, lint or format
commands ran.

### F1 — high: completion authority switches before its producer

`storage-writer-k164` requires missing `storage-close` to make a run
incomplete, while leaving `execute()` unchanged and the full suite green
(`.grove/14-k162/02-impl--storage-writer-k164.md:31-39`). The current
`execute(plan: Plan, evidence_root: Path) -> Path` writes the result followed
by `write_identity(root)`, with no close record
(`experiments/exec-reliability/controller.py:979-998`). The existing healthy
run test obtains its directory through that entry point and requires
`retention().verify(path)["ok"] is True`
(`experiments/exec-reliability/test_retention.py:37-59`).

Those requirements cannot all hold at k164. The horizon delegates legacy-run
handling to that leaf, but the node also requires existing tests to run without
change (`.grove/14-k162/_evidence-writer.md:29-32,74-78`). Accepting a missing
close as complete would contradict the new completion contract; weakening the
healthy-run assertion would lose existing coverage. The plan needs an explicit
compatible transition that keeps this live producer/verifier pair working at
each boundary. This is acceptance work, not merely a decision about archived
directories.

### F2 — high: the intent-stall control must pass before close is bounded

The cutover removes the intent-gate expected-failure marker and requires that
control to pass, yet explicitly permits a stalled writer to hold the close
wait until k168 (`.grove/14-k162/04-impl--storage-cutover-k166.md:27-34`).
The spec's control stalls the writer on intent-2 while checking that the first
resource's teardown is unchanged
(`docs/specs/diagnostic-evidence-storage.md:389-390`). After the gate's
two-second limit, teardown can finish but this writer still cannot acknowledge
close. A watchdog in k163 bounds the test, not the controller: expiry leaves
this now-unmarked control failing. Resuming the writer just to make the
intermediate test return would no longer establish the persistent-stall case.
The same dependency affects the overflow control's stalled writer.

The cutover's claimed green boundary therefore depends on bounded publication
and writer shutdown currently assigned to k168. The plan must settle that
dependency before the stall controls are required to pass.

### F3 — medium: the output-overflow control precedes output transport

The required overflow control stalls the writer while a fixture writes output
and checks drops by record name
(`docs/specs/diagnostic-evidence-storage.md:383-385`). Its marker must come
off in k166 (`.grove/14-k162/01-impl--storage-controls-k163.md:43-49` and
`.grove/14-k162/04-impl--storage-cutover-k166.md:27-28`). But k166 explicitly
keeps call streams in files; the turns that send output bytes through the
outbox arrive in k167
(`.grove/14-k162/04-impl--storage-cutover-k166.md:10-13` and
`.grove/14-k162/05-impl--pipe-capture-k167.md:16-20`). Today's `_call()`
passes evidence file descriptors directly to the child
(`experiments/exec-reliability/controller.py:779-792`).

At the assigned boundary, fixture output cannot fill the outbox or produce
stream-name drop counts. Overflowing plan or intent metadata would test a
different cause. This control needs an owner whose delivered output path can
actually exercise it, in addition to the publication dependency in F2.

### F4 — medium: a missing writer is allowed to count as fault evidence

The first slice says a writer-specific control fails because no writer exists
and instructs the implementer to record that as its cause
(`.grove/14-k162/01-impl--storage-controls-k163.md:15-19`). The spec instead
requires pre-repair controls to stall or fail the existing storage functions
through current seams (`docs/specs/diagnostic-evidence-storage.md:368-369`).
The writer-death control is supposed to exercise loss during P2 and establish
unchanged teardown (`docs/specs/diagnostic-evidence-storage.md:381-382`). An
absent writer or missing command parameter fails before that fault is injected;
it provides no evidence that the teardown assertion detects the old storage
failure. A general strict expected-failure marker can also keep accepting that
setup failure after the intended storage behaviour is repaired.

The baseline plan needs to distinguish reaching the injected storage fault from
failing to construct the test. Each recorded pre-repair failure must demonstrate
the required behavioural gap; absence of the new interface is insufficient.

## Scope and handoff

The six leaves cover the main spec areas and preserve the `Path` return
contract. The findings concern intermediate acceptance and fault evidence;
they do not call for a different storage architecture. Integrate this review
before implementation, retaining all thirteen controls and their required
negative evidence. The integration owns triage and any plan changes.

Graph coverage was checked at generation `2026-11-15T02:19:52Z`. Controller
and test metadata had changed; retention and the planning documents were not
tracked by that generation. The material claims above therefore use direct
reads of the current source and documents, not an assertion of graph
completeness.
