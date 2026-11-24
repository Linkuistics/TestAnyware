> Historical report. Paths and commands refer to the [archived checkout](../../README.md).

# Shared driver review integration (k128)

The k127 review's F1 is a real measurement defect. `execute()` now implicitly
hashes `turns.py` alongside the controller, inspection and worker capture modules.
F2 is a visible limitation for singleton safety flags, with an additional
mutation control for the observable initial-due clock bypass.

## F1: implicit runtime dependency

`test_implicit_turn_driver_drift_changes_subjects_after` copies the four runtime
modules into a temporary directory and points the controller's module location
there. A real peer changes the copied `turns.py`; the plan never explicitly names
that file as a subject. The test checks the independent before and changed-byte
SHA256 digests and requires the run to fail because its inputs drifted.

Before the fix, [red/pytest.stdout](red/pytest.stdout) fails on the missing hash
key. The retained run receipt confirms that the peer changed the file while
the controller incorrectly reported success. After the one-line subject fix,
the same control passes and the drift receipt correctly reports failure.
Removing that subject again in `subject-mutant-inputs` reproduces the failure.
Only temporary copied subjects are changed; the measured runtime stays frozen.

## F2: observable policy and singleton limitation

The four compatibility consumers each register one turn. Flipping a consumer's
`safety` flag cannot alter that singleton's order, so no per-consumer flag
mutation is credited as evidence. The existing shared-driver ordering tests
exercise competing ready owners. k126 must verify the production registrations
with competing owners when it connects the actual scheduler; its charter now
points to this limitation.

The initial-due clock bypass does have an observable contract. The added
`test_shared_turns_initial_due_owners_do_not_consume_clock` lets three initially
due owners terminate while failing any clock read. It passes unchanged code and
rejects the `initial-clock-read` mutant with the intended clock-consumption
failure. This guards the initial callback behavior independently of wall time.

## Verification and retained evidence

| Condition | Result |
| --- | --- |
| Original missing-subject RED | 1 expected failure, no errors |
| Fixed focused controls | 9 passed |
| Remove implicit subject mutant | 1 expected failure, no errors |
| Remove initial clock bypass mutant | 1 expected failure, no errors |
| Full frozen suite | 875 passed |
| Ruff, format, strict Pyright | all passed |

[integration-audit.json](integration-audit.json) compares individual test identities
and outcomes with k124 `candidate-full-2`: all 873 prior items retain their passing
outcomes, no items disappear, and the two added controls pass. It also verifies
every artifact digest recorded by the five condition verdicts. Every checker
finished within its unchanged bound, was reaped, and passed exact fixture audits.

The unchanged k124 runner owns the bounded checks: controls 60 seconds, full
pytest 120 seconds, static tools 30 seconds, then TERM 2 seconds and KILL/reap
2 seconds on timeout. Each condition has exclusive output, declaration, JUnit,
stdout/stderr, per-run receipts and before/after hashes. The subjects include
all frozen Python sources (including copies of the new preparation/preservation
scripts), manifest, lockfile, Taskfile, the runner and its k124 planning/tooling
inputs, the process-inspection spec, interpreter and named system executables.
Those hashes match before and after every condition, and final runtime/tooling
sources and Taskfile match `green-inputs` byte for byte.

[prior-after.json](prior-after.json) verifies all **204,212** earlier evidence
paths unchanged, including all k124 samples, mutants, receipts and earlier
failures. Every new artifact is under this directory; none replaces old evidence.

Repeat preparation and preservation through `diagnostic:driver-review-prepare`
and `diagnostic:driver-review-preserve`; check new tooling through
`diagnostic:driver-review-tools`. Run frozen checks with the existing
`diagnostic:common-driver-check`, using a new OUT each time. Preparation accepts
`MUTANT=no-turns-subject` or `MUTANT=initial-clock-read` for the two rejected
regressions. The successful focused selection is
`implicit_turn_driver_drift or shared_turns`.

This slice establishes input-freeze coverage and the named policy controls.
Production shared scheduling, overlapping teardown and aggregate acceptance
remain open under k126/k28/k17. Guest acceptance remains 0/10 unavailable;
these local fake-peer controls do not change it. No guests, privilege,
installed changes or unrelated processes were used.
