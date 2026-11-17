# macos-exec-outcomes-k200

**Reviews:** macos-exec-outcomes-k12

## Goal

Adversarially review the k12 exec outcome matrix, its seams, its checker and the
report claims, before `lifecycle-evidence-k8` relies on them.

## Context

- Read the producer commit for `macos-exec-outcomes-k12`, its decision log and
  `docs/experiments/exec-reliability-macos-exec-outcomes.md`.
- The tooling is `experiments/exec-reliability/outcomes_k12.py`,
  `outcomes_k12_seam.py`, `outcomes_k12_check.py`, `test_outcomes_k12.py` and
  `evidence/macos-exec-outcomes-k12/probe/main.swift`.
- The Taskfile entries are
  `diagnostic:macos-exec-outcomes{,-check,-old-path,-tools}`.
- The evidence is `run-01` under
  `experiments/exec-reliability/evidence/macos-exec-outcomes-k12`.

## Done when

- The reference seam is read for any way that it can report an outcome
  wrongly. The read covers the wait status, the group TERM and KILL and the
  temporary-file capture.
- The loopback agent copy is checked against the shipped agent. The check covers
  the byte-identity claim and any behaviour that the new `main.swift` changes.
- The checker is read for a defect class that it can miss or assign wrongly. The
  read covers the `REQUIRED` alternatives and the CLI text mode, which has no
  timeout flag.
- Each claim in the report is traced to a byte or a time in `run-01`.
- Each finding has a severity, a file-and-line anchor and a concrete failure.

## Notes

Do not rerun the matrix in place of `run-01`. A new run goes into a new
directory. The root boundaries apply: no clone, no default socket, no installed
app and no `pkill`.

## Findings

### 1. P1 — Cleanup treats a reusable command string as process ownership

Anchors: `experiments/exec-reliability/outcomes_k12_seam.py:80`,
`experiments/exec-reliability/outcomes_k12_seam.py:196`, and
`experiments/exec-reliability/outcomes_k12.py:179`.

`deadline_command()` gives every invocation the same twelve sleep commands,
such as `sleep 30.11`. `sweep()` enumerates all processes of the current user
and sends SIGKILL to every exact command match. The final `audit()` similarly
kills any command matching the fixed `TOKEN` expression, independently of its
relationship to this run. Although both record the process start time, neither
checks it against an identity established when this run created the process.

A separate same-user `sleep 30.11` that survives until the reference deadline's
settle/sweep is killed. A concurrent matrix invocation has the same collision.
The final audit also kills matching processes belonging to other seams or runs.
Unique run directories and the controller's empty resource ledger do not confer
ownership of these PIDs. This violates the root brief's positively identified,
run-owned cleanup boundary and can terminate unrelated host work. The recorded
run's empty residual lists do not exercise this failure.

### 2. P2 — Missing deadline evidence can pass the faithful-reference gate

Anchors: `experiments/exec-reliability/outcomes_k12_check.py:133`,
`experiments/exec-reliability/outcomes_k12_check.py:240`, and
`experiments/exec-reliability/outcomes_k12_check.py:330`.

For a concrete damaged record, remove `deadline_ns`, `term_ns`,
`grace_expired_ns`, `kill_ns`, and `reap_ns` from the otherwise unchanged
reference `deadline-term-ignored` receipt. `reference_phases()` returns `{}`;
`phase_defects()` defaults missing phases to zero. Its timeout and signal status
still normalize to `deadline-signal`, which `status_defects()` accepts, and
`soundness()` has no independent requirement for those marks. With the other
receipts unchanged, the faithful-reference gate therefore has no problem to
report. The checker can accept evidence that proves neither TERM grace nor
KILL/reap completion.

The same status branch also accepts any signal number for a reference deadline:
it returns before checking `code`. For example, signal 15 in the TERM-ignoring
case is accepted even though that frozen command requires KILL. These are
source-derived acceptance failures, not executed mutations. The committed
run-01 records do contain the expected marks and signals; the issue is the
gate's ability to reject incomplete or contradictory future evidence before
`lifecycle-evidence-k8` relies on it.

### 3. P2 — CLI measurements use a raised comparison ceiling

Anchor: `experiments/exec-reliability/outcomes_k12_seam.py:510`.

`cli_case()` sets its outer bound to `case.timeout + CLI_MARGIN + HOST_MARGIN`,
giving a two-second deadline case 32 seconds and an ordinary ten-second case
40 seconds. The root brief fixes raw host exec at guest budget plus 20 seconds
and ordinary observation at 30 seconds; it explicitly prohibits raised
comparison ceilings. The CLI's internal HTTP allowance does not change that
external contract.

An ordinary CLI call returning after 35 seconds is recorded with
`bound_exceeded = False`. `judge()` forwards that flag and `soundness()` checks
the flag rather than comparing elapsed time with the agreed ceiling, so this
stall can pass the sound-evidence mode. The committed inventory and CLI
receipts confirm the 40/32-second bounds. Actual run-01 elapsed times remain
below the agreed ceilings; this finding concerns what the instrument accepts.

## Inspection evidence and limits

- Reviewed producer commit `093a1036bb5ebdf8d09632f1a79fd96c0e5052c6`,
  the five named implementation/test/probe artifacts, the Taskfile entries,
  the report, the relevant shipped agent sources, and recorded run-01 evidence.
  No test, build, lint, format, checker, or matrix command was run. No production
  or test code was changed.
- The raw reference receipts distinguish raw exit 23 (5888), raw exit 9 (2304),
  and signal 9 (9), with exact tempfile captures. Reference TERM-to-reap is
  about 2.5 ms; the ignored-TERM grace is 2.000024 s and KILL-to-reap about
  0.467 ms. The traced old runner's grace is 3.003215 s, supporting the report.
- Inspected all 48 table rows against the recorded seam receipts. Payload bytes
  and rounded elapsed times agree. The CLI text spawn-error table uses the
  checker's empty guest-stderr projection; its raw host stderr retains the
  HTTP-500 diagnostic. The three CLI JSON timeout receipts and their roughly
  ten-second durations support the report's stated observation and causal limits.
- All sixteen retained-file identities match their recorded sizes and SHA-256
  digests. The controller's 24 subject digests agree before and after. Reported
  source identities match current files. Recorded controller calls return zero,
  all three servers stop by TERM, and temporary directories are removed.
- The copied main binds loopback and omits notification scheduling. The router
  and runner byte-identity claim is supported by source and retained hashes.
  The checker deliberately accepts documented race alternatives; CLI text mode
  exposes no timeout flag. Those facts are not additional findings.
- Tier 2 graph discovery was followed by coverage checks. The generation was
  `2026-11-15T02:19:52Z`; several relevant files were changed or untracked by
  that index. Direct current-source reads supplied the evidence for findings.
  This is bounded inspection, with no new experimental measurement.

## Decisions (running log)

- The cleanup ownership failure and the two false-acceptance paths meet the
  acceptance/material-risk threshold. Cut an adjacent `integrate-review-impl`
  task to triage this review before later lifecycle evidence consumes k12.
  The existing recorded run is retained without modification.
