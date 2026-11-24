> Historical report. Paths and commands refer to the [archived checkout](../../README.md).

# Fork-failure channel EOF ordering (k116)

A timely admitted helper fork failure can lose its no-child classification when
result EOF arrives before control EOF. The main loop rejects the empty result
frame; its cleanup loop already reads control EOF but did not record that fact
in `closed_channels`. The two-line repair records this existing observation.
It admits no late message and changes no framing, byte cap, deadline or signal
policy. Incomplete or contradictory data still retains uncertainty.

The new real-pipe control injects EAGAIN at the helper fork boundary and drops
the terminal result, producing a real worker exit 70 with no helper. Parent read
adapters defer only EOF observation to force both orders, after the actual
fork-failure control bytes arrive. They synthesize neither bytes nor EOF and add
no sleep. Worker creation, protocol transport and exact-child reaping remain
real. The fork refusal and dropped frame are deliberately injected faults.

`red-order` was frozen and measured before the runtime edit: control-first
passes; result-first fails with `incomplete worker frame length`, lost receipt
errno, unknown creation and a residual remedy. The retained failure message
already contains errno 35 and the worker is reaped with exit 70. Removing only
the repair in `omit-cleanup-eof` reproduces the same failure. This demonstrates
an EOF-ordering defect with the earlier lost-result symptom; it does not prove
which order occurred in any historical sample lacking an EOF trace.

`green-controls` passes all 37 focused cases, including both new schedules and
the existing invalid, duplicate, contradictory, partial, trailing, open-channel,
late-publication, publication-error and worker-retirement controls. A confirmed
fork failure still fails the observation and preserves worker failure. Known
helper obligations and unknown creation remain distinct.

## Measurement contract

`comparison-plan.json` declares two interleaved full-suite parent/candidate pairs
before measurement. Each side uses the same 854 tests, config, lockfile and
Taskfile. Only `worker_capture.py` differs, by the two EOF bookkeeping lines.
This compares genuinely changed worker bytes under full suite load, unlike the
historical identical-byte isolated comparisons. It is a finite local sample,
not a global repeatability guarantee or shared-scheduler implementation.

Preparation and checker scripts are adapted from k118 into this new directory;
old tooling and receipts remain untouched. Each condition retains declarations,
raw logs, JUnit, receipt artifacts and before/after source/tool/interpreter and
executable hashes. Pytest has 60 seconds for focused controls and 120 seconds for
each full suite; each static check has 30 seconds. Timeout cleanup keeps the
existing checker TERM 2 seconds then KILL/reap 2 seconds. Independent exact
termination-fixture identity/absence audits run outside the tested worker.
EOF fixtures retain exact worker PID, direct-child reap status, failure errno
and absence of helper identity; this injected fork failure creates no helper.

Tool development initially found a Ruff long line and strict Pyright unknown
collection types; both were corrected before full-suite measurement. The early
RED and green control receipts retain the tool hashes they actually used.

## Retained results

All four planned full runs completed in the declared order, without a checker
timeout or a changed read subject. Each contains the same 854 named tests.
Ruff, formatting and strict Pyright passed in each full run.

| Condition | Tests passed / run | Pytest checker seconds | Failed test identities |
| --- | ---: | ---: | --- |
| red-order | 1 / 2 | 0.597 | result-first EOF |
| green-controls | 37 / 37 | 4.667 | none |
| omit-cleanup-eof | 1 / 2 | 0.949 | result-first EOF |
| parent-full-1 | 852 / 854 | 79.486 | result-first EOF; blocking sample 2 deferred-reap |
| candidate-full-1 | 854 / 854 | 69.129 | none |
| parent-full-2 | 853 / 854 | 77.735 | result-first EOF |
| candidate-full-2 | 854 / 854 | 73.321 | none |

“Result-first EOF” is
`test_fork_failure_lost_result_eof_order[result]`. The additional first-parent
failure is
`test_declared_shutdown_real_child_phases[blocking-2-deferred-reap]`, at
`test_shutdown.py:298`, asserting every probe precedes its original cutoff.
Its [retained receipt](parent-full-1/runs/shutdown-blocking-deferred-reap-1794716286099350000.json)
has one probe 0.004944042069837451 seconds after cutoff 333565.29822554096.
The independent fixture cleanup restored the original waitpid, killed only its
still-owned direct child and reaped status 9. All 84 shutdown fixture cleanup
receipts in each full run report the original exact-child WNOHANG audit as
reaped (336 total). The outer checker also found both separately journaled
termination fixtures absent in each full run, without further signals.

The EOF change does not touch those shutdown polls. That failure remains open
in k119 along with the earlier timing/setup failures. Neither candidate pass
proves it fixed. These four runs establish the forced EOF boundary on this
host; they do not establish causality for an earlier untraced lost-result
sample or reliable timing under every load.

The frozen parent worker SHA-256 is
`726ad21c17dc5a44a067f06539401465704b4d140312077f3016e7384b3b61fb`;
the candidate is
`535fccf76098bbddb38ef1398b2a1dd8d9fe85aaf16575da1dd57f3e409c7ff2`.
Pairwise source comparisons allow exactly the two cleanup EOF lines in
`worker_capture.py`; all other frozen sources, tests, manifests and Taskfile
are identical. Both pairs use the same respective bytes. The final audit also
compares the shipping sources with the measured candidate.

After the last full run, the result auditor was extended to require the exact
additional shutdown failure, all 336 shutdown cleanup receipts and matching
per-test identities across runs. `full-suite-audit.py` preserves the exact
auditor bytes read by the four full measurements. Earlier RED/control runs
retain their own tool hashes. The new auditor's strict typing initially found
an untyped list; its annotation was corrected before the final tooling check.
No measured condition was overwritten or replayed to replace a failure.

The [final evidence audit](condition-audit.json) passes: exact failed identities,
all seven conditions' before/after hashes and artifact digests, matching candidate
shipping sources, fourteen EOF receipts, 336 shutdown cleanup receipts and
181,726 unchanged earlier evidence paths. A fresh final tooling check passes
Ruff, format checks and strict Pyright for all five tools; its observed result
and final tool hashes are recorded in [final-verification.json](final-verification.json).

## Reproduction

From the repository root use `mise exec -- task` with
`diagnostic:fork-channel-prepare`, `diagnostic:fork-channel-check`,
`diagnostic:fork-channel-preserve`, `diagnostic:fork-channel-tools` and
`diagnostic:fork-channel-audit`. Prepare takes INPUTS and a fresh OUT relative to
`experiments/exec-reliability`; optional MUTANT=omit-cleanup-eof removes the
repair. Check takes INPUTS, fresh OUT, MODE=controls or full, and optional SELECT.
Preserve takes MODE=before or after. Each retained output is one-shot. Do not
overwrite an old condition or increase ceilings to conceal a failure.

## Remaining scope

Complete-worker-scheduler-k119 retains the entire original aggregate charter,
including the shared scheduler, private-wait removal, persistent worker lifecycle,
teardown overlap, remaining controls and final k28 review. Its future scheduler
comparison must use its actual changed runtime under interleaved full-suite load.
Historical unsupported ps states and other timing/setup failures remain
unresolved; passing samples do not repair them. k28/k17 remain open, guest
acceptance remains 0/10 unavailable. No guests, installed applications,
privilege acquisition or unrelated process signals were involved.
