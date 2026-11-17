# linux-timeout-lifecycle-k206

**Integrates:** linux-timeout-lifecycle-k205


## Goal

Integrate review `linux-timeout-lifecycle-k205` before
`lifecycle-evidence-k8` consumes the reviewed artifact.



## Context

Read the review's findings from its commit and verify each against the producer
artifact. Apply the integration acceptance and material-risk threshold; the
review's list is input for triage, not a predetermined repair charter.

This is a sibling in the existing review chain. Its immediate placement keeps
the reviewed source and evidence stable until triage and any necessary repairs
are complete, directly unblocking the downstream lifecycle evidence decision.

## Done when

- Each finding has a recorded disposition against the agreed acceptance.
- Accepted repairs and their appropriate verification are complete, or any
  necessary finite continuation is explicitly owned under the Grove rules.
- The downstream consumer has an accurate account of the resulting artifact and
  retained evidence.

## Notes

The root brief's diagnostic and evidence boundaries continue to apply.

## Decisions (running log)

**F1, the cleanup kill by PID (P2): real issue, repaired.** The source shows
the window in `linux_k5_seam.py:257` of the producer. The cleanup read the start
ticks once and then called `os.kill` by PID. Leaf k204 reuses this cleanup in a
live guest, so the risk is material. `kill_here` now opens a pidfd, reads the
start ticks again and signals through the pidfd.

Five host controls cover a
match, reuse before the open, reuse after the open, a same-ticks successor and
a refused pidfd. A run in the pinned image showed the real kernel path. A wrong
start time spared a `sleep` child, the right one killed it with signal 9, and a
reaped PID read as `gone`. The report now states the `run-03` window and the
same-tick limit of the identity.

**F2, partial source hashes (P2): real issue, repaired.** `plan()` hashed only
two agent modules before and after the run. `sources()` hashed the rest before
the run only, and nothing hashed `outcomes_k12_check.py`. One `source_paths()`
list now feeds both records. It holds the local import closure, which the code
computes with `ast`, plus the agent package and the stubs.

A control tests
that each recorded source is a plan subject. The `run-03` gap is permanent, so
the report states it under Provenance.

**F3, replay from the absolute run path (P2): real issue, repaired.** The
checker now reads the run directory inside `OUT` by the base name of the
recorded path. The absolute path stays as provenance only. A replay of a copy
of `run-03` wrote a `check.json` that is byte-identical to the retained one,
with 66 verdicts, no problems and no missing defects. The retained file kept
its hash.

**P3, report precision: real issue, repaired.** The run-03 reference phases
give TERM at 2.001 s and the end at 2.004 s for `deadline`, `issue-sleep` and
`issue-fork-wait`. The report now gives both times. The claim that the traced
seam costs microseconds had no measurement, so the report now says that the
leaf did not measure it.

**Verification.** `task diagnostic:linux-timeout-lifecycle-tools` passes with
ruff, format, pyright and 21 tests. Against the k5 source, seven new controls
fail and the other 14 pass. The kill controls fail on the old code because its
pidfd seams are absent.

The image run supplies the behavioural evidence for F1.
The retained `run-03` measured the pre-repair tooling. Leaf k204 is the first
run of the repaired tooling, so it is a new condition.
