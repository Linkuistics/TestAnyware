# Diagnostic process inspection

## Problem and scope

The local diagnostic controller must continue owned-resource teardown when a
process-table query stalls. The existing helper timeout can block during helper
creation or the wait after KILL. Detecting an overrun after that wait returns
does not make teardown independent.

This is the selected design for the macOS-host diagnostic controller, pending
design review and implementation. It is not a delivered reliability result.
The historical tutorial trial remains **0/10, unavailable**. The public Host CLI
and TestAnyware app keep their existing routing and protocol contracts. This
design changes no In-VM agent or guest cancellation interface.

The reusable contract below survives this investigation. Implementation source
evidence and retained controls belong in the
[diagnostic foundation report](../experiments/exec-reliability-diagnostic-foundation.md#bounded-inspection-design).
The [visual document](../design/diagnostic-process-inspection/README.md) presents
the ownership, deadline and helper-start views. Diagrams are editable sources;
they describe the design and are not model-checking evidence.

## Ownership

The **controller** is the existing outer process. It owns the declared resource
ledger, the workload leader, all workload PID/group signals (including signal
0), monotonic deadlines and leader reaping. It keeps the leader unreaped until
it closes the workload's signal authority. No other component may reap that
leader. Exit notification does not reap it.

One **inspection worker** is a direct child in its own session, started before
any resource acquisition. It owns process-table helper creation, execution,
capture and helper reaping. It handles one request at a time. Its helper is
the identified absolute `ps` executable, in the worker's group, with the
existing explicit PID/PGID/state projection. Neither the worker nor its helper
may signal a workload, even to probe existence. They receive a read request,
not a termination callback. This is a trusted-code contract, not a security
sandbox for hostile code.

The controller owns termination and reaping of its worker. The worker may signal
only its own unreaped direct helper by PID. It does not signal its group.
The controller may signal the worker group only while the worker leader is
unreaped, and closes that authority before reaping it. The helpers themselves
have no signalling role. Thus no group identity is released while a worker
can still signal that group. Workload and worker groups are disjoint.

Deleting this boundary would move helper creation, exec and wait hazards back
into the controller. Keeping those operations in a thread would still couple
interpreter shutdown and shared state to a stuck inspector. Moving the entire
workload into a second execution supervisor would add target-signal ownership
handoffs and duplicate the existing execution boundary. The selected worker
hides the helper mechanics while leaving safety policy with its existing owner.

## Startup and helper ownership

Before acquisition, the single-threaded controller prepares channels and imports,
then forks the worker without an exec handshake. The parent retains the returned
PID immediately; the child establishes its session and reports readiness.
The controller accepts no request until it has acknowledged that readiness.
Before confirmed session creation, cleanup targets only the pinned worker PID;
group cleanup requires the confirmed worker PGID. No helper may start earlier.
No fork occurs from a controller with active threads. Bootstrap readiness has
a declared one-second ceiling. Failure prevents acquisition. Bootstrap failure
uses at most two seconds TERM grace and two seconds KILL/reap, records the worker
or unresolved creation attempt, and fails preparation. A kernel stall in fork
itself is outside a hard real-time guarantee; it happens before external
resources are owned and must not be represented as successful preparation.
Every later preparation exit, including subject-hash or declaration failure,
still closes and accounts for an already started worker.

After acquisition the controller never creates a process-table helper. For
each request, the worker uses this handshake:

1. Publish a creation intent carrying run, request and worker identities.
2. Fork one helper, keeping it behind a gate before exec. The worker publishes
   its PID, parent PID, group and unique request token. It keeps the helper
   unreaped while registration is outstanding.
3. The controller records the helper in its in-memory ledger and acknowledges
   that exact request only if its epoch is still open and its deadline has not
   expired. Registration does not authorize a signal to the workload.
4. The worker opens the gate. The helper checks the request deadline and token
   before exec and closes unrelated inherited descriptors. Parent EOF, a missed
   deadline or a rejected token prevents exec. No evidence file is a gate.
5. The worker captures bounded bytes, publishes the terminal outcome and reaps
   its helper. Each helper reap is recorded with its PID and request token.

Creation intent without registration is an **unknown creation attempt**. The
controller stops accepting that request and signals only the pinned worker
group. A late child cannot obtain a new acknowledgement; even a previously
acknowledged child must check expiry before exec. A child stalled across that
check can still run late under OS scheduling: its result is invalid and its
identity remains a cleanup obligation. No handshake promises atomic scheduling.

Every known helper gets an itemized outcome, including exec errors, denied
signals and missing reap acknowledgement. A worker death never establishes
helper death. A group kill return never establishes that every member died.

## Request and result contract

The internal inspection interface accepts a run identity, request identity,
workload leader PID/PGID, signal epoch, phase and absolute monotonic deadline.
The caller requests an observation; it does not assemble a process or perform
a helper timeout. The worker returns a value containing those identities, the
helper identity, query start/end, raw stdout/stderr, helper status and parsed
members. The controller adds receipt time, disposition and any signal-0 event.
All monotonic timestamps refer to this host and boot.

Use private inherited pipes or local socket pairs with explicit framing and
nonblocking endpoints. No blocking read, write, EOF drain, join, context-manager
wait, process destructor wait or interpreter-exit join may enter the controller's
safety path. A message is bounded at 1 MiB, combined raw table output at 256 KiB,
and parsed membership at 4096 rows. Overflow, broken framing and partial messages
fail the observation; never silently truncate into a valid table. A controller
turn consumes at most 64 KiB before checking deadlines, so a ready channel cannot
starve signals. Identity/control messages have their own bounded channel and
cannot sit behind raw table output. Backpressure is a failed handoff, not a
reason to wait beyond the request deadline.

An accepted table requires all of the following:

- Matching run, worker, request, workload and epoch; the epoch is still open.
- Start no earlier than request dispatch and completion of the phase's last
  termination-signal attempt (including a denied attempt),
  with ordered start/end/receipt times no later than the request deadline.
- A successful helper exit and complete framing/capture; empty stderr; valid,
  unique member PIDs and states, all with the requested PGID; the pinned leader
  present. A final zombie-only observation includes that leader as a zombie.
- No competing terminal record, conflicting duplicate PID, changed identity or
  stale record substituted for the requested result. Exact duplicate framing is
  a protocol error too; requests have one terminal result.

The observation interval is **[query start, query end]**, within the recorded
dispatch-to-receipt interval. It is not an atomic kernel snapshot or a promise
about times outside that interval. The controller makes any signal-0 probe
after validation, before epoch closure and before reap, and records its own
interval separately. It does not fold that probe into the table's time.

Classify the table separately from signal permission:

| Table and signal evidence | Classification and consequence |
| --- | --- |
| Any executable member observed | `live`; final termination fails, retaining members and remedy |
| Valid table, all members zombie, leader exit observed | `zombie_only_observed`; permits the observation-based completion decision |
| EPERM with a valid live table | `permission_failure`; fails, preserves the real errno and attempted scope |
| EPERM with a valid zombie-only table | `zombie_only_eperm`; not proof of why an earlier signal failed |
| EPERM with missing/invalid table | `permission_unknown`; fails |
| ESRCH, missing leader, invalid/late/missing table or worker failure | `unknown`; fails, never absence |

Do not claim arbitrary descendant absence from a table. Controlled fixtures
must independently account for every known child identity. Escaped descendants,
forks missed by a non-atomic observation and controller-crash recovery remain
outside the demonstrated guarantee. A final live group fails after expected
normal exit and after PID-only interruption, even when exit status matches.

## Deadlines and safety ordering

Inspection is asynchronous. The controller checks its monotonic clock and
pending safety events before processing messages. It never starts a synchronous
query to decide whether a signal is due.

Let **T** be TERM start plus declared grace (at most two seconds). Let **K** be
the earlier of grace completion and T, plus declared kill/drain/reap allowance
(at most two seconds). K never exceeds T plus that allowance. When the leader
exits normally, K is the observed exit time plus the same allowance. All work
described below, including final observation and leader reap, shares K.

For group termination, observe leader exit without reaping. On that event during grace,
request one pre-escalation snapshot with deadline T. If timely evidence shows
only zombies, signals can finish early. Otherwise, at T send KILL immediately
to the still-pinned workload group; an in-flight snapshot does not defer it.
The KILL attempt changes the signal epoch and invalidates older observations for the
final decision. No fresh pre-KILL inspection is inserted at T.

For PID-only interruption, TERM and any required KILL target only the leader.
An observed leader exit suppresses PID KILL. Unknown exit observation causes
the bounded PID safety attempt and a failed receipt. A live or unknown group
never authorizes widening to a group signal. Normal leader exit likewise does
not authorize group cleanup. Both paths still require a final group observation
and retain residual identities/remedies if it is live or unknown.

Request at most one final snapshot after the last signal and leader-exit
observation. A timely pre-escalation zombie-only result can serve as final if no
signal or epoch changed. If no leader exit is observed, final termination is
unknown. Allocate **R = min(100 ms, kill allowance / 4)** inside K for closing
authority and attempting reap; the final inspection cutoff is K minus R. This
allocation is not an estimate or proof of helper latency. Late evidence is a
real instrument failure; no sample is retried, replaced or reclassified.

At cutoff, close the target epoch, disable every target signal/probe entry point
and discard later results for decision purposes. Attempt leader reap using
nonblocking direct-child wait, retaining exact status if available. Poll only
within K. If not reaped by K, record the pinned residual leader and failure,
then continue resource teardown. Never perform a blocking wait to repair that
failure. An already-exited leader may be reaped earlier once final observation
is settled and target signal authority is closed. Reaping forbids all later
target group operations, including cleanup callbacks and signal 0.

There is at most one in-flight inspection. A snapshot deadline or protocol
failure permanently disables the worker for this run. Do not create a replacement
worker, retry the query or queue a final query behind a stalled one. Retain late
bytes only as rejected evidence within the same size cap. Later workload calls
remain unreached. Resource cleanup, disposal and their public absence commands
are still attempted; their local process inspection is explicitly unavailable,
so their receipts and disposal certainty cannot become success merely because
the command exited zero.

On worker failure the controller starts worker-group TERM and at most two seconds
grace, then KILL and at most two seconds nonblocking reap. These cleanup timers
run concurrently with resource teardown; they do not extend T, K, the 30-second
guest-cleanup ceiling or the 45-second disposal ceiling. There is no implicit
wait for the worker at function return or interpreter shutdown. The controller
retains strong references/ownership until bounded shutdown is settled, or
records a residual. A worker that remains alive must retain its helper ownership
records; one whose exit was observed can no longer signal its helpers.

After the last safety action, a healthy worker receives a close request with no
inspection outstanding and exits after accounting for its last helper. The
controller observes and reaps it nonblockingly. The same two-second grace and
two-second KILL/reap caps apply if orderly close does not finish; successful
workload results do not excuse a missing worker/helper cleanup outcome.

These are conditional scheduling guarantees, not hard real-time guarantees.
The controller's own fork, signal and nonblocking kernel calls still require
the OS to return and schedule it. Every measured overrun fails. An actual
uninterruptible helper can remain alive; that does not justify blocking the
controller or claiming disposal/cleanup certainty. Workload command creation
and stalled evidence storage are separate existing hazards, not fixed by this
inspection boundary.

## Failure retention and storage boundary

Failure is sticky. A timely valid later observation cannot erase a failed,
late, contradictory or missing required snapshot. Preserve the original reason,
raw partial bytes, timestamps, target scope, real errno, worker/helper identities,
reap evidence and every rejected result. A test whose oracle correctly rejects
a damaged sample passes the control, while that sample's receipt stays failed.

The controller's in-memory resource ledger and deadlines are sufficient to
start safety actions. Inspection has no filesystem dependency on its control
channel. Persisting its receipt still uses the existing evidence sink until
the separate stalled-storage design lands. That design must consume immutable
inspection outcomes and residual identities without making publication an
acknowledgement required for signalling, reaping or disposal. It owns blocking
open/read/write/hash and end-to-end storage-independent safety, including the
evidence descriptors used by teardown commands. This spec alone does not close
that guarantee.

Residual records distinguish known helper, unknown creation attempt, worker,
workload leader and observed group members. Retain PID/PGID, run/request token,
parentage, executable identity when known, acquisition and observation times,
and reap state. After releasing a leader, its old PGID is historical evidence,
not a reusable kill command. The remedy is independent identity revalidation,
then only positively owned PID cleanup or cooperative fixture release. If
identity cannot be re-established, leave it uncertain; never use a broad name
kill or signal a merely reused PID.

## Test seams and required controls

Use the agreed local subprocess/transport boundary through the real controller.
Configure a private inspection-worker adapter at that composition point for
delayed creation, malformed transport and wait fault injection. Keep policy and
table validation as value transformations with explicit clock values. Tests of
those transformations supplement, rather than replace, real process controls.
No public pluggable backend or new guest interface is needed.

Freeze source, checker, interpreter, helper identities, finite case order,
deadlines and inputs before measurement. Each failed condition remains in its
own new evidence directory. An independent outer test controller observes
safety execution and audits the inspector; it must not use the faulted worker
to certify its own cleanup.

| Control | Required observation |
| --- | --- |
| Real delayed inspector | An owned real worker/helper is held beyond the request deadline; target KILL/reap attempts and a marker-producing disposal command occur on their unchanged deadlines. Releasing it later cannot turn the receipt green. |
| Creation and gate races | Hold before fork, between fork and registration, before ACK, and before exec. The outer controller continues safety work; every published helper is audited, unpublished creation stays unknown, and late ACK/expired token cannot start a valid query. |
| Real timeout/KILL/wait sequence | A real helper exceeds its budget, receives actual KILL, then the worker is held at the wait-return seam. Safety still runs. Retain the actual helper PID and signal result, then independently release/terminate and account for worker and helper. |
| Simulated unreapability | A wait adapter reports no completion through the kill/reap deadline. A failed residual receipt is emitted without blocking. Explicitly label the simulation; it is not an actual uninterruptible kernel task. |
| Genuine permission failure | A narrowly confined test controller receives a real kernel EPERM when signalling its positively owned fixture; a separate authorised owner cleans it. Freeze the confinement mechanism and verify that it actually denies the signal. If unavailable, retain that prerequisite as unavailable; an injected errno does not satisfy this control. Do not acquire privileges or signal unrelated processes. |
| Permission uncertainty | Combine the real denied signal with a missing/failed table and require `permission_unknown`. Separately retain observed zombie-only EPERM without claiming its cause, and injected classifier cases clearly marked as such. |
| Handoff defects | Wrong epoch/identity, duplicate/conflicting terminal result, partial frame, stderr, output overflow and post-cutoff delivery fail, while independent safety markers still appear. |
| Signal authority and reuse | Real early reap mutations are rejected before any later OS group operation. Deliver a stale callback after authority closes; no signal-0/TERM/KILL may occur. Audit worker-group authority separately from workload authority. |
| Timing and scope | Delay inspection across T; KILL is not delayed. Remove the shared deadline cap or overrun failure predicate and show the checker fail. Preserve opposed PID/group child outcomes, conditional PID KILL, normal-exit live child failure and controller-survival mutation controls. |
| Audit and evidence | Independently enumerate each known helper and fixture child, record actual wait/absence evidence, and reject live, reused, malformed-selector and uncertain outcomes. Keep exact argv paths, source hashes and per-case verdicts. |

The actual uninterruptible-kernel-task case is not deliberately induced. Its
contract is a failed residual and continued independently runnable safety work;
only the simulated wait and real delayed-worker paths are demonstrated by these
controls. Helper creation and uninterruptible behaviour cannot be proven bounded
by a successful ordinary run.

## Migration and review criteria

Keep the caller's immutable call/plan/resource declarations and independent
cleanup/disposal ordering. Replace the synchronous table-query dependency;
move signal-0 to the controller and add the worker lifecycle, bounded protocol,
epoch validation and residual ledger. Extend the diagnostic receipts coherently
and keep old retained receipts readable as historical evidence. Update the
self-check's explicit source/hash/type-check lists for any new module.

Retain all useful existing scope, reap, live-member, checker and exact-path
audit controls. The retained-file continuation must add stream/result identity
and surviving-writer checks without weakening live-group failure. The storage
continuation must preserve this sole signal owner and nonblocking handoff.

The design is acceptable only if review cannot find a helper wait, creation
handshake, result drain or shutdown hook that can block the safety owner, or a
worker callback that can act on a released target. If the proposed Python
worker cannot meet those rules with the actual runtime, return that concrete
counterexample to design before implementation claims the guarantee. A larger
timeout, another thread, replacement inspector or wider signal scope is not a
repair of this contract.
