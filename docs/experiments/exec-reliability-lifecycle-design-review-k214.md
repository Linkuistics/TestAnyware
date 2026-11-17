# Review of the guest command lifecycle design (k214)

Six material findings need triage before `lifecycle-delivery-k213` plans the
implementation. The design has useful boundaries: one process owner, explicit
cancellation on a separate connection, a typed outcome, and a download from one
open handle. Its cleanup bounds, evidence contract and compatibility promise
are not yet coherent enough to implement as written.

The producer is `lifecycle-evidence-k8`, commit
`6089b92f5c2516c8ebb3c2fd3d8e8a1421c617a8`. This review inspected its committed
specification, four ADRs, glossary additions and six diagram sources against
the root brief, the causal reports and relevant current source. It ran no
tests, builds, lint, formatting or diagnostic workloads, and changed no design,
production or test code.

Graph generation and index coverage could not be verified. The graph MCP tools
were unavailable, and the CLI refused project and coverage queries with:
`codebase-memory-mcp: CBM CLI could not start because a pre-coordination or unverified CBM generation is active; close all CBM sessions and commands, then retry`.
Evidence below comes from direct source and document inspection. This is not an
exhaustive graph audit.

## R1 — P1: reaping can prevent the bounded failure record from ever being produced

Location: `docs/specs/guest-command-lifecycle.md:167`.
Related: `docs/specs/guest-command-lifecycle.md:203`,
`docs/specs/guest-command-lifecycle.md:212`,
`docs/specs/guest-command-lifecycle.md:511`,
`docs/adr/0024-one-owner-ends-each-guest-command.md:44`,
`docs/design/guest-command-lifecycle/terminate.mmd:17`.

The termination sequence bounds the TERM grace and the signal-0 probe, but
places an unconditional leader reap between SIGKILL and that probe. The spec
itself admits a group member can remain in uninterruptible sleep. If that member
is the leader, SIGKILL does not make an exit status available at the deadline.
A blocking reap prevents the owner from reaching the probe or finalising the
promised `groupEmpty: no`, `withinBounds: false` record. A non-blocking reap
would instead need a defined failure result and continuing owner; neither is
specified. Agent shutdown waits on the same termination path.

This violates the root's two-second kill/drain/reap budget and its requirement
to preserve a failed observation rather than wait indefinitely. Define a bounded
reap attempt, the truthful terminal evidence when no exit status exists, and
who retains responsibility for a later reap without unsafe group signalling.
Keep the ADR and termination diagram consistent with that failure path. A
controlled unavailable-exit-status case must exercise it; an ordinary
TERM-ignoring process that dies promptly on KILL does not.

## R2 — P1: unbounded capture work cannot satisfy a constant finalisation bound

Location: `docs/specs/guest-command-lifecycle.md:119`.
Related: `docs/specs/guest-command-lifecycle.md:124`,
`docs/specs/guest-command-lifecycle.md:519`,
`docs/specs/guest-command-lifecycle.md:606`,
`docs/adr/0023-guest-command-outcome-is-a-typed-record.md:72`.

Capture files remove the need to wait for every writer to close a pipe. They do
not make reading, hashing and base64-encoding those files constant-time. The
spec reads each file up to its size at finalisation, carries those bytes in the
record alongside legacy text, explicitly leaves output unbounded, and promises
a final record within 4.5 seconds of the outcome. An arbitrarily large file, or
a stalled filesystem read, exceeds that bound after group termination has
already finished. Only a read error currently permits `capture: incomplete`;
there is no capture deadline or defined result for exhausting one.

The bounded command can therefore leave its caller waiting on unbounded
evidence work, and the proposed controls contain no large/stalled capture case.
Reconcile the evidence delivery contract with the finite finalisation budget.
Specify what bytes remain available, what makes a capture partial, and how the
record returns within the budget. Merely increasing the constant or moving the
copy to another worker does not resolve the unbounded work. Keep exact-byte
acceptance for the root's declared samples and add a control for this failure
path; do not silently weaken those oracles.

## R3 — P1: the validity rules accept contradictory cleanup claims

Location: `docs/specs/guest-command-lifecycle.md:343`.
Related: `docs/specs/guest-command-lifecycle.md:175`,
`docs/specs/guest-command-lifecycle.md:336`,
`docs/specs/guest-command-lifecycle.md:582`,
`docs/design/guest-command-lifecycle/read-record.mmd:7`.

The exhaustive acceptance rules check the id, legacy fields, stream bytes and
digests, block presence, and non-negative ordered times. A timed-out record can
satisfy all of them while claiming both `groupEmpty: no` and
`withinBounds: true`. The same is true of `groupEmpty: unknown`, or a SIGKILL
sent after the two-second grace with `withinBounds: true`. Those claims
contradict the stated meaning of `withinBounds`, yet the Host CLI would accept
the record and emit `ok: true`.

"Step order" also needs an actual partial ordering: a leader that ignores TERM
can exit after SIGKILL, whereas a cooperative leader exits before it. Requiring
the listed `leaderExitAfterMs` field to precede `killSentAfterMs` would reject
the first legal case. An unsuccessful bounded reap from R1 also needs an honest
representation instead of an invented exit time or status.

Define cross-field truth constraints and the permitted time relationships,
including unavailable observations and the bounds used to derive
`withinBounds`. The contradictory-termination corruption control must alter a
claim such as the examples above while leaving the remaining record valid,
and both legal leader-exit orderings must remain acceptable. This directly
implements the root rule that contradictory evidence never counts as success.

## R4 — P1: the additive compatibility promise changes existing field values

Location: `docs/specs/guest-command-lifecycle.md:303`.
Related: `docs/specs/guest-command-lifecycle.md:363`,
`docs/specs/guest-command-lifecycle.md:492`,
`docs/adr/0023-guest-command-outcome-is-a-typed-record.md:31`,
`docs/adr/0023-guest-command-outcome-is-a-typed-record.md:47`,
`agents/macos/Sources/TestAnywareAgent/ProcessRunner.swift:125`,
`agents/linux/testanyware_agent/system_endpoints.py:32`,
`cli-rs/crates/testanyware-cli/src/commands/file.rs:42`.

The proposed legacy `exitCode` is -1 for every signal death. Today's macOS
runner returns `process.terminationStatus` (9 for SIGKILL), and Linux returns
`subprocess.run().returncode` (-9 for SIGKILL). Existing CLI JSON forwards that
value as `exit_code`; an old CLI's text exit also uses it. A new agent therefore
changes observable values for old consumers even if they ignore every added
field. This is explicitly disallowed by the design's own compatibility
rationale and old-CLI/new-agent matrix.

Other differences confirm this is more than a naming issue. The current macOS
decoder drops non-UTF-8 streams instead of replacing invalid sequences. Linux
returns a decode error and, on timeout, empty stdout, `Process timed out` and no
`timedOut` field. The new table changes those values too. Keeping
`schema_version: 1.0` because fields are only added does not explain these
changes in meaning.

Choose and document a coherent compatibility policy. The smallest additive
policy keeps each platform's old projection while putting corrected facts in
the typed fields, with validity rules that understand that projection. If the
intended change instead breaks old values, state that migration/version
decision and reconcile the ADR, CLI envelope, skew matrix and fixtures. Do not
claim both policies at once.

## R5 — P2: CLI JSON drops the lifecycle evidence needed by public-path acceptance

Location: `docs/specs/guest-command-lifecycle.md:366`.
Related: `docs/specs/guest-command-lifecycle.md:83`,
`docs/specs/guest-command-lifecycle.md:193`,
`docs/specs/guest-command-lifecycle.md:320`,
`docs/specs/guest-command-lifecycle.md:517`,
`docs/design/guest-command-lifecycle/read-record.mmd:10`.

The agent record includes `lifecycle` with leader PID, group ID, leader start
time, spawn duration and owner lateness. The public CLI JSON projection lists
the added fields but omits `lifecycle`. Its `record` field is only the string
`agent` or `legacy`, so it does not preserve the omitted object elsewhere. The
read-record diagram repeats that projection.

The acceptance route uses the public CLI, and the design relies on those facts
to expose a late owner and permit independent identity checks. For example,
two `exited` records with owner lateness of zero and 900 ms can produce the same
specified CLI JSON, although only one meets the 500 ms bound. The verifier
would have to bypass the agreed public path or lose the evidence.

Expose the lifecycle evidence through the CLI contract and schemas, define its
legacy/unavailable representation, and retain it in public-path evidence.
Check that the published timing fields also make each claimed bound
observable; the current lifecycle example does not define a separate
outcome-to-final-record measurement.

## R6 — P2: the design promotes a candidate stalled phase to an established cause

Location: `docs/specs/guest-command-lifecycle.md:71`.
Related: `docs/adr/0024-one-owner-ends-each-guest-command.md:21`,
`docs/experiments/exec-reliability-macos-completion.md:136`,
`docs/experiments/exec-reliability-macos-completion.md:161`,
`docs/experiments/exec-reliability-macos-completion.md:178`.

The evidence table says pool starvation can produce the symptom and labels
that cause established in the probe. ADR-0024 says the reproduced receipt
occurred when the second wait block queued behind a busy pool. The cited
report establishes the receipt under the saturated `free-1` condition, but
expressly says it observed neither dispatch block nor the child exit. Its
single-hop control removes the second block and replaces `waitUntilExit`, so
it cannot isolate which phase stalled. The report consequently labels global
queue starvation delaying that block "Possible, not established".

The design correctly leaves the historical guest cause unresolved, but still
overstates what the host probe proved. That undermines the required separation
of observed failures from causal premises and could make a green control look
like proof of a particular repair. Preserve the established reproduction under
the declared load, classify the stalled phase as unresolved, and explain that
the new runner removes both candidates without claiming either was proved.
This correction requires no new causal investigation before delivery.

## Other reviewed obligations and handoff

The four ADRs address distinct decisions: outcome representation, process
ownership, cancellation semantics and download consistency. No extra ADR or
format change is justified by this review. Holding the leader through the last
destructive group signal, leaving normal background servers alive, and making
disconnect independent of cancellation are coherent choices within the stated
limits. Escaped descendants, agent crashes and in-place file overwrite remain
explicit limits, not demonstrated cleanup or consistency successes.

The download contract corrects the demonstrated metadata/open race and
overlong Linux response at their responsible boundaries. Its client control
must use raw HTTP bytes with the real client: the k14 report establishes that
the current client accepts the declared-length prefix, and
`cli-rs/crates/testanyware-agent-client/src/lib.rs:734` consumes decoded body
frames while `cli-rs/crates/testanyware-agent-client/src/lib.rs:575` discards
the connection driver's result. A body-only fixture cannot demonstrate that
bytes after Content-Length were rejected before destination replacement.
Candidate D already owns this change and its transport control; this is a
verification constraint, not an additional finding or implementation slice.

The existing process/file, HTTP/CLI and app seams have concrete substitution
needs. The spec retains the independent byte oracles, corruption controls,
isolated builds, unchanged global installations and goldens, sequential five
fresh clones per OS, failed-sample retention and final resource audit. None of
this review supplies a passing acceptance result or changes the historic 0/10.

An `integrate-review-design` sibling must independently triage these findings
from this review's commit before `lifecycle-delivery-k213` relies on the design.
Local contract repairs can stay in that integration. If a verified finding
requires the architecture to be rethought, commission the producer/review chain
that Grove requires rather than introducing an unreviewed redesign there.
