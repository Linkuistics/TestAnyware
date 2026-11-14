# Result framing review integration — k80

Both k79 findings are real issues. The receiver now rejects declarations below
two bytes as soon as the four-byte header arrives. A JSON object needs at least
`{}`. This changes one header predicate and adds no wait or deadline extension.

The framing tests collect error, requested-read budget, cumulative byte count,
first TERM, early return and fixed deadlines separately. An error assertion can
no longer hide the other checks.
Oversized and partial hostile frames explicitly require a framing error and an
unset receipt timestamp. The existing one-byte malformed JSON fixture now has
the same requirements.

## Retained conditions

Every condition has equal before/after subject hashes, raw streams, JUnit,
artifact digests and reaped outer checkers. No checker timed out. Pytest has a
60-second budget, followed by bounded TERM/KILL and reap handling.

| Condition | Snapshot | Cases | Failures | Pytest seconds |
| --- | --- | ---: | ---: | ---: |
| [red-original-01](red-original-01/verdict.json) | inputs-original | 82 | 32 | 31.18 |
| [red-minimum-01](red-minimum-01/verdict.json) | inputs-minimum-red | 82 | 6 | 10.95 |
| [green-full-01](green-full-01/verdict.json) | inputs-green | 393 | 1 | 54.32 |
| [green-full-02](green-full-02/verdict.json) | inputs-green-repaired | 393 | 0 | 51.16 |
| [red-original-02](red-original-02/verdict.json) | inputs-original-final | 90 | 34 | 37.50 |
| [green-full-03](green-full-03/verdict.json) | inputs-green-final | 401 | 0 | 47.83 |

`inputs-original/worker_capture.py` is the original receiver retained by k75.
`inputs-minimum-red` uses k75's incremental receiver with the new tests.
Both GREEN snapshots use the new minimum-length predicate. The repaired GREEN
snapshot also updates the one-byte malformed fixture's timestamp/error checks.
The auditor maps each measured source to its frozen snapshot by hash.
The final two snapshots additionally separate cumulative byte counts from
requested-read budgets. The final original snapshot still uses the original
k75 receiver. The earlier four-condition auditor and all its evidence remain
unchanged; the extended census uses a separate auditor.

The original receiver independently fails all eight requested-read budgets.
Seven modes fail the first-TERM and early-return bounds. Each event must occur
before 0.45 seconds within the original 0.9-second allowance. Overflow meets
both timing bounds. The original receiver also meets the fixed deadlines and most cumulative
byte counts. Those checks remain controls, without a false claim that they were RED.
The original hostile oversized and partial cases supply two further failures.

The minimum-length RED isolates six failures. Zero and one-byte declarations
each produce the old query timeout, late TERM and late return. Each property
fails in its own testcase. After the predicate change, the first full run fails
only the old malformed fixture expectation. Its one-byte body now fails at the
header, before a receipt timestamp exists. The corrected full run passes all
393 tests, lint, formatting and strict types. Its four checks total 53.38 seconds.
The last RED independently fails overflow's cumulative byte count: 1,081,344
bytes against the 1,048,581-byte boundary. Its other cumulative counts pass.
It also exposes the updated malformed-fixture expectation on the original
receiver. Final GREEN passes all 401 tests and the three static checks, totaling
48.97 seconds. The earlier failed and successful samples remain retained.

The auditor verifies exact failure sets and reason patterns, then compares every
observed testcase with final GREEN. All 73 failing observations now pass.
A control corrupts one reason while preserving its testcase name and failure
count. The same validator rejects that unrelated setup error.

## Byte and lifecycle controls

Real forked workers publish through real pipes and hold control open for two
seconds. The malformed-frame fixtures never fork a helper. Parent-side reads
record requested and returned bytes. Final GREEN retains six independent
receipts per mode, with these exact totals:

| Mode | Result bytes |
| --- | ---: |
| Oversized header | 4 |
| Short header EOF | 2 |
| Short body EOF | 5 |
| Trailing byte | 7 |
| Cap overflow | 1,048,581 |
| Zero declaration | 4 |
| One-byte declaration | 4 |
| Fragmented oversized header | 4 |

Each request respects the remaining header budget, then the declared remainder
plus one detection byte, capped at 32 KiB. Every malformed case rejects before
the timing cutoff and leaves the receipt timestamp unset. Query and shutdown
deadlines keep their original values. Unknown helper creation remains a failure.

Positive controls accept fragmented valid JSON, including a body of exactly
FRAME_CAP bytes padded with legal JSON whitespace. They use real `/usr/bin/true`
helpers and obtain matching helper/worker reap evidence. A complete result with
control still open fails at the original deadline and retains its helper
obligation. Test ledger disposal does not acknowledge a runtime helper reap.

## Evidence and resource audit

The initial checked [auditor](audit.py) ran through `diagnostic:resource-audit`
and passed Ruff lint and formatting. Its [verdict](audit-01/verdict.json) records:

- 3,026 declared subjects with equal before/after hashes, including the auditor,
  snapshots, artifacts, live diagnostic sources and selected tools.
- All 80,336 files in [prior-evidence.json](prior-evidence.json) unchanged before
  and after the audit, including every earlier k75 evidence byte.
- All 1,616 retained exact positive PID selectors absent in a separate `/bin/ps`
  observation, with exit 1 and empty stdout/stderr.
- Live, absent, mixed and invalid selector controls give
  `[false, true, false, false]`. Simulated selectors remain excluded.
- 734 recorded or sibling scratch directories checked. Two mock markers were
  removed after exact PID absence and matching their acquisition plans, run
  identities and exact `owned` bytes. Their hashes and final absence remain recorded.

Those four k80 conditions belong to the initial census. Later PID absence
establishes absence at audit time. It grants no historical reap authority and
does not resolve unpublished helper creation. Failed captures remain failed.
Measured pytest cache files are explicitly retained with the frozen subjects.

The [extended auditor](audit-final.py) also passes lint and formatting through
the same recipe. Its [final verdict](audit-final-01/verdict.json) reconciles all
six conditions and 73 failing observations, rejects the wrong-reason control
and checks six independent receipts per framing mode. It freezes 4,570 subjects
with equal before/after hashes, including the initial auditor and its outputs.
The narrative report is excluded from the measured subjects. All 80,336 prior
evidence files remain unchanged. Its new census observes 2,440 exact fixture PID
selectors absent, checks 593 scratch directories and removes one remaining
exactly plan-owned mock marker after the same ownership and absence checks.
The four oracle controls again give `[false, true, false, false]`. Neither census
turns later absence into historical reap evidence or resolves unpublished helper
creation.

The persistent worker/shared scheduler remains live in
`complete-worker-scheduler-k78`. Every aggregate ancestor stays open.
Ten-clone acceptance remains **0/10, unavailable**. This slice acquired no
privilege and touched no guest or installed application.
