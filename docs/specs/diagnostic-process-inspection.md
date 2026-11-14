# Diagnostic process inspection

## Problem and scope

The local diagnostic controller must continue owned-resource teardown when a
process-table query stalls. The existing helper timeout can block during helper
creation or the wait after KILL. Detecting an overrun after that wait returns
does not make teardown independent.

This contract covers the macOS-host diagnostic controller. The public Host CLI
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
Bootstrap readiness has a declared one-second ceiling. Failure prevents acquisition. Bootstrap failure
uses at most two seconds TERM grace and two seconds KILL/reap, records the worker
or unresolved creation attempt, and fails preparation. A kernel stall in fork
itself is outside a hard real-time guarantee; it happens before external
resources are owned and must not be represented as successful preparation.
Every later preparation exit, including subject-hash or declaration failure,
still closes and accounts for an already started worker.

Before every worker or helper fork, the parent verifies that it has one thread
and SIGCHLD has disposition SIG_DFL without native SA_NOCLDWAIT auto-reaping.
Check the kernel action's flags as well as its handler and Python's cached
disposition: a default handler with SA_NOCLDWAIT can discard child wait status.
The thread check must account for native threads as well as Python threads.
An unsupported check or a failed precondition
fails preparation or the request before fork. SIGCHLD remains SIG_DFL throughout
child ownership, including workload ownership. An embedding host that changes
these conditions cannot use this fork boundary. Python's fork warning alone is
not a precondition check.

The fork boundary blocks catchable signals while the child resets inherited
state. The parent restores its previous mask immediately after fork. The child
resets catchable signal dispositions to SIG_DFL, including SIGTERM, SIGINT,
SIGHUP, SIGPIPE and SIGCHLD, and clears the inherited signal wakeup descriptor.
It closes inherited descriptors except its explicitly owned protocol endpoints.
The worker keeps only its two channels. A helper keeps only its exec gate and
capture endpoints, and closes the gate before exec. Neither retains host
evidence or pytest capture descriptors. The child then unblocks signals.

Each child uses fresh local ownership state. It must not invoke inherited
subprocess cleanup, host callbacks, finalisers or exit hooks. Worker return,
bootstrap failure and every child exception leave through `os._exit`, without
unwinding into the host. Helper failure before exec follows the same rule.
Fork hooks supplied by an embedding host must satisfy these ownership rules
too. An unsupported host environment fails preparation rather than silently
weakening the boundary.

After acquisition the controller never creates a process-table helper. For
each request, the worker uses this handshake:

1. Publish a creation intent carrying run, request and worker identities.
2. Fork one helper, keeping it behind a gate before exec. Publish its PID,
   parent PID, group and unique request token on the control channel.
   Keep the helper unreaped until publication completes or the attempt fails.
3. After complete publication, release the gate without a controller ACK.
   If publication fails or exceeds the deadline, keep the gate closed.
   The controller records each received identity in its in-memory ledger,
   including late identities needed for cleanup. Registration never authorises
   a signal to the workload.
4. The helper checks the request deadline and token before exec. Parent EOF,
   a missed deadline or a rejected token prevents exec. No evidence file is
   a gate. The controller validates the epoch when it receives the result.
5. The worker captures bounded bytes, publishes the terminal outcome and reaps
   its helper. Each helper reap is recorded with its PID and request token.

A timely explicit fork-failure result records that no helper was created for
that request, with the real errno and no helper residual. Creation intent with
neither that result nor a published identity is an **unknown creation attempt**.
The controller makes that classification at the request deadline or observed
worker exit, whichever comes first. It retires the worker and signals only its
pinned worker group. A late identity refines the residual ledger but cannot
repair the failed observation. A published helper remains a known cleanup
obligation until its reap evidence is accounted for.

Publication before exec preserves identity evidence without requiring the
controller to record it before exec. Controller-crash recovery is outside this
contract, so a per-helper ACK adds no required guarantee. The helper still
checks expiry at its gate. A child stalled across that check can run late under
OS scheduling. Its result is invalid and its identity remains a cleanup
obligation. No handshake promises atomic scheduling.

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
| EPERM with a valid zombie-only table | `zombie_only_eperm`; permits observation-based completion and does not itself fail termination. Other failures remain sticky. This does not explain an earlier denied signal. |
| EPERM with missing/invalid table | `permission_unknown`; fails |
| ESRCH, missing leader, invalid/late/missing table or worker failure | `unknown`; fails, never absence |

Do not claim arbitrary descendant absence from a table. Controlled fixtures
must independently account for every known child identity. Escaped descendants,
forks missed by a non-atomic observation and controller-crash recovery remain
outside the demonstrated guarantee. A final live group fails after expected
normal exit and after PID-only interruption, even when exit status matches.

## Deadlines and safety ordering

Inspection is asynchronous. One controller scheduler serves workload waits,
termination, nonblocking reaps, resource teardown, both channels and worker
cleanup timers. Each phase returns control to that scheduler after a bounded
step. No phase owns a private wait loop or blocking `Popen.wait`. The scheduler
waits only until the earliest active deadline or ready event, then checks the
clock and due safety actions before processing messages. The channel byte cap
also applies across each turn. A teardown command cannot suspend worker timers.

Let **T** be TERM start plus declared grace, at most two seconds. Let **A** be
the declared kill/drain/reap allowance, at most two seconds. For group
termination, **K = T + A**. For PID-only interruption, **K = min(G, T) + A**,
where **G** is the controller's decision time when it observes leader exit
during grace. If it makes no such decision, K is T + A. For normal exit,
K is the controller's observed exit time plus A. A late scheduler turn never
moves T or K. Final observation and leader reap share K.

For group termination, observe leader exit without reaping, but perform no
inspection during grace. At T, attempt KILL on the still-pinned workload group,
even if the leader already exited. The attempt advances the signal epoch,
including when the kernel denies it. Only then can the final query start.
There is no snapshot deadline at T and no latency reserve for an early query.
The accepted cost is the full grace duration and a possibly redundant KILL
attempt on a zombie-only group. Early group completion is not supported.

For PID-only interruption, TERM and any required KILL target only the leader.
An observed leader exit suppresses PID KILL. Unknown exit observation causes
the bounded PID safety attempt and a failed receipt. A live or unknown group
never authorizes widening to a group signal. Normal leader exit likewise does
not authorize group cleanup. Both paths still require a final group observation
and retain residual identities/remedies if it is live or unknown.

Request at most one final snapshot after the last signal attempt and leader-exit
observation. Allocate **R = min(100 ms, A / 4)** inside K for closing authority
and attempting reap. The final inspection cutoff is K minus R. If the controller
has not observed leader exit before that cutoff, it starts no query and records
`unknown`. A failed or absent exit observation never proves completion.

This single observation has explicit limits. A healthy helper can miss its
cutoff if leader exit leaves too little time. Members still exiting at query
time can produce a final `live` result. Each case fails the call, even if all
members later exit. The controller does not retry or poll for a better table.
R allocates reap time and proves nothing about helper latency. Every late
required observation remains a failed sample.

At cutoff, close the target epoch, disable every target signal/probe entry point
and discard later results for decision purposes. Attempt leader reap using
nonblocking direct-child wait, retaining exact status if available. Poll only
within K. If not reaped by K, record the pinned residual leader and failure,
then continue resource teardown. Never perform a blocking wait to repair that
failure. An already-exited leader may be reaped earlier once final observation
is settled and target signal authority is closed. Reaping forbids all later
target group operations, including cleanup callbacks and signal 0.

There is at most one in-flight inspection. A snapshot deadline, helper creation
or execution failure, or protocol failure permanently disables the worker for
this run. Do not create a replacement
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
controller observes and reaps it nonblockingly. The close request starts the
declared grace clock. At its expiry, an unfinished close enters worker KILL/reap
without another grace interval. Successful workload results do not excuse a
missing worker/helper cleanup outcome.

Before the run freezes, its plan declares bootstrap readiness, bootstrap-failure
TERM grace and KILL/reap, worker-failure TERM grace and KILL/reap, and orderly
close grace and KILL/reap. Readiness is at most one second. Each grace and
KILL/reap phase is at most two seconds. Bootstrap failure can therefore consume
five seconds including readiness. Orderly close can add four seconds after the
last safety action. A worker already retiring keeps its original deadline.
These are declared invocation phases, never additions to a workload or disposal
ceiling. Receipts record their absolute starts, deadlines, outcomes and overruns.

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
channel. The storage boundary consumes immutable inspection outcomes and
residual identities. Publication must not be an acknowledgement required for
signalling, reaping or disposal. Storage isolation covers blocking
open/read/write/hash and evidence descriptors used by teardown commands.
This inspection contract alone establishes no storage-independent safety
guarantee.

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
delayed creation, malformed transport and wait fault injection. The adapter
freezes its helper executable and hold points from the plan at worker bootstrap.
Hold points cover before fork, between fork and identity publication, before
exec, and after actual helper KILL at wait return. They are never per-request
switches. The same hashed worker source runs with all holds disabled in ordinary
conditions. Each control records the distinct adapter configuration and helper
identity. Keep policy and table validation as value transformations with explicit
clock values. Tests of
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
| Inherited state | Start with host signal handlers, an exit hook, wakeup and evidence descriptors. Deliver TERM to a healthy worker while a workload is live. Independently show no worker-origin workload signal, evidence write or host hook. Reject extra-thread and non-default SIGCHLD preconditions before fork. |
| Creation and gate races | Hold before fork, between fork and publication, and before exec. Exercise explicit fork failure separately from silence after intent. Every published helper is audited. Unpublished creation stays unknown, and expired tokens cannot produce valid observations. |
| Real timeout/KILL/wait sequence | The adapter selects an identified owned helper that deterministically exceeds its budget. It receives actual KILL before the worker stops at the wait-return seam. Safety still runs. Retain the helper PID and signal result, then independently account for worker and helper. |
| Setuid ps signal path | Separately attempt KILL against a positively owned, unreaped instance of the actual identified ps executable. Record its real/effective UIDs, signal result and reap evidence. A helper that exits before the attempt is not proof of KILL delivery. Retain this nondeterministic control as unavailable when its timing prerequisite fails. |
| Simulated unreapability | A wait adapter reports no completion through the kill/reap deadline. A failed residual receipt is emitted without blocking. Explicitly label the simulation; it is not an actual uninterruptible kernel task. |
| Genuine permission failure | Use an independently owned fixture whose real and effective UIDs both differ from the signalling controller's UIDs. A separately authorised privileged owner creates and cleans it. Retain the actual kernel EPERM. Expect this control to be unavailable unattended. Its receipt names the missing owner/privilege, fixture and sender identities or unreached fields, intended denial and cleanup method, and operations not attempted. An injected errno does not satisfy it. Do not acquire privilege or signal unrelated processes. |
| Permission uncertainty | Combine the real denied signal with a missing/failed table and require `permission_unknown`. Separately retain observed zombie-only EPERM without claiming its cause, and injected classifier cases clearly marked as such. |
| Handoff defects | Wrong epoch/identity, duplicate/conflicting terminal result, partial frame, stderr, output overflow and post-cutoff delivery fail, while independent safety markers still appear. |
| Signal authority and reuse | Real early reap mutations are rejected before any later OS group operation. Deliver a stale callback after authority closes; no signal-0/TERM/KILL may occur. Audit worker-group authority separately from workload authority. |
| Timing and scope | Make the leader exit within the last 30 ms of group grace. No query starts before KILL at T, and a healthy final query gets the remaining final budget. Exercise exit observed after K minus R and members still exiting during the sole final query. Remove the shared cap or overrun failure predicate and show rejection. Preserve PID-only scope and conditional KILL, live-child failure and controller-survival controls. |
| Shared scheduling | Hold a teardown command open while worker TERM, KILL and reap deadlines expire. Each worker action occurs on its original deadline. Exercise bootstrap failure and orderly close against their declared invocation bounds. |
| Audit and evidence | Independently enumerate each known helper and fixture child, record actual wait/absence evidence, and reject live, reused, malformed-selector and uncertain outcomes. Keep exact argv paths, source hashes and per-case verdicts. |

The actual uninterruptible-kernel-task case is not deliberately induced. Its
contract is a failed residual and continued independently runnable safety work;
only the simulated wait and real delayed-worker paths are demonstrated by these
controls. Helper creation and uninterruptible behaviour cannot be proven bounded
by a successful ordinary run.

On macOS, a setuid-root ps retains the caller's real UID. The documented signal
permission rule allows a sender whose real or effective UID matches the
receiver's real or effective UID. Setuid status alone therefore supplies no
EPERM fixture. Actual signal and reap evidence remain necessary.
