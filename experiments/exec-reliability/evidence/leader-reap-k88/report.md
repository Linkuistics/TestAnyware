# First workload-leader reap at an expired deadline

The controller now makes the first exact-child `waitpid(pid, WNOHANG)` attempt
when the fixed reap deadline K has arrived or passed. Signal and process-query
authority closes before this attempt. Only subsequent polls require time before
K; an unsuccessful expired attempt neither sleeps nor polls again. A successful
late reap records status and releases the strong owner while preserving the
failed process-bound result. ECHILD and withheld completion retain the owner and
residual receipt. No grace period is renewed.

This is the narrowed complete-worker-scheduler-k88 slice. The full persistent
worker, shared scheduler, outstanding controls and final k28 review remain in
complete-worker-scheduler-k91. k28 and k17 remain open. Ten-clone acceptance is
**0/10, unavailable**; no guest or installed application was exercised.

## Retained measurements

| Condition | Outcome | Purpose |
| --- | --- | --- |
| `red-1` | Seven expected failures, nine passes | Original runtime against the six new equality/expired controls and ten neighboring cases. |
| `green-1` | All 16 cases pass | Minimal first-attempt fix, same case identities and bounds. |
| `full-1` | All 460 tests, lint, formatting and strict types pass | Repository `diagnostic:self-check` against the live diagnostic sources. |
| `green-2` | All 16 cases pass | Same runtime and cases with the final formatted, explicitly typed retained runner. |

The six new cases each start an actual `/usr/bin/true` child, wait for its exit,
and retain a zombie observation before the runtime attempt. Their clocks are
simulated at equality/expiry; the success cases perform a real exact-child reap.
No-completion and ECHILD responses are simulated adapters. Each case retains
its exact PID, attempts, receipt, and an independent final absence observation;
the independent owner reconciles unsuccessful runtime attempts. These are
ownership and receipt controls, not measurements of operating-system latency.
The existing expired-observation control additionally exercises the caller.

Frozen `red-inputs` and `green-inputs` retain the complete diagnostic sources,
manifest, lockfile and task recipes. Each condition retains raw streams, JUnit,
checker PID/PGID, bounded shutdown/reap receipts, before/after subject hashes,
and artifact digests. Controls have 20 seconds and the full suite 120 seconds;
checker shutdown remains TERM 2 seconds then KILL/reap 2 seconds. No bound was
raised. `full-1` uses the repository checker order (lint, format, types, tests).
The focused runner uses only pytest. The runtime change is version-invariant
loop admission; the existing Python waitpid citation remains beside the call.

## Evidence audit and reproduction

The omission control in `negative-inputs` provides only five of the required
six child audit records, using the same auditor assertion logic. It exits 1 at
`assert len(audits) == 6`, with raw output in `negative.*`; no success verdict is
created. The primary samples remain unchanged.

`audit-verified.json` checks exact test identities, the expected RED failure set,
all six outcome/deadline combinations in every condition, exact positive-PID
WNOHANG attempts, failed timing, status/errno/residual relationships and retained
zombie/absence observations. It also checks every artifact digest and all
103,182 prior evidence files against `prior-evidence.json`. It hashes its
complete enumerated input set before and after inspection and records equal
inventory digests. This audit checks recorded observations; it makes no new
process-table query and claims no observation after PID reuse.

`run_condition_original.py` preserves the runner bytes used for the first two
conditions before formatting and an explicit commands-list type annotation.
`red-inputs/Taskfile.yml` preserves the earlier task recipe used by those runs
and `full-1`; `Taskfile.green-2.yml` preserves the subsequent recipe. The auditor
verifies these archived versions when a subject's current bytes differ from the
measured version. The final runner and auditor
pass their separate lint, format and type checks in `tools-verified.*`.

Reusable commands from the repository root (OUT must be fresh):

```sh
mise exec -- task diagnostic:leader-reap-check INPUTS=experiments/exec-reliability/evidence/leader-reap-k88/green-inputs OUT=/tmp/new-leader-reap-controls
mise exec -- task diagnostic:leader-reap-tools
mise exec -- task diagnostic:leader-reap-audit OUT=/tmp/new-leader-reap-audit.json
mise exec -- task diagnostic:self-check OUT=/tmp/new-diagnostic-full
```

The RED source/checker bytes and outcomes are immutable. The full run does not
resolve the earlier intermittent permission-table and empty-PID fixture failures.
Clock samples cannot prevent preemption between a deadline check and a syscall.
Independent review of this slice must precede changes to this boundary in k91;
the separate aggregate review remains required before k21.
