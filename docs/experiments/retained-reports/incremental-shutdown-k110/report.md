> Historical report. Paths and commands refer to the [archived checkout](../../README.md).

# Incremental worker shutdown (k110)

Worker shutdown now exposes `begin_close()` and `advance_close()`. The existing
`close()` consumer drives those same operations. A caller can interleave other
work between shutdown turns while retaining the worker's original deadlines,
signal history, sticky failure and phase receipts.

This is the narrowed k110 lifecycle slice. The complete original aggregate
charter is preserved in `complete-worker-scheduler-k114`; the separate k113
review owns independent challenge of this slice. Shared controller scheduling
and concurrent teardown are still outstanding. k28/k17 stay open; ten-clone
acceptance remains **0/10, unavailable**.

## Implemented boundary

`begin_close()` validates bounds, closes command EOF once and freezes the phase
timeline in a typed private state. It performs no poll, signal or wait.
`advance_close()` performs at most one exact-child `WNOHANG` attempt, sends due
worker escalation within the fixed authority, and returns an absolute next
wake or terminal `None`. Neither operation sleeps. Receipt assembly and channel
closure occur once at completion. Repeated begin/advance/close calls grant no
new authority. `close()` remains a synchronous compatibility driver.

The previous phase/reserve formulas, earlier finite cutoff, prior attempted
TERM/KILL (including denial), strict on-time/equality/late accounting and
owner-lifetime first-reap guard remain in force. Request polling consumes that
first-reap exception: expired close cannot renew it. Read the
[k112 report](../worker-reap-scope-k112/report.md) for the resulting waitable-child
residual limitation. This refactor introduces no new framework API or dependency.

## Retained conditions

Every directory below is an exclusive output; failed samples were preserved.
Declarations, raw stdout/stderr, checker receipts, per-case JUnit XML, subject
hashes and fixture receipts remain beside their verdicts.

| Condition | Cases | Result |
| --- | ---: | --- |
| [red](red/verdict.json) | 42 | 42 expected missing-`begin_close` failures before implementation |
| [green-controls](green-controls/verdict.json) | 151 | All pass |
| [green-full](green-full/verdict.json) | 797 | 796 pass; ordering/ignore-term fails closed on a real unsupported ps state |
| [self-check](self-check/verdict.json) | 797 | All pass; lint, formatting and strict types pass |
| [review-controls](review-controls/verdict.json) | 238 | All pass |

The shutdown matrix runs blocking and incremental drivers over fourteen modes,
three samples each: orderly, healthy stall, failure TERM, failure KILL, deferred
reap, delayed wait return, prior/denied TERM, prior/denied KILL, finite/expired
cutoff, finite reserve and aged TERM. The 42 incremental cases are the added
behavior. They forbid sleeps inside begin/advance, count at most one poll per
turn, retain the same start/deadline on every turn, observe independent caller
turns before escalation, and verify terminal idempotence. Existing exact-clock
controls retain early, equal and 3 ms late phase expectations.

Fixture forks, worker signals, exits and exact-child status consumption are
real. Deferred reap and delayed syscall return are explicitly simulated;
denied signal history is injected and is not evidence of real UID EPERM. Each
real fixture independently restores the original signal/wait functions and
checks or consumes its exact child's status. This does not establish a real
setuid ps kill, actual privilege denial or a hard real-time guarantee.

## Preserved full-run failure

The frozen full replay failed
`test_review_group_kill_precedes_inspection[ignore-term]` on its final `ok`
assertion. Its
[raw receipt](green-full/runs/e0d6a3f2a0dc45b2ac356b44732ba778/result.json)
records helper stdout hex `3330343136203330343136203f4520200a`, which decodes to
`30416 30416 ?E` with trailing whitespace. The parser reports `unsupported
process state`, retains unknown group evidence and fails closed. The receipt
records TERM then KILL before the query, leader reap with return code -9, and
an on-time orderly worker close without signals. This identifies the observed
failure path; it does not establish why ps emitted that state or resolve its
repeatability. No parser, test bound or timing allowance was relaxed.

The subsequent required self-check uses the same runtime/test bytes and passes.
The audit compares per-case identities and outcomes: exactly this ordering case
changes from failure to pass. That replay does not repair or erase the first
sample. k114 carries this observation with the earlier inspection-repeatability
limitations.

## Reproduction and integrity

Run repository tasks through `mise exec -- task`. Frozen sources were prepared
using `diagnostic:worker-reap-scope-prepare INPUTS=. OUT=evidence/incremental-shutdown-k110/<new-inputs>`
from the diagnostic task's directory. The matching `diagnostic:worker-reap-scope-check`
takes `INPUTS`, a new `OUT`, `MODE=controls|full`, and optional `SELECT`. RED selected
`declared_shutdown_real_child_phases and incremental`; focused green used the
runner's default bounded shutdown selection. Run the mandated live checks with
`diagnostic:self-check OUT=<new-root-relative-directory>` and
`diagnostic:review-controls OUT=<new-root-relative-directory>`.

The frozen runner retains its declared 60-second focused/120-second full
checker bounds and 30-second static bounds; the standard self-check retains its
own declarations. All checkers were reaped without timeout. Every condition's
before/after subjects match. Frozen sources include all eleven top-level Python
files, manifest, lockfile and Taskfile. The runner also hashes the spec, runner
tools and named system tools. Live self-check/review-control declarations name
their twenty subjects. Tool/audit Taskfile wrappers were added after self-check;
the final review-control run includes them. Runtime/test/manifest/lock bytes
still match `green-inputs`; the earlier frozen Taskfile remains historical.

`diagnostic:incremental-shutdown-tools CLI_ARGS=--check` passes Ruff format,
lint and Pyright for the evidence tools. `diagnostic:incremental-shutdown-preserve`
inventoried before/after prior evidence. `diagnostic:incremental-shutdown-audit`
produced [condition-audit.json](condition-audit.json): five conditions, 4,919
audited read subjects unchanged, 453 named exact-child cleanup receipts verified,
and all 167,784 prior evidence paths unchanged. The audit does not run the
subjects again; its output is exclusive, so use a separate copy for a new audit.

No guests, privilege, installed applications or unrelated processes were used.
Controller readiness/request/wait/reap loops, aggregate safety-first scheduling,
actual shutdown/teardown overlap, remaining real controls and final k28 review
remain owned by k114. No aggregate completion is claimed here.
