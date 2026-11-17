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

Findings and evidence reconciliation:
`docs/experiments/exec-reliability-macos-disconnect-review-k202.md`.

Do not rerun a diagnostic in place of `host-01` or `live-01`. A new run goes into
a new directory. The root boundaries apply: no extra clone, no default socket, no
installed app and no `pkill`.

## Decisions (running log)

- Source inspection establishes an actionable host-oracle finding: `run_case`
  suspends process polling during each synchronous follow-up, then stamps shell
  disappearance after the answer. The checker can treat a request held until
  shell exit as responsive, especially when the concurrent reference is held
  the same way. Record the finding for integration; do not rerun diagnostics or
  change the instrument in this review.
- The host sweep and build audit establish another actionable finding: matching
  a command substring and rechecking its start time proves stable identity,
  not ownership. An unrelated observer of the private build directory can be
  killed. The owned-process boundary in the root brief applies here.
- The live PID inspection also has an actionable identity gap: `shell.of.<pid>`
  and `child.pid` carry no process generation. Reuse can turn an unrelated
  parent/child pair into the measured pair. Current receipts do show the expected
  commands; the finding concerns the instrument's required PID-reuse behavior.
- Live samples prove presence near 18 seconds and absence by 30 seconds. They do
  not establish that the child ended with the handler or at its deadline. Record
  the classifier/report overstatement because lifecycle-evidence-k8 consumes
  these design inputs. Keep the measured availability and byte observations.
- Record four findings in the linked review artifact and insert
  `macos-disconnect-availability-k203` immediately after this review. Its finite
  handoff is triage and necessary repairs before the lifecycle evidence decision;
  it carries this review's handle rather than a copied repair list.
