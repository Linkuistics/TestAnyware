# 22. The TestAnyware app admits macOS guests through one ordered queue

Date: 2026-10-25

## Status

Accepted. Recorded in grove `gh-issue-7-vm-queue`, leaf `admission-queue-k2`.
The area's spec is `docs/specs/admission-queue.md`. The requirements are in the
decision log of the leaf `plan-k1` of the same grove.

## Context

Apple allows two macOS guests to run at the same time on one Mac.
Virtualization.framework refuses a third with
`VZErrorVirtualMachineLimitExceeded`, and tart reports it as "The number of VMs
exceeds the system limit". Concurrent groves and agents start macOS guests
through the TestAnyware app (ADR-0004), one app for each user (ADR-0019).

Before this decision, `vm start --wait` retried `tart run` every 3 s, and
whichever waiter retried first after a slot freed got it. The contract amendment
of 2026-10-19 left a queue out. It found that only the framework's refusal proves
that no slot is free. It also feared that a queue would serialise starts that
the limit does not block.

The sources show why no process can count every guest of a Mac:

- tart builds the list in its refusal from the VMs of its own `TART_HOME`. It
  applies no OS filter (`Sources/tart/Commands/Run.swift` of
  [tart](https://github.com/openai/tart), lines 574 to 595 at `95a0dac`).
  `tart list` reads the same storage.
- tart treats a VM as running while its `tart run` holds an fcntl lock on the
  VM's configuration (`VMDirectory.swift` and `PIDLock.swift` at `95a0dac`).
- The kernel keeps the count of running guests
  ([Grymalyuk, 2023](https://khronokernel.com/macos/2023/08/08/AS-VM.html)). No
  interface reads it, and no source shows how to tell a macOS guest from a Linux
  guest by its process.
- Linux guests do not count against the limit
  ([tart discussion 507](https://github.com/openai/tart/discussions/507),
  [gitlab-tart-executor issue 93](https://github.com/cirruslabs/gitlab-tart-executor/issues/93)).

The tools that run tart at scale use a static count of two:

- Orchard's scheduler serves pending VMs by creation time. A later VM can pass
  one that does not fit, and a refused `tart run` makes the VM fail
  (`internal/controller/scheduler/scheduler.go` of
  [Orchard](https://github.com/openai/orchard) at `22bd63c`).
- An Orchard worker that restarts removes VMs. It does not adopt them.
- The CI runners on tart fail the job on a refusal, or they try again at once.
  None of them serves its waiters in order on the host.

On macOS 26.x the count of the framework sometimes stays up after a guest stops.
Only a restart of the Mac clears it
([Apple forum thread 830119](https://developer.apple.com/forums/thread/830119),
[tart issue 1217](https://github.com/openai/tart/issues/1217)).

## Decision

**The TestAnyware app keeps one ordered admission queue for the macOS guest
slots of its user. It admits the head while its count of the guests that it can
see shows a free slot. A refusal by the framework proves a guest that it cannot
see, and sends the start back to the head.**

- **The count only refuses.** It holds the app's own VMs, and the starts and
  golden creations that it admitted. It also holds the other running macOS
  guests that tart reports for the user. While tart answers, a full count is a
  fact. A free count only allows one try.
- **The refusal proves.** When the framework refuses an admitted start, the
  start goes back to the head, ahead of each waiter that joined after it. The
  app then tries one start at a time, 3 s apart,
  until the framework accepts one of those tries. So no later start takes a
  freed slot before an earlier start.
- **The queue blocks only what the limit blocks.** Linux and Windows guests never
  join it. When the count shows two free slots, the app admits two starts at
  once.
- **A start holds its slot until its guest is gone.** Golden creation holds its
  slot through an open connection, and so between its boots too.
- **Nothing about the queue is stored.** A waiter is an open connection, so a
  restart leaves none. The app counts again from tart and from the sidecars. A
  start record lets it remove a start that a crash of the app interrupted.
- **`config.rhai` can lower the app's share** of the two slots to one or zero
  (ADR-0021).

## Considered options

- **The retry of today.** Rejected. Waiters are not served in order, so a start
  can wait 600 s while later starts pass it.
- **A static count with no proof, as Orchard and the CI runners use.**
  Rejected. A guest of another user or tool would make each start fail. The
  refusal is the only fact about such a guest.
- **A count of the framework's processes for every user.** Rejected. No source
  shows that one process stands for one guest, or how to tell a macOS guest from
  a Linux guest. A Linux guest would then hold back starts that the limit does not
  block.
- **Try every waiter after a refusal, as before.** Rejected. Two waiters that try
  together can let the second take the freed slot first.
- **A queue file that outlives the app.** Rejected. A waiter is a connection, and
  a restart closes it, so the file could hold only waiters that no client still
  waits for. Durable jobs are issue #10.
- **One queue for every user of the Mac.** Rejected by the human in `plan-k1`. It
  needs a machine-wide service, which ADR-0019 rejected for the Local Network
  grant. A shared lock directory has no order across users, and a crash can leave
  its locks.

## Consequences

- `vm start --wait` of a macOS guest waits in order. A start without `--wait`
  fails at once when the queue holds a waiter, so it cannot pass a waiter.
- A guest that the app cannot see costs one `tart run` every 3 s while starts
  wait, as the retry of today does.
- The stuck count of the framework looks like a guest that the app cannot see.
  The app cannot tell the two apart, and its remedy names both.
- Golden creation asks the app for a slot over the Unix socket, although its
  guest traffic keeps the direct path (ADR-0004).
- The app sees the hold of golden creation, but not its boots. When a guest that
  the app cannot see holds the other slot, a start that the app admits can take
  the free slot first. The next boot of golden creation then fails with
  `VM_LIMIT_REACHED`.
- A restart of the app ends each wait with `APP_UNREACHABLE` and cause
  `quitting`. The client loses its place.
- The protocol of the app gains version 2, for `vm slots` and for the hold of
  golden creation.
- Release 2.6.0 shipped this decision. On 2026-10-26 the live checks of the queue
  passed on the candidate and again on the installed release, on this Mac. The
  spec records the run (`docs/specs/admission-queue.md`, "Test seams").
