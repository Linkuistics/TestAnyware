# Diagnostic evidence storage

## Problem and scope

The local diagnostic controller must continue owned-resource teardown when
evidence storage stalls or fails. Today the controller opens, writes, reads
back and hashes evidence files on its only thread. Teardown commands also
receive evidence files as their standard output and error. A blocked
filesystem call therefore stops the scheduler. That stall voids the 30-second
guest-cleanup ceiling and the 45-second disposal ceiling. Catching a raised
OSError does not help when the call never returns.

This contract covers the macOS-host diagnostic controller. It keeps the
[diagnostic process inspection](diagnostic-process-inspection.md) contract
unchanged. The public Host CLI, the TestAnyware app, the In-VM agent and the
guest cancellation interface keep their existing contracts.

Implementation source evidence and retained controls belong in the
[diagnostic foundation report](../experiments/exec-reliability-diagnostic-foundation.md#evidence-storage-design-k18).
The [visual document](../design/diagnostic-evidence-storage/README.md) shows
the ownership, phase and failure views. Diagrams are editable sources. They
describe the design. They are not evidence from a formal model.

## Ownership

The **controller** keeps every role that the inspection contract gives it. It
owns the resource ledger, all workload and worker signals, monotonic
deadlines, the one scheduler and every reap. It also owns the evidence
**outbox**, a bounded in-memory queue of serialised frames. After storage
readiness, the controller thread makes no filesystem system call until
publication ends. It holds no evidence descriptor and opens no evidence path.

One **evidence writer** is a direct child in its own session. The controller
starts it as a fresh interpreter in isolated mode, with an exec and without a
fork-only copy. Its working directory is the filesystem root.

The writer owns the run directory, every evidence file descriptor and every
blocking open, write, read and hash. It receives frames on its standard input
and sends acknowledgements on its standard output. Its standard error is the
null device. It starts no process and sends no signal. This is a
trusted-code contract, not a security sandbox for hostile code.

The controller owns the termination and reap of the writer. It records the
writer PID from its own spawn call and never accepts a PID from the writer.
It may send KILL to that PID only while the writer is unreaped. It reaps the
writer only by a nonblocking wait on that exact PID. No library helper may
reap the writer. The writer has no group role, so the controller never signals
its group.

Workload, teardown, inspection-worker and helper processes receive no writer
pipe end. The writer receives no workload, worker or teardown descriptor. Every
pipe end is non-inheritable in the controller. The forked inspection worker
closes inherited descriptors under its own contract.

## Phases and bounds

A run has four storage phases. Each phase has declared bounds in the plan.
The validator refuses a bound larger than the ceiling below.

**P0, before ownership.** The controller validates the plan and imports every
module that later phases use. It starts the writer. No resource is owned yet.

The controller never resolves, creates or reads the evidence root, in any
phase. The writer resolves it. P0 has no storage bound, because it owns
nothing. Starting the writer still waits for its exec, as any command start
does.

**P1, readiness.** The controller sends a setup frame with the run name, the
plan bytes and the subject list. The writer creates the run directory
exclusively and keeps a descriptor to it. It writes the plan record. It hashes
the subjects and writes the subjects-before record. Then it acknowledges
readiness with the subject digests.

The readiness bound is at most five seconds from the spawn. A missing, late or
failed acknowledgement is a readiness failure. On readiness failure, the
controller sends KILL to the writer and reaps it within two seconds. It
acquires nothing and starts no inspection worker. The run fails with storage
`unavailable`.

**P2, owned run.** The controller starts the inspection worker and runs the
plan. Every evidence record becomes a frame in the outbox. The controller
never waits for these frames, with one exception. Before it acquires resource
**n**, it enqueues the intent-n record and waits for its acknowledgement.

This intent gate is at most two seconds and runs as scheduler turns. Workload,
teardown and worker timers stay runnable during the gate. A missing, late or
failed intent acknowledgement prevents that acquisition. It then triggers the
forward stop below. The gate never applies to signalling, reaping, cleanup,
disposal or worker close.

The writer handles frames in sequence order. A backlog of earlier frames can
therefore make the gate miss its bound. That miss stops forward progress. It
never changes teardown.

**P3, publication.** Publication starts only after the last teardown action
and the settled inspection-worker close. Let **S** be the publication start
and **P = S + W**, where the declared window **W** is at most ten seconds.

The controller sends the ledger and requests the subjects-after digests. It
waits for those digests until **S + W/2** at most. Then it sends the result
record, with each missing digest as `unavailable:storage`. Last, it requests
the identity record and sends a close frame. The writer computes the identity
record from the files under its run-directory descriptor. The controller
drains the outbox by nonblocking writes until P.

If the writer acknowledges close before P, the controller reaps it
nonblockingly by P plus two seconds. At P, an unfinished writer receives KILL.
The controller then polls for its reap until P plus two seconds. An unreaped
writer becomes a residual in the run outcome.

P1 can add at most seven seconds before ownership. Each intent gate can
add at most two seconds before an acquisition. Publication can add at most
twelve seconds after worker close. These are declared invocation phases. They
never add to a workload ceiling, the guest-cleanup ceiling or the disposal
ceiling. Receipts record each phase's absolute start, deadline, outcome and
any overrun.

## Channel contract

The controller serialises each record at enqueue time. A frame holds bytes,
never a live object or a descriptor. Later changes to controller state cannot
change a queued frame. Inspection observations and residual identities enter
frames unchanged, as the inspection contract produces them.

A frame has a sequence number, an operation, a record name and at most one
mebibyte of payload. The writer refuses a record name that is reserved, empty,
or contains a path separator. It creates each file exclusively, without
following links, relative to its run-directory descriptor. A larger record is
a sequence of append frames after one exclusive create.

The frame pipe end is nonblocking in the controller. Each scheduler turn writes
until the pipe reports `EAGAIN` or until 256 KiB. `EAGAIN` leaves the frame in
the outbox. `EPIPE` marks storage failed. The acknowledgement pipe end is
nonblocking, and each turn reads at most 64 KiB. Both ends are readiness
sources for the scheduler.

The outbox holds at most 64 MiB. A frame that would exceed the cap is dropped
and counted by record name. The controller never blocks to make room.

The writer acknowledges each frame by sequence number with `ok` or with the
real errno and the operation. An acknowledgement means that the write call
returned. It does not mean that the bytes are durable.

## Workload and teardown descriptors

Every call that the controller starts writes its standard output and error to
pipes. Its standard input is the null device. Its working directory is the
filesystem root.

The scheduler drains each pipe nonblockingly, at most 64 KiB per stream per
turn. It keeps a running length and SHA-256 digest per stream.
It decides the expected-output match from the bytes in memory. A readback of
the evidence file never decides a receipt.

The controller closes a call's read ends at that call's final observation.
A remaining descendant that writes later receives SIGPIPE or `EPIPE`, and the controller
never waits for EOF. The stream bytes enter the outbox as append frames.

The validator refuses a teardown call whose executable is relative or lies under
the evidence root. It compares paths lexically, without filesystem access. A
teardown command therefore neither inherits nor execs an evidence path.

## Forward stop and failure reporting

Storage fails when an acknowledgement reports an error, when the frame pipe
reports `EPIPE`, or when the writer exits early. An outbox drop and a missed
intent gate are also storage failures.

Storage failure is sticky. It stops
forward progress: no further acquisition and no further workload call. It never
changes teardown. Cleanup, disposal, inspection and worker close run with their
existing deadlines. The run outcome is failure, because required evidence is
incomplete.

The result record has a storage section. It holds the readiness interval and
outcome. It counts frames enqueued, acknowledged, failed and dropped, with the
first failure's sequence number, operation, record name and errno. It holds
the outbox high-water mark, the forward-stop reason and the publication
outcome.

For every record, it holds the controller-computed length and
SHA-256 digest of the bytes the controller sent. A reader can therefore detect
a short or missing file without trusting the writer. When the subjects-after
acknowledgement does not arrive before P, each subject digest is
`unavailable:storage`.

The controller returns the run outcome to its caller in memory. The outcome
holds the run directory, the overall verdict, the storage section, the ledger
and any residuals, including a residual writer. Reporting it after return may
block, because all safety work is complete by then. A caller learns the safety
outcome without reading evidence back.

## Controller-crash limits

Recovery from a controller crash is out of scope. This contract states only
what survives the crash.

- Owned resources stay acquired. Nothing releases them.
- The writer receives EOF on its standard input. It writes the complete frames
  that it has already read, then writes a storage-close record. That record
  says `orderly` after a close frame and `eof_without_close` otherwise. Then
  the writer exits.
- An intent record whose acknowledgement arrived before the crash exists as a
  file. A person can use it to find a leaked resource.
- A writer that is stalled when the controller crashes stays alive as an
  orphan. Nothing signals it.
- The writer does not call fsync. A host crash or power loss can lose
  acknowledged bytes.

## Timing conditions

These are conditional scheduling guarantees, not hard real-time guarantees.

- The controller's own nonblocking calls still need the OS to return and
  schedule them.
- Lazy imports and source-line lookup read the filesystem. P2 and P3 import
  no module. Error records keep the exception type and message, without a
  formatted traceback.
- Starting a teardown command reads its executable. An executable on a stalled
  filesystem blocks that start. The validator excludes only the evidence root.
  Other command-start hazards stay with the inspection and workload contracts.
- KILL takes effect only when the writer leaves an uninterruptible operation.
  A writer stuck in such an operation can remain unreaped after P plus two
  seconds. That residual never blocks the controller or extends a ceiling.

## Decisions

A separate writer process is the smallest boundary that leaves the scheduler
free of blocking storage calls. The rejected alternatives follow.

- **Writer thread in the controller.** The inspection worker's fork admission
  requires exactly one Python thread and one native thread. A thread would
  also need a join or abandonment at interpreter shutdown.
- **Storage in the inspection worker.** A storage stall would block process
  queries and helper ownership. It would join two failure domains that the
  inspection contract keeps apart.
- **Nonblocking or asynchronous file I/O.** Regular files ignore `O_NONBLOCK`.
  Open, close and metadata calls still block.
- **Longer timeouts.** A timer cannot interrupt a blocked call on the only
  thread that runs the timer.
- **Safety in a separate supervisor.** Moving teardown out of the controller
  would split the sole signal and reap ownership.
- **Writer forked without exec.** A fresh interpreter starts with a known
  descriptor table and no controller state. The five-second readiness bound
  affords the exec cost.

No ADR records this choice. The diagnostic controller is an investigation
harness, so the choice is not hard to reverse.

## Test seams and required controls

The controller accepts a writer command as an internal parameter. The
default is the real writer. A control substitutes a wrapper that runs the real
writer with one storage fault. The controller logic is identical in both
cases. Controls also declare short phase bounds and ceilings through the plan.

Each control must fail on the build before the repair. On that build, it stalls
or fails the existing storage functions through the current test seams.

- **Stalled writer.** Stop the writer with SIGSTOP after readiness. Cleanup and
  disposal finish within their ceilings. The worker closes within its bounds.
  Publication reaches P, the writer receives KILL and is reaped by P plus two
  seconds.
- **Blocking storage call.** The wrapper's first P2 file open blocks on a FIFO
  with no peer. The result matches the stalled-writer control.
- **Storage error.** After readiness, the control removes write permission
  from the run directory. The writer reports the real errno. Forward progress
  stops and teardown is unchanged. The control is `unavailable` when it runs as
  root.
- **Writer death.** Kill the writer during P2. The controller observes `EPIPE`
  or the exit and reaps the writer. Teardown is unchanged.
- **Outbox overflow.** Declare a small outbox cap and stall the writer while a
  fixture writes output. Drops are counted by record name, and forward
  progress stops.
- **Readiness failure.** The writer never acknowledges readiness. The ledger is
  empty, no intent record exists and no inspection worker starts. The writer
  is reaped within two seconds of the readiness deadline.
- **Intent gate.** The writer stalls on intent-2. The second resource is never
  acquired. The first resource's teardown is unchanged.
- **No controller storage calls.** An audit hook in the controller records
  every audited filesystem and import event. It finds none from readiness to
  the end of publication.
- **Inherited state.** Fixture commands and the writer report their open
  descriptors. Calls hold only their pipes and the null device. The writer
  holds only its three standard descriptors and its run-directory descriptor.

A deterministic control cannot produce an uninterruptible filesystem stall.
The stalled-writer and blocking-call controls use interruptible stalls. The
uninterruptible case remains the timing condition above.

## Out of scope

- Recovery after a controller crash.
- Durable storage through fsync.
- Hazards of starting a workload or teardown executable outside the evidence
  root.
- Live VM measurement. Missing real permission or setuid-ps evidence stays
  `unavailable`. The historical 0/10 unavailable trial is preserved.
