# macos-disconnect-availability-k202

**Reviews:** macos-disconnect-availability-k13

## Goal

Adversarially review the k13 disconnect diagnostics, the checker and the report
claims, before `lifecycle-evidence-k8` relies on them.

## Context

- Read the producer commit for `macos-disconnect-availability-k13`, its decision
  log and `docs/experiments/exec-reliability-macos-disconnect-availability.md`.
- The tooling is `experiments/exec-reliability/disconnect_k13.py`,
  `disconnect_k13_burst.py`, `disconnect_k13_host.py`, `disconnect_k13_seam.py`,
  `disconnect_k13_check.py`, `disconnect_k13_run.sh` and `test_disconnect_k13.py`.
- The Taskfile entries are
  `diagnostic:macos-disconnect-availability{,-host,-check,-tools}`.
- The evidence is `host-01` and `live-01` under
  `experiments/exec-reliability/evidence/macos-disconnect-availability-k13`.
- The checker changed after `host-01` ran. The first judgement stays in
  `host-01/check-01.json`.

## Done when

- The responsiveness oracle is read for any way that it can call a blocked
  request responsive. The read covers the reference chain and the span rule.
- The inspection script is read for any way that it can name the wrong shell
  or child. The read covers PID reuse and the `shell.of.<pid>` record.
- The host seam sweep and the host audit are checked for kills of processes
  that this run does not own.
- The loopback agent copy is checked against the shipped agent handler sources.
- The `completed-reported-timeout` class is checked in both checker modes.
- Each claim in the report is traced to a byte or a time in `host-01` or
  `live-01`.
- Each finding has a severity, a file-and-line anchor and a concrete failure.

## Notes

Do not rerun a diagnostic in place of `host-01` or `live-01`. A new run goes into
a new directory. The root boundaries apply: no extra clone, no default socket, no
installed app and no `pkill`.
