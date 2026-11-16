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

The controller sends the ledger and requests the subjects-after digests. The
writer returns those digests in its acknowledgement and writes no
subjects-after file. The controller waits for them until **S + W/2** at most.
That instant is the only subjects-after cutoff. Each digest that has not
arrived by then is `unavailable:storage`.

Then the controller serialises the result record as a snapshot. The
[snapshot rules](#forward-stop-and-failure-reporting) state what the result
can hold. Last, the controller sends one close frame. It sends no separate
identity request. The controller drains the outbox by nonblocking writes
until P.

**Close transaction.** The writer handles the close frame after every
earlier frame. Then it runs these steps and reads no further frame:

1. Create the storage-close record. The record says `orderly`. It holds the
   sequence number of the last frame that the writer applied. It also counts
   the frames that the writer refused or failed.
2. Compute the identity record over every file under the run-directory
   descriptor except the identity record itself. The listing therefore
   includes the storage-close record.
3. Create the identity record.
4. Acknowledge the close frame with the length and SHA-256 digest of the
   storage-close record and of the identity record.
5. Exit. Write nothing more to a run file.

If a step fails, the writer acknowledges the close frame with the real errno
and the operation. It writes no identity record after a failed step, and then
it exits. A run directory without an identity record is never complete.

If the writer acknowledges close before P, the controller reaps it
nonblockingly by P plus two seconds. At P, an unfinished writer receives KILL.
The controller then polls for its reap until P plus two seconds. An unreaped
writer becomes a residual in the run outcome. The terminal storage outcome is
final at P plus two seconds at the latest.

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
or contains a path separator. The reserved names are `plan`, `result`,
`subjects-before`, `storage-close` and `result-identity`.

The writer creates each file exclusively, without following links, relative
to its run-directory descriptor. A larger record is a sequence of append
frames after one exclusive create. An append writes only to the file that its
create made, and only while that file has one link. The writer refuses an
append to a replaced or linked file.

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

The controller holds no write end of a call's pipes after the spawn. The
scheduler drains each pipe nonblockingly, at most 64 KiB per stream per turn.
It keeps a running length and SHA-256 digest per stream. It decides the
expected-output match from the bytes in memory. A readback of the evidence
file never decides a receipt.

**Final drain.** At a call's final observation, after the termination
decision and the leader reap, the controller drains each stream once more. It
reads 64 KiB at a time until EOF or `EAGAIN`, with at most 16 reads per
stream. A pipe buffer holds less than that limit, so an exited writer reaches
EOF within it. The final drain is bounded work without a wait. It adds no
cleanup or disposal allowance.

The controller then records one completeness state per stream and closes the
read ends:

- `eof`: the stream reached EOF. The stream can pass.
- `writer_open`: the stream reported `EAGAIN`, so a descendant still holds a
  write end. The receipt fails, because later bytes are unknown.
- `cutoff`: the read limit ended the drain before EOF. The receipt fails.

The receipt's length and digest describe exactly the bytes in memory. Those
bytes decide the expected-output match, and they enter the outbox as append
frames. A remaining descendant that writes later receives SIGPIPE or `EPIPE`.
The controller never waits for EOF.

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

### Result snapshot

The result record has a storage section. The controller serialises the
result at enqueue, so the section is a snapshot. Its cutoff is the result's
own sequence number and monotonic enqueue time, and the section records both.

As of the cutoff, the section holds the readiness interval and outcome. It
counts frames enqueued, acknowledged, failed and dropped, with the first
failure's sequence number, operation, record name and errno. It holds the
outbox high-water mark and the forward-stop reason. Its publication field
always says `pending`, because publication ends after the snapshot.

The section has one digest entry for each record that the controller
serialised before the result, sent or dropped. Each entry holds the
controller-computed length and SHA-256 digest of the record's bytes. The
result record and the close frame have no entry, so the coverage has no
cycle. A reader can therefore detect a short or missing file without trusting
the writer.

The writer generates three records itself: subjects-before, storage-close and
identity. The readiness acknowledgement carries the length and digest of the
subjects-before record. The section records them as writer-reported. The
close acknowledgement carries the length and digest of the storage-close and
identity records. Those values arrive after the snapshot, so only the
in-memory outcome holds them.

When the subjects-after acknowledgement does not arrive by **S + W/2**, each
subject digest is `unavailable:storage`.

### Terminal outcome

The controller returns the run outcome to its caller in memory. The outcome
holds the run directory, the overall verdict, the result snapshot, the ledger
and any residuals, including a residual writer.

The outcome also holds a terminal storage section. It holds the final frame
counts, the close acknowledgement and the length and digest of the identity
record. It also holds the writer's exit or residual state and the publication
outcome. The publication outcome is one of these:

- `complete`: an `ok` close acknowledgement arrived before P, and the
  controller reaped the writer.
- `incomplete`: the close acknowledgement reported an error or never arrived,
  and the writer exited before P.
- `killed`: the writer received KILL at P, and the controller reaped it.
- `residual`: the writer was unreaped at P plus two seconds.

The overall verdict requires the publication outcome `complete`. A caller can
anchor the identity digest from the terminal section. Reporting the outcome
after return may block, because all safety work is complete by then. A caller
learns the safety outcome without reading evidence back.

### Completion authority

The result's verdict is part of the snapshot, so it cannot show that
publication completed. A retained reader treats a run directory as complete
only when all of these conditions hold:

- The identity record exists, and its listing names every file present except
  itself.
- Every listed file is unchanged against its pin.
- The listing includes the storage-close record. That record says `orderly`,
  reports no refused or failed frame, and names a last applied sequence
  number at least the result's own sequence number.
- Each digest entry in the result matches the length and digest of its file.

A missing or changed storage-close record means the directory is incomplete.
So does a record that says `eof_without_close`. These conditions change the contract of the
retention verifier in `experiments/exec-reliability/retention.py`. The
verifier and its tests change with the writer.

## Controller-crash limits

Recovery from a controller crash is out of scope. This contract states only
what survives the crash.

- Owned resources stay acquired. Nothing releases them.
- The writer receives EOF on its standard input. If the close frame has
  arrived, the close transaction finishes as above. Otherwise the writer
  applies the complete frames that it has already read and discards a partial
  trailing frame. It writes a storage-close record that says
  `eof_without_close` and creates no identity record. Then it exits. That
  directory is never complete.
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

The design review added three rules, and each rejects one alternative.

- **Final outcome in the result.** The result precedes its own
  acknowledgement and the close. A persisted result therefore holds a
  snapshot, and only the in-memory outcome holds the final publication state.
- **Identity request before close.** That order leaves the storage-close
  record outside the identity listing. The retention verifier then rejects a
  healthy directory.
- **Waiting for stream EOF.** An escaped descendant can hold a write end
  forever. The final drain reads only buffered bytes, and an open writer fails
  the receipt.

No ADR records these choices. The diagnostic controller is an investigation
harness, so the choices are not hard to reverse.

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

Four more controls cover the rules that the design review added. Each must
fail on a writer build that lacks the rule that it names.

- **Buffered output at exit.** A fixture writes a known stdout of 8 KiB and
  exits. The control delays every drain turn until the leader reap. The
  receipt says `eof`, and its expected-output match passes. Without the final
  drain, the match fails.
- **Inherited writer.** A fixture starts a descendant in a new session. The
  descendant keeps the stdout write end open and never writes. The receipt
  says `writer_open` and fails. Teardown adds no wait for EOF.
- **Close record.** A healthy run passes the completion conditions. A copy
  without the storage-close record fails them. So does a copy with a changed
  storage-close record. A build that computes identity before the
  storage-close record fails the healthy case.
- **Result snapshot.** The result's publication field says `pending`, and no
  digest entry names the result. The control kills the writer after the
  result frame and before close. No identity record exists, and the publication
  outcome is `incomplete`. The retained directory is incomplete.

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
