> Historical report. Paths and commands refer to the [archived checkout](../../README.md).

# k108 shutdown review integration

All three k108 findings were accepted and repaired within the declared worker
shutdown slice. The final frozen run passes 712 tests, Ruff, format and pyright.
The shared scheduler and aggregate acceptance remain with
`complete-worker-scheduler-k107`; k28 and k17 remain open.

## Findings and changes

**F1 — binding earlier deadlines.** The new real-child `reserve` control leaves
300 ms to a finite cutoff with a 160 ms reap allowance and 250 ms TERM grace.
The reserve therefore determines KILL time before fresh grace would expire,
without clamping KILL to shutdown start. It checks the absolute value with
`rel=0`. The `aged-term` control sends a real TERM, ages its recorded attempt by
at least 30 ms, and leaves enough cutoff slack for the earlier attempt to
determine KILL time. Each mode runs three times. Removing the reserve, ignoring
the prior TERM, and renewing grace after that TERM each fail the corresponding
three controls.

**F2 — strict phase timing.** Each phase now records `timing_outcome` as
`on_time`, `at_deadline`, or `late`, separately from its operational `outcome`.
The specification remains unchanged: positive measured lateness is an overrun;
equality exhausts the deadline without being a strict overrun. A measured late
grace phase now also makes worker failure sticky, even when KILL/reap finishes
inside the final shutdown bound. A prior KILL still does not restart grace.

The exact-clock control demonstrates early TERM completion at 100.03,
escalation exactly at the 100.07 cutoff, and late escalation at 100.073.
It checks the recorded finish against those independent expected times, the
timing classification, and failure propagation. Mutants that hide the late
classification or suppress grace-overrun failure are rejected.

This is deliberately strict, and carries a practical limitation: ordinary real
escalations commonly execute just after the cutoff because this synchronous
Python loop has no hard real-time guarantee. They remain `late` and now carry
failure, rather than receiving an undocumented tolerance. The full real-child
matrix records 30 late grace phases, six on-time completions, and six equality
receipts for modes with prior KILL. Equality for a phase that was not restarted
is not evidence of a new precisely timed signal. Passing the checker means the
expected failure was correctly reported; it does not mean those late shutdowns
succeeded within every phase bound.

**F3 — repetition and timing sensitivity.** The 14 real-child modes each run
three times, producing 42 independent exact-child reaping audits per complete
matrix. Exact-clock controls supplement those real signals with a 3 ms grace
offset and a 3 ms KILL delay. Both mutants are rejected. The seam owns no OS
child and does not claim to prove 3 ms wall-clock precision. Real samples retain
their scheduling allowance; the exact-clock checks establish sensitivity to
the narrower arithmetic and scheduling regressions.

## Retained verification

The [final verdict](full-final-3/verdict.json) names every frozen subject and
retains before/after hashes, command bounds, checker PID/PGID and reap status.
Its [JUnit record](full-final-3/pytest.xml) contains 712 passing cases, with zero
failures, errors or skips. Ruff, format and pyright each exit zero. Runtime,
tests, manifest, lockfile, Taskfile, specification, evidence tooling, interpreter
and invoked system binaries are covered by the subject hashes. The final frozen
Python inputs match the delivered source byte for byte.

| Mutant | Retained result | Expected failures |
| --- | --- | --- |
| Remove finite reap reserve | [verdict](mutant-drop-reserve/verdict.json) | 3 reserve samples / 42 cases |
| Ignore earlier TERM | [verdict](mutant-ignore-prior-term/verdict.json) | 3 aged-TERM samples / 42 cases |
| Grant fresh TERM grace | [verdict](mutant-fresh-grace/verdict.json) | 3 aged-TERM samples / 42 cases |
| Add 3 ms to TERM grace | [verdict](mutant-grace-offset/verdict.json) | 3 / 3 clock cases |
| Delay KILL by 3 ms | [verdict](mutant-kill-offset-2/verdict.json) | 2 / 3 clock cases |
| Hide grace timing result | [verdict](mutant-hide-grace-late/verdict.json) | 2 / 3 clock cases |
| Suppress grace failure | [verdict](mutant-hide-grace-failure/verdict.json) | 1 / 3 clock cases |

Each mutant is a single guarded source replacement. Pytest exit 1 is the
expected rejection; none timed out, and every checker was reaped. Every recorded
real-child audit confirms exact-child reaping, including rejected mutant runs.
The audits retain the runtime receipts, signal calls and wait probes. No
unowned process was signalled by these controls.

The [red run](red/verdict.json) rejected the missing timing field in two cases;
the [green run](green/verdict.json) passed 85 selected cases. Adding the early
clock case brought the full suite to 712. The first [full run](full-final/verdict.json)
passed all tests and static checks. [Full replay 2](full-final-2/verdict.json)
also passed all tests, but rejected two overlong lines in the newly revised
mutation preparer; format and pyright passed. After splitting those string
literals without changing their values, full replay 3 passed every check.
All three full runs have identical per-case identities and outcomes, not merely
matching totals. Their runtime inputs are identical.

An earlier `kill-offset` preparation stopped with
`ValueError: mutation kill-offset: expected exactly one source site` because its
pattern also matched another loop. The incomplete [copied input](inputs-kill-offset/)
is retained; it has no preparation receipt and is not a measured mutant. The
replacement now includes the unique KILL-branch condition, and the successful
preparation and rejection use the `-2` paths. This preparation error and the
failed Ruff replay are retained rather than represented as passing evidence.

Every completed run has equal before/after subject hashes. Artifact hashes in
all twelve verdicts were independently recomputed after the runs. The
[preservation result](prior-after.json) compares the exact prior inventory:
157,506 paths, no additions, removals or changed bytes. All evidence predating
k109 remains intact.

## Reproduction and remaining scope

Use `mise exec -- task diagnostic:shutdown-review-prepare INPUTS=. OUT=...` to
freeze inputs, adding `MUTANT=<name>` for a guarded regression. Then use
`diagnostic:shutdown-review-check INPUTS=... OUT=... MODE=full` or
`MODE=controls SELECT=...`. Paths are relative to `experiments/exec-reliability`;
outputs must be fresh. The checker independently bounds pytest to 120 seconds
for a full run or 60 for controls, static commands to 30 seconds, and timeout
cleanup to two seconds TERM plus two seconds KILL/reap. No timeout occurred.

This reconciliation does not deliver shared earliest-deadline scheduling,
controller-teardown overlap, a first reap after an expired cutoff, or aggregate
guest acceptance. k107 retains those requirements, the earlier timing-sensitive
suite history, and the final bounded-inspection review. The evidence here is
local controlled-child verification; ten-clone live-guest acceptance remains
unavailable and unclaimed.
