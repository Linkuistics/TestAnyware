# macos-agent-lifecycle-k233

**Reviews:** macos-agent-lifecycle-k219

## Goal

Try to break the macOS lifecycle owner of macos-agent-lifecycle-k219 before
the cancellation leaf macos-agent-cancellation-k221 builds on it. Find each
command whose record does not match what the kernel did. Find each case where
the owner can signal a process outside its group. Find each value of today's
macOS reply that changed for a Host CLI that reads only the legacy fields.

## Context

- Spec `docs/specs/guest-command-lifecycle.md`: sections "Guest command
  lifecycle", "Exec record", "Compatibility and version skew", "Bounds" and
  "Test seams and required controls".
- ADR-0023 and ADR-0024.
- The owner: `agents/macos/Sources/TestAnywareAgent/ProcessRunner.swift`.
- The wire types: `agents/macos/Sources/TestAnywareAgentProtocol/ExecRecord.swift`.
- The mapper: `agents/macos/Sources/TestAnywareAgent/ExecRecordMapping.swift`.
- The server: `handleExec` in `agents/macos/Sources/testanyware-agent/AgentServer.swift`.
- Recorded replies: `agents/macos/Tests/fixtures/exec-records`.
- Controls: `ExecOwnerTests.swift`, `ExecReplyFixtureTests.swift`, the k12
  record judge in `experiments/exec-reliability/outcomes_k12_check.py`, and
  the evidence in `experiments/exec-reliability/evidence/macos-agent-lifecycle-k219`.

## Specific doubts

- The owner reaps the leader, and only the owner may reap it. Check every
  path that ends without a reap, and the late-reap thread.
- The schedule: SIGTERM at T0, SIGKILL by T0 + 1.9 s, the status wait to
  K + 1.4 s and finalisation by K + 1.9 s. Check that no wait can block past
  its bound, including a kqueue error and a stalled capture read.
- The arbitration at the deadline: a leader that exits at the deadline must
  never give `timed_out`.
- The capture worker owns the read descriptors after the deadline. Check that
  an abandoned worker cannot read a reused descriptor or change a record.
- The spawn: `POSIX_SPAWN_CLOEXEC_DEFAULT`, the signal reset and the
  environment. Check what a command inherits from the agent.
- The legacy projection must equal today's macOS values for each outcome, and
  the record must pass every validity rule of the spec.

## Done when

- Each finding names the code, the input and the observed or derived wrong
  result.
- The review reports findings, not fixes.

## Decisions (running log)

- D1. Review the producer commit `d7e121df` by inspection only. Do not run
  tests, builds, lint or formatting. The record fixtures, source and retained
  results are the evidence. The graph CLI failed at startup because an
  unverified generation is active; use direct source inspection and disclose
  that graph freshness and coverage could not be established.
- D2. The kqueue exit event is not a checked wait status. The owner returns
  true for the event and then calls blocking `waitpid`. XNU emits NOTE_EXIT
  before setting SZOMB. Record this as a source-derived risk to the required
  non-blocking reap and K+1.4 s status cutoff, with no runtime reproduction
  claimed.
- D3. The public timeout reaches checked UInt64 multiplication after spawn.
  The JSON value 20000000000 overflows it and terminates the agent. Record
  the reachable arithmetic failure independently of any timing observation.
- D4. The capture deadline stops the semaphore wait only. Acquiring the
  progress lock, encoding base64 and projecting legacy text have no deadline.
  Record the resulting gap in the finalisation bound; the five-byte stalled
  read and 8 MiB all-zero tests do not exercise that work at the cutoff.

- D5. A failed EV_ADD registration other than ESRCH is discarded while its
  kqueue stays valid. A later wait on that empty queue does not take the
  error fallback and can delay a completed command until its full timeout.
  This is a separate source-derived finding about the required kqueue error
  path.

## Findings

The reviewed producer is `macos-agent-lifecycle-k219`, commit `d7e121df`.
The findings below are derived from source. No test, build, lint, formatter or
live command probe was run in this inspection-only review. They require
triage before the later cancellation owner builds on this implementation.

### R1 — P1: NOTE_EXIT takes the owner into a blocking reap before status readiness

Code: `agents/macos/Sources/TestAnywareAgent/ProcessRunner.swift:155-158`,
`:177-180`, `:434-437` and `:558-573`.

Condition: the owner receives NOTE_EXIT while the leader has not yet become
waitable. `waitExit` sets its cached `exited` flag and returns true for any
returned event. It does not check `hasExited` at this transition. The normal
exit branch immediately calls `settleExit`, and the termination branch records
`exitAt` and calls `reap`. Both use `waitpid(pid, &status, 0)`.

This condition is allowed by the kernel implementation, not just by an
invented waiter: Apple's XNU emits NOTE_EXIT in `proc_exit` before it sets the
process state to SZOMB. Between those operations it obtains the parent and
takes its process lock. The proc filter publishes the notification from the
recorded event flags; it does not wait for SZOMB. The wait implementation
handles exit status at SZOMB. See
[XNU kern_exit.c](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/kern/kern_exit.c#L2353)
and [XNU proc filter](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/kern/kern_event.c#L1231),
inspected on 2026-11-18. This establishes a possible ordering; it is not a
measurement of the installed guest kernel.

Derived wrong result: if the leader is delayed between notification and
waitable status, the owner blocks inside `reap` beyond its deadline. In the
termination path it cannot take the K+1.4 s unavailable-status branch or
publish by K+1.9 s. On a normal exit it can also wedge capture finalisation.
The spec explicitly requires every owner reap to be non-blocking; only the
late-reap thread may wait for eventual status. The unavailable-status test
replaces the entire waiter with one that always returns false, so it does not
exercise an early real notification followed by unavailable status.

### R2 — P1: an accepted timeout can overflow after spawn and crash the agent

Code: `agents/macos/Sources/testanyware-agent/AgentServer.swift:428-451`,
`agents/macos/Sources/TestAnywareAgentProtocol/ExecRecord.swift:13-17` and
`agents/macos/Sources/TestAnywareAgent/ProcessRunner.swift:426`.

Input: an attached `/exec` body such as
`{"command":"sleep 30","timeout":20000000000,"id":"overflow"}`.
The value fits the decoded Swift Int and the Host CLI's i64 timeout
(`cli-rs/crates/testanyware-protocol/src/agent_requests.rs:56-59`). The server
passes it to the owner without a range check.

Derived wrong result: the checked multiplication converts it to
20000000000000000000 nanoseconds, above UInt64.max
(18446744073709551615), and traps. This occurs after a successful spawn and
before arbitration or cleanup. The whole agent exits, no exec record is
published, and the newly created session has no surviving owner. Other
commands served by the agent are affected too. This is a reachable input
failure, independent of scheduler or kernel timing. The protocol documents
no upper timeout bound, and the existing owner tests use small timeouts only.

### R3 — P1: finalisation still performs unbounded work after its deadline

Code: `agents/macos/Sources/TestAnywareAgent/ProcessRunner.swift:653-678`,
`:455-466` and `:262-264`; publication continues in
`agents/macos/Sources/testanyware-agent/AgentServer.swift:109-116` and `:452`.

Condition: a large valid text capture is still making progress when the
finalisation deadline expires. For example, a command can write a large
ASCII stdout file before its timeout, with capture still running at K+1.9 s.
Output size is expressly unbounded in the spec.

Derived wrong result: the deadline applies only to the semaphore wait. The
owner next takes an untimed lock that the worker holds during Data growth
and hashing. After obtaining the snapshot, the owner base64-encodes the
entire retained prefix and decodes and trims the entire legacy text on its
own thread. None of that work has a cutoff. Its time and allocations grow
with the prefix, so abandoning the read cannot bound publication to K+1.9 s
or the root's K+2 s ceiling. The same issue applies to the two-second
finalisation bound after a normal exit.

In addition, `finalisedAfterMs` is sampled before capture-file removal,
continuation delivery, reply mapping and JSON/body construction. Those steps
can delay the published reply after the measurement used by `withinBounds`.
The record can therefore understate publication time. This is a source-derived
missing bound, not a claimed measured overrun on the producer's machine.

The retained controls cover an 8 MiB all-zero file that finishes reading and a
five-byte stream whose read stalls before producing a prefix. They do not
exercise a large retained text prefix at the cutoff or the publication work
after it. The stalled-read test also accepts up to 2300 ms, above the spec's
2000 ms outcome-to-record bound.

### R4 — P2: a failed proc registration leaves a valid queue with no wake source

Code: `agents/macos/Sources/TestAnywareAgent/ProcessRunner.swift:128-137`
and `:144-167`.

Condition: `kqueue()` succeeds, but the EV_ADD call fails with an error other
than ESRCH, such as ENOMEM, while the leader still runs. The registration
error is discarded. Resource failure in registration is supported by
[XNU knote attachment](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/kern/kern_event.c#L6448).

Derived wrong result: the first `hasExited` can return false, then the owner
waits on a valid empty queue with a timeout equal to the entire command
budget. A command that exits just afterwards, such as `sleep 0.1` with a
30-second timeout, produces no queue event. The subsequent `kevent` returns
0 only at the deadline, so the `count < 0` polling fallback never executes.
The reply is delayed for the full budget despite a finished command. This
violates the required prompt completion under the kqueue error path and can
consume the root's entire ordinary-observation ceiling. The current tests
have no failed-registration control.

## Evidence and limits

- Directly inspected the owner, mapper, wire types, server diff, owner and
  fixture tests, fixture validity checker, changed cleanup fixture, Rust
  request/reader boundaries, k12 judge, spec and cited ADRs.
- The producer's retained `agent-test-20/results.txt` reports 20 successful
  runs with unchanged subjects. `judgements-02.txt` reports faithful outcomes
  at all 16 record-seam cases. These are the producer's recorded results,
  not new verification runs, and do not exercise R1-R4's conditions.
- The timeout branch sends TERM and KILL before its explicit reap. The capture
  worker retains its own read descriptors and checks abandonment under the
  same lock as prefix updates. Those inspected paths do not supply an
  additional finding about signalling a reused group or mutating a published
  prefix. No general hostile-code containment claim is made.
- Both `index_status` and the attempted `check_index_coverage` failed before
  serving a query: `CBM CLI could not start because a pre-coordination or
  unverified CBM generation is active; close all CBM sessions and commands,
  then retry`. No graph generation, freshness or coverage result was
  available. The conclusions are bounded to directly inspected source and
  the explicitly cited upstream kernel implementation.
- The producer log names `final.sh`; it is not present at the named evidence
  directory. The review relies on retained result files rather than that
  absent script. No missing evidence has been reconstructed.
- Cancellation, duplicate-id retention, downloads and shutdown remain k221's
  charter. This review does not treat those planned additions as defects in
  k219. The ten-clone guest acceptance remains 0/10, unavailable.
