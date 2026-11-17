# lifecycle-evidence-k215

**Integrates:** lifecycle-evidence-k214

## Goal

Reconcile review `lifecycle-evidence-k214` of design producer
`lifecycle-evidence-k8` so `lifecycle-delivery-k213` can plan from a coherent
contract and accurately classified evidence.


## Context

Read the findings from the commit for `lifecycle-evidence-k214`, including its
durable review document. That commit is the authority for the findings. This
task deliberately does not restate them. The producer's specification, ADRs,
glossary and diagrams are the reviewed artifact. Its task and the root brief
supply the acceptance requirements.

This is the next flat sibling after the review because planning must consume
settled design contracts before implementation slices are cut.

## Done when

- Verify and triage each finding against the reviewed artifact, current source,
  requirements and retained evidence. Record its class. A finding is a real
  issue, an unclear contract, a trade-off accepted against the agreed bar, or
  noise raised for want of context.
- Apply the justified design repairs and complete appropriate verification.
  Keep the specification, minimum coherent ADR set, glossary and diagrams
  consistent wherever a repaired contract appears. Preserve historical failed
  samples and the stated limits of causal evidence.
- Leave an explicit disposition and evidence for each finding, with a coherent
  planning input for `lifecycle-delivery-k213`. Preserve the root acceptance
  cases, byte oracles, bounds and frozen trial obligations.
- If a verified finding requires substantial redesign rather than local
  repair, commission the necessary producer/review chain ahead of planning
  under Grove's integration discipline. Do not introduce an unreviewed redesign
  or expand this task into implementation.

## Notes

Numbered ADRs are the repository's authorised format. Reconcile their content
without converting their format. Many consumers use only the Host CLI. Grade
its contract against that supported path. Keep the historic trial result
unchanged until new acceptance evidence exists.

## Decisions (running log)

**Method.** I read each finding in the k214 commit and its review document. I
compared each one with the spec, ADR-0023 to ADR-0026, the glossary and the
six views. I also read today's runners in
`agents/macos/Sources/TestAnywareAgent/ProcessRunner.swift` and
`agents/linux/testanyware_agent/system_endpoints.py`, and the Host CLI in
`cli-rs/crates/testanyware-cli/src/commands/file.rs`. For R6, I read
`docs/experiments/exec-reliability-macos-completion.md`.

**R1, blocking reap: real issue, repaired inline.** Step 4 reaped the leader
with a wait that had no bound. A leader in uninterruptible sleep would then
hold back the record and the agent shutdown. The owner now waits for the exit
status until K + 1.4 s and never blocks in a reap. A missing status gives
`leaderStatus` `unavailable` and `groupEmpty: no`. The owner keeps the leader,
reaps it later and sends no further signal.

The spec, ADR-0024 and `terminate.mmd` carry this repair. A new control needs
a leader whose status stays unavailable past the bound. A leader that ignores
SIGTERM and dies on SIGKILL does not qualify. Agent shutdown waits only until
each record is published.

**R2, unbounded capture read: real issue, repaired inline.** The capture read,
digest and encoding had no deadline. The bound from the outcome to the final
record therefore could not hold. A worker now reads each file from offset 0,
and the owner stops waiting 1.9 s after SIGKILL or after the outcome. An
unfinished stream is an incomplete prefix with its own length and digest and
the reason `deadline`. A control with a stalled capture reader covers it.

Output size stays unbounded. The exact-byte oracles for the declared samples
do not change. A stalled read holds one thread, and the spec states this
limit. The bounds are now 2 s from the outcome to the record, or 4 s with a
termination.

**R3, termination validity: unclear contract, repaired inline.** The validity
rules accepted `withinBounds: true` with `groupEmpty` `no` or `unknown`, and
after a late SIGKILL. A strict step order would also reject a leader that
exits only after SIGKILL. The spec now has a termination fields table and
cross-field rules that form a partial order. The agent derives `withinBounds`
from `groupEmpty`, `killSentAfterMs` and `finalisedAfterMs`.

The contradictory-termination control changes one claim and keeps every other
field valid. The controller accepts both leader orders and the unavailable
record.

**R4, legacy values: real issue, repaired inline.** The spec set the legacy
`exitCode` to -1 for each outcome that is not an exit. Today macOS reports the
signal number and Linux reports the negative signal. Text mode would also have
changed its bytes and exit codes. Many consumers read only text mode or the
existing JSON fields, so I chose the smallest additive policy.

Each agent now reproduces today's values as its named legacy projection,
`macos` or `linux`. The Host CLI keeps text mode, its existing JSON fields and
its exit codes. The exact-byte text mode, the notice line and the 128 + N exit
table of the producer are withdrawn. JSON mode carries the outcome and the
exact bytes, and the acceptance controller reads JSON mode.

Three departures remain, and each one removes a defect. A finished command is
no longer reported as timed out. A macOS spawn error gives a record, not an
HTTP error. A cancellation is a new outcome. `schema_version` stays `1.0`.

**R5, lifecycle evidence missing from the CLI: real issue, repaired inline.**
The Host CLI JSON contract left out `lifecycle`. The public path could not
show owner lateness or the time to the record. JSON mode now adds `lifecycle`,
with null for a legacy record, and the record adds `finalisedAfterMs`. The
bounds table names the record field or the controller measurement for each
bound.

**R6, starvation claim: real issue, repaired inline.** The evidence table and
ADR-0024 implied that a probe had found pool starvation as a cause. The k3
report shows a receipt from the old runner, 10 of 10, with no free pool
worker. Its single-hop control removed both the second wait block and the
Foundation exit wait, so it does not locate the stalled phase. The spec and
ADR-0024 now state the receipt as established in a host probe only, and the
stalled phase as unresolved.

The runner still removes both candidate mechanisms. No design choice depends
on which one stalled.

**Review handoff.** The download framing control must serve raw HTTP bytes to
the real client. The controls table now states this for candidate D. The
review gave it as a verification constraint, not a finding.

**Glossary and views.** `CONTEXT.md` adds **Legacy projection**.
`terminate.mmd`, `read-record.mmd`, `lifecycle.mmd`, `diagrams.json` and the
design README carry the repaired rules. ADR-0025 and ADR-0026 need no change.
No ADR is added or removed.

**No redesign or new review round.** Each repair narrows a rule inside the
reviewed architecture. One owner, explicit cancellation, the typed record and
the single-handle download stay. No repair adds an owner, a wire version or a
seam outside the agreed three. The two new test seams sit at the owner's
leader-wait source and capture reader, inside the process and file seam.

**Planning input for `lifecycle-delivery-k213`.** The spec sections
"Implementation candidates", "Test seams and required controls", "Bounds" and
"Validation route" are current. Three controls are new: the unavailable leader
status, the large or stalled capture and the legacy projection. Candidates B
and C each own the legacy projection of their platform. The four corruption
controls, the byte oracles and every root case stay. An opt-in exact-byte text
mode is possible later work, not an obligation.

**Verification.** The spec, ADR-0023, ADR-0024, the glossary and the design
README pass the prose checker with no hard finding. The local preview rendered
all six diagrams with no Mermaid error. No code changed, and no control ran.
The historical result of 0 of 10 stays unchanged.
