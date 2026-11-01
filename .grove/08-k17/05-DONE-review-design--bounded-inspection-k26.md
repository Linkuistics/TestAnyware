# bounded-inspection-k26

**Reviews:** bounded-inspection-k24

<!-- tc: off because the producer wrote this charter and the review keeps it verbatim -->

## Goal

Adversarially review the proposed inspection boundary before planning turns it
into code and retained-evidence-k21 relies on it.

## Context

Read the producer commit by handle, `docs/specs/diagnostic-process-inspection.md`,
the three editable visual views and the foundation report's bounded-inspection
section. Compare against k23-final-01, review k22 and integration k23. The current
runtime has not been changed; no inspection or storage guarantee is delivered
by this design. k25 is already live after this review and before k21.

## Done when

- Try to construct a helper creation, exec, capture, signal denial or post-KILL
  wait that can block the outer controller, including protocol backpressure,
  partial frames, destructor/exit hooks and concurrent worker cleanup timers.
  Check whether single-threaded pre-acquisition bootstrap and its readiness
  handshake cover actual Python/macOS behavior without an unacknowledged owner.
- Challenge target and worker group pinning independently: a late or stale
  inspector/callback must not signal a reaped/reused group. Check direct-helper
  PID ownership, no helper before readiness/registration, unknown creation races,
  finite shutdown, and residual cleanup after an unresponsive worker.
- Trace normal exit, TERM exit, grace expiry, PID-only interruption and permission
  failure through the stated absolute deadlines. Seek hidden extra allowances,
  an inspection that defers KILL, a final result from the wrong epoch, a dropped
  sticky failure or an unjustified absence claim. The reaping allocation is not
  evidence of ps latency. PID-only scope and live-final-group failure must hold.
- Check whether deterministic controls can demonstrate real delayed inspection,
  actual KILL followed by a stalled wait path, genuine EPERM plus uncertainty,
  stale/contradictory handoff and independent itemized helper cleanup. Injected
  permission errors and simulated unreapability must not pass as kernel evidence.
  Unknown real permission prerequisites must remain visible.
- Verify the worker contract leaves blocked evidence storage with k18, identifies
  workload spawn as a separate limit, and gives k21 immutable observation/residual
  inputs without weakening surviving-writer failure. Challenge unnecessary
  caller obligations or avoidable complexity against the named alternatives.
- Check spec/diagram agreement, source-backed claims and the concrete k25
  continuation. Produce findings only. If findings warrant integration, insert
  it before bounded-inspection-k25; a clean review creates nothing. Do not create
  an empty integration or take over implementation.

## Notes

No live guest, app quit, unsafe process suite or acceptance claim. This session
is the independent review; no competing in-session reviewer is needed. Static
analysis is sufficient unless a narrowly frozen, independently supervised local
experiment is necessary. Any such experiment retains every failed condition and
audits each exact owned identity. No provider evidence was present for k24, so
the producer intentionally wrote no Creator line.

<!-- tc: on -->

## Findings

The review read producer commit `53589d41`: the spec, the three views and
their manifest, the foundation report section and the brief changes. It read
`controller.py`, `peer.py` and `test_controller.py` at `44dc137e`, which k24
did not change. It read the pinned interpreter's `subprocess.py` and the
macOS `kill(2)` manual page. It ran no test, build, lint or format command.
Spec citations name section headings. Line numbers refer to
`experiments/exec-reliability/` at `44dc137e`.

### F1 (high): the pre-escalation snapshot keeps a ps-latency dependency and now voids the run

"Deadlines and safety ordering" requests one pre-escalation snapshot with
deadline T when the leader exits during grace. The same section allows one
in-flight inspection, and a missed snapshot deadline retires the worker for
the run. Later workload calls then stay unreached.

Take a leader that exits 20 ms before T. The controller dispatches the
snapshot with 20 ms left. Review k22 measured 12 ms to 34 ms per
uninstrumented snapshot, before the new handshake hops. The snapshot misses T.

KILL at T is correct, but the worker is now retired. No final snapshot can
run, the final observation is `unknown`, the call fails, and every teardown
receipt loses inspection. The deadlines view draws exactly this path. The
design cannot tell a stalled inspector from a healthy `ps` dispatched too
close to T.

Integration k23 externalised R2 because no static reserve establishes an
inspection-time guarantee. Deadline T on this request is such a reserve in
another form. The spec does not declare the window as a retained limitation.
An acceptance clone whose leader handles TERM slowly would fail from this
instrument limit, with every later operation unreached.

Three alternatives are open to the integrator. First, drop the pre-escalation
snapshot and send group KILL at T without a condition. k17 requires
conditional KILL for PID-only interruption only, zombies ignore KILL, and the
cost is no early finish.

Second, keep the request but give it the final cutoff
that would apply if KILL became due. A result from the old epoch is then
rejected for the decision without retiring the worker. Only a request that
misses the final cutoff retires it. Third, keep the design and declare the
window as a known false-failure class with its width.

The first or second
is preferable. In each case the "Timing and scope" control needs a healthy
`ps` dispatched inside the last 30 ms of grace.

### F2 (medium): fork without exec copies handlers, hooks and dispositions into the worker, and the spec names no reset

"Startup and helper ownership" forks the worker without exec. "Ownership"
forbids any worker signal to a workload. "Request and result contract"
forbids destructor waits and interpreter-exit joins. "Migration and review
criteria" rejects any shutdown hook or worker callback that can act on a
released target.

A fork child is a copy of its host process. It inherits
every Python signal handler, every `atexit` hook, the signal wakeup
descriptor, open descriptors and the `subprocess._active` list. A worker
that leaves through `sys.exit` or an exception runs the host's `atexit` hooks.
A host SIGTERM handler would run inside the worker on worker-group TERM.

`controller.py` installs no handler today. The tests, however, call `execute`
in process under pytest (`test_controller.py:17`). The worker is then a fork
of the pytest process, with its hooks and capture descriptors. The pre-exec
helper child is in turn a fork of the worker and holds the controller channels
until exec.

SIGCHLD matters too. POSIX and macOS reap children automatically when the
parent ignores SIGCHLD, and dispositions survive exec. An inherited SIG_IGN
would silently defeat leader and helper pinning in both processes. The spec
does not require SIG_DFL. The rule "No fork occurs from a controller with
active threads" names no check, and Python 3.12 only warns.

The "never signals" and "no shutdown hook" properties are therefore rules
without structure, and no control exercises them.

The smallest correction is
a post-fork discipline in "Startup". Reset SIGTERM, SIGINT, SIGHUP, SIGPIPE
and SIGCHLD to SIG_DFL. Clear the wakeup descriptor. Close every inherited
descriptor except the two channels. Exit only through `os._exit`.

Assert
one thread and SIGCHLD at SIG_DFL before each fork. Add a control that
delivers TERM to a healthy worker while the workload is live. It audits that
no workload signal and no evidence write came from the worker.

### F3 (medium-low): the per-helper acknowledgement round trip buys little and adds a failure class

Steps 1 to 4 of the handshake require intent, fork, publication, a controller
ACK, gate release, and then the helper's own deadline and token check. The
helper's own check already prevents exec after expiry. Publication already
makes the helper known when its message arrives. The ACK adds controller-side
recording before exec. That only matters if the controller dies in between,
and controller-crash recovery is out of scope.

The costs are two extra hops
per query and a controller turn before each query can start. A late ACK is
also a new way to retire the worker. Keep publish-before-exec and the helper's own
expiry check, check the epoch on receipt, and drop the ACK. If the ACK
stays, the spec should state what it protects that the helper's check does
not.

### F4 (medium-low): the controls need hold points and helpers that "Test seams" does not own

"Test seams and required controls" names one private worker adapter at the
controller composition point and value transformations. The controls table
needs more. It holds the real worker before fork, between fork and
registration, before exec and at the wait-return seam. It needs a real helper
that exceeds its budget and receives an actual KILL. It needs a real kernel
EPERM.

- Holds in the real worker are seams with an enabling point. Name it. Freeze
  it at worker bootstrap from the plan, never in a per-request field that a
  live run can trip. State that the same worker source runs with holds
  disabled, identified by hash.
- `/bin/ps` cannot be made slow on demand. The KILL and wait control needs the
  helper executable as an adapter parameter. An actual KILL of the setuid-root
  `ps` is then a separate control, and it may not be deterministic. The macOS
  `kill(2)` page permits that signal because the receiver's real uid matches
  the sender's. Record that rule, since k22 R3 notes the real path never ran.
- A real EPERM needs a target whose real and effective uid both differ from the
  worker's. Creating one needs privilege. State that this control is expected
  to be unavailable unattended, and what its unavailable receipt must contain.

### F5 (low-medium): the deadline algebra leaves terms undefined

- "Grace completion" in the definition of K. The current controller uses its
  own decision time (`controller.py:299`). State it.
- The consequence of `zombie_only_eperm`. Every other row of the table passes
  or fails. The current code does not fail on it alone (`controller.py:286`).
  State it.
- After group KILL, the final snapshot waits for the leader-exit observation.
  If exit is observed after K minus R, no snapshot can run and the result is
  `unknown`. Name that expected failure class so k25 can target it. The old
  loop polled until zombie-only. The single-shot rule also fails a group whose
  members are still exiting at query time. Say so.
- The handshake outcome taxonomy. A worker that reports fork failure has no
  PID and no residual. Silence after intent is an unknown creation attempt.
  "Creation intent without registration" covers both. Separate them, and say
  when the decision is made: at the request deadline or at worker exit.
- New phase bounds. The root brief declares every new phase bound before a run
  freezes. Bootstrap readiness has its one second. Worker close after the last
  safety action and bootstrap-failure cleanup each add up to four seconds to
  the controller's invocation. Neither is declared as a plan bound.

### F6 (low-medium): "concurrently" presumes one event loop the spec never requires

"Deadlines and safety ordering" runs worker cleanup timers concurrently with
resource teardown and forbids an implicit wait at function return. The
controller is single threaded, and threads are rejected. Today every wait is
its own loop: `_wait` (`controller.py:145-151`), the TERM and KILL loops in
`_terminate` (`:294-311`) and `Popen.wait` (`:392`).

For a worker timer to
fire during a teardown call, each of those must become a step of one loop.
That loop also serves the channels and worker timers. "Migration and review
criteria" lists what to add but not this restructuring. k25 may size the
increment as "add a worker" when it is "rebase every wait on one loop".

The existing controls that reach the helper through `subprocess.run`
(`test_controller.py:485-497`) will no longer reach it. The sentence "Retain
all useful existing ... controls" should say that those move to the adapter
seam as value cases: foreign group, usage error, timeout and missing leader.

### F7 (low): the spec carries status and work orders that belong in briefs

"pending design review and implementation", "until the separate
stalled-storage design lands", "The retained-file continuation must add",
"The storage continuation must preserve" and the whole "Migration and review
criteria" section are status or work orders. `diagrams.json` carries a status
of "review pending". `docs/specs/` is a current-state set, and these lines go
stale the moment the grove moves. Move the work orders to the k17 brief or to
k25, and keep the spec to the contract.

### Checked with no finding

- Ownership holds structurally. The controller is the only parent of the
  leader and of the worker, so no other process can reap either. Helpers are
  grandchildren. Workload and worker groups are disjoint through `setsid`.
- The pre-`setsid` race is covered: cleanup by worker PID until the controller
  holds the worker's PGID.
- The blocking paths in "Problem and scope" are real at Python 3.12.15.
  `run` calls `process.kill()` then `process.wait()` without a timeout, and
  `Popen.__exit__` calls `self.wait()` again. `/bin/ps` is setuid root on
  this host. The projection matches `controller.py:188`.
- Normal exit, TERM exit during grace, grace expiry, PID-only interruption and
  EPERM each reach K without an extra target allowance. KILL at T does not
  wait for a snapshot. A denied attempt advances the epoch. R lies inside K.
- Failure is sticky, no query is retried, no worker is replaced, PID-only
  scope never widens, and a live final group fails after an expected exit.
- k18 keeps storage. Workload spawn is a named separate hazard. k21 receives
  immutable values, and its context was updated.
- The spec and the three views agree. The deadlines view draws the F1 path as
  a failure, so F1 is a design question and not a drawing defect.
- No ADR is right. The choice is local and reversible, and the spec records
  the rejected alternatives.
- k25 is live after this review and before k21.

## Decisions (running log)

- Findings warrant reconciliation. Integration `bounded-inspection` is inserted
  before `bounded-inspection-k25`, the first later live sibling entry.
- The session had no `GROVE_PROVIDER_EVIDENCE`. The review writes no Creator
  line and invents no provenance.
