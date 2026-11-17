# linux-timeout-lifecycle-k205

**Reviews:** linux-timeout-lifecycle-k5

## Goal

Adversarially review the k5 Linux timeout lifecycle tooling, the checker and the
report claims, before `lifecycle-evidence-k8` relies on them.

## Context

- Read the producer commit for `linux-timeout-lifecycle-k5`, its decision log
  and `docs/experiments/exec-reliability-linux-timeout-lifecycle.md`.
- The tooling is `experiments/exec-reliability/linux_k5.py`,
  `linux_k5_seam.py`, `linux_k5_check.py`, `test_linux_k5.py` and the stubs in
  `linux_k5_stub`.
- The Taskfile entries are
  `diagnostic:linux-timeout-lifecycle{,-check,-old-path,-tools}`.
- The evidence is `run-01`, `run-02` and `run-03` under
  `experiments/exec-reliability/evidence/linux-timeout-lifecycle-k5`. Only
  `run-03` is complete.

## Done when

- The ownership rule is read for any way that it can kill a process that the
  case did not create. The read covers PID reuse, the observer process and the
  `kill` and `limit` modes.
- The faithful bound and the observation times are checked against the
  host clock and the container clock.
- The stubs are checked to leave the exec path of the agent unchanged.
- The `RLIMIT_NPROC` spawn failure is checked for a case where the limit fails
  some other process instead of the handler spawn.
- Each frozen defect in `REQUIRED` is checked in both checker modes.
- Each claim in the report is traced to a byte or a time in `run-03`.
- Each finding has a severity, a file-and-line anchor and a concrete failure.

## Notes
