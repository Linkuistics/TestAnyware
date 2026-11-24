> Historical report. Paths and commands refer to the [archived checkout](../../README.md).

# Separate permission-probe timing

The controller now records when its signal-0 permission probe starts and
finishes. A receipt can distinguish the helper's earlier process-table query
from this later operation. Both new fields are null when no probe was attempted.
An attempted probe retains its interval even when it raises an operating-system
error or finishes at the inspection cutoff; the existing classification and
deadline checks still decide whether the observation is usable.

## Reading the three intervals

- `started_monotonic` through `finished_monotonic` covers the whole controller
  observation, including the worker exchange, cleanup, parsing and probe.
- `helper.worker.published_times.started_monotonic` through
  `finished_monotonic` retains the validated helper/query capture interval. It
  includes helper lifecycle work and is bounded by worker dispatch and receipt.
- `probe_started_monotonic` through `probe_finished_monotonic` brackets only
  `killpg(pid, 0)`, ending in `finally` before outcome classification. A skipped
  probe has null/null rather than an invented zero-length interval.

These intervals describe successive observations. They do not establish an
atomic process snapshot. A permission probe cannot repair stale table evidence,
and `EPERM` alone cannot establish whether executable members survive.

## Retained validation

| Condition | Per-case result | Evidence |
| --- | --- | --- |
| Frozen original runtime with new controls | All ten selected cases fail on `KeyError: 'probe_started_monotonic'` | [RED verdict](red-02/verdict.json), [JUnit](red-02/pytest.xml) |
| Frozen changed runtime with identical controls | The same ten case identities pass | [GREEN verdict](green-01/verdict.json), [JUnit](green-01/pytest.xml) |
| Current diagnostic suite | 424 cases pass; Ruff lint/format and strict pyright exit zero | [Full verdict](full-01/verdict.json), [JUnit](full-01/pytest.xml) |
| Evidence reconciliation | Identical RED/GREEN identities, all selected cases present in full suite, unchanged subjects/artifacts and 75,628 prior entries | [Audit verdict](audit-01/verdict.json) |

The nine deterministic cases cover permitted probing, live and zombie-only
tables with injected `EPERM`, injected `ESRCH`, a probe reaching the cutoff,
invalid table data, absent leader exit, expired cutoff and sticky suppression.
They check interval boundaries and retain the query and overall meanings.

The tenth case queries an owned, unreaped `/bin/sleep` session through the real
worker and `/bin/ps`, then probes the same group. Its raw receipt orders dispatch,
query, receipt and probe inside the overall interval. Both helper and worker
report reap; a separate fixture audit records the exact workload PID, SIGKILL
status and completed wait. The audit names the raw receipt and fixture audit for
both GREEN and the full suite under `real_receipts`.

Every checker completed without timeout and was reaped. Condition declarations
freeze source, manifests, Taskfile, runner, prior-evidence inventory and executable
subjects before execution; verdicts compare them afterward and hash raw artifacts.
The deterministic audit also checks current source against frozen GREEN inputs,
reconciles individual JUnit cases, and rehashes the prior inventory. Symlink
entries compare link text; regular files compare SHA-256 bytes. This auditor is
producer verification; the separately scheduled review supplies independent
adversarial assessment.

The initial inventory setup failed on a historical symlink to a directory. Its
following runner setup failed before tests because inputs were not yet written.
Neither attempt is credited as RED. The retained measured conditions begin at
`red-02`; no historical evidence was rewritten.

## Reproduction

From the repository root, choose a new output directory for each measurement:

```sh
mise exec -- task diagnostic:probe-interval-check INPUTS=experiments/exec-reliability/evidence/probe-intervals-k81/inputs-red OUT=/tmp/probe-red-new
mise exec -- task diagnostic:probe-interval-check INPUTS=experiments/exec-reliability/evidence/probe-intervals-k81/inputs-green OUT=/tmp/probe-green-new
mise exec -- task diagnostic:self-check OUT=/tmp/probe-full-new
```

RED intentionally exits nonzero. The auditor ran through
`diagnostic:resource-audit` with `AUDITOR` pointing to this directory's `audit.py`;
its fixed `audit-01` destination refuses replacement. Preserve the retained run
and use a separate evidence copy when repeating that audit.

## Remaining work

The current transport still creates a worker per query. The persistent worker,
shared scheduler and every other original aggregate obligation remain in
`complete-worker-scheduler-k83`; bounded-inspection-k28 and diagnostic-evidence-k17
remain open. The permission errors here are injected controls, not privileged
UID/KILL evidence. No guests, installed app changes or privileged operations were
used. Ten-clone acceptance remains 0/10 unavailable. This slice's review does not
satisfy the eventual aggregate runtime review before retained-evidence-k21.
