# storage-client-k174

**Reviews:** storage-client-k165

## Goal

Review the controller storage client adversarially before the cutover slice
connects it to `execute()`.

## Context

Read the producer commit for `storage-client-k165`. The client is
`experiments/exec-reliability/storage_client.py`. The fault wrapper is
`experiments/exec-reliability/writer_faults.py`. The tests are
`experiments/exec-reliability/test_storage_client.py`. The turn scheduler is
`experiments/exec-reliability/turns.py`.

The spec is `docs/specs/diagnostic-evidence-storage.md`. Its sections
"Ownership", "Phases and bounds" (P0 and P1) and "Channel contract" give the
contract. The producer leaf records each decision about the storage states,
the failure causes and the wrapper faults.

## Done when

- Check that no turn can block on a stopped, slow or dead writer. Include a
  partial frame, a full pipe and a writer that exits while frames remain.
- Check that one turn writes at most 256 KiB and reads at most 64 KiB.
- Check that `EAGAIN` keeps the frame and its offset.
- Check that the client sends KILL only to the PID from its own spawn.
- Check that the client sends KILL only while that PID is unreaped.
- Check that only the client's exact-PID wait reaps the writer.
- Check that a readiness failure ends with storage `unavailable`, a KILL and
  a reap within 2 seconds. Include a late `ok` acknowledgement.
- Check that the client counts each drop and each abandoned frame by record
  name.
- Check that a drop leaves no gap in the sequence numbers.
- Check that the reap-then-drain order cannot lose a close acknowledgement or
  hold a turn open. Include a process that holds the acknowledgement pipe.
- Check that the writer receives only its standard descriptors and its
  run-directory descriptor, even when the controller holds inheritable pipes.
- Check that the wrapper faults are enough for the nine required controls.
  Name any fault that `storage-cutover-k166` or `storage-publication-k168`
  must add.

## Notes

`execute()` does not use the client yet, so the nine required controls keep
their markers.

## Decisions (running log)

- The lost-child signal path is a material process-safety finding against the
  explicit Ownership contract. It warrants integration before the client is
  consumed by `storage-cutover-k166`.
- Keep that integration as a sibling under `evidence-writer-k162`: the finite
  handoff is a client ownership repair with regression evidence, and it needs
  no context boundary beyond this node. Insert it before the first later live
  sibling, `storage-cutover-k166`.
- Error-acknowledgement forward stop, the intent gate and publication outcome
  accounting remain with their existing cutover/publication owners. Their
  absence from this unconnected client is not a separate review finding.

## Review evidence

Inspected producer commit `c35af351693a45206b1bf5167097780339c74cb1`
(`storage-client-k165`) against the current source and the storage spec's
Ownership, P0/P1, Channel contract and required controls. Read
`storage_client.py`, `writer_faults.py`, `test_storage_client.py` and
`turns.py`, plus the writer acknowledgement path and the control fixtures.
The graph generation is `2026-11-15T02:19:52Z`; coverage reports these new
experiment files as `not_tracked`, so conclusions use direct source inspection.
No tests, builds, lint or formatting commands were run, and no production or
test code was edited. The producer records 1054 passing tests and nine expected
failures; those are producer evidence, not a result independently reproduced by
this review.

## Finding F1 — lost writer ownership still permits KILL

**Priority: high; acceptance and process safety.**

At `experiments/exec-reliability/storage_client.py:467–473`, an exact-PID
`waitpid` raising `ChildProcessError` sets `residual = True`, fails storage
with `writer_lost` and releases the pipes. This branch explicitly recognises
that another reaper took the child. It leaves `pid`, `reaped_at` and
`killed_at` unchanged. The `_kill()` guard at lines 450–457 checks only the
two timestamps and whether a PID exists; it does not reject lost ownership.

This produces an immediate unsafe path while readiness is pending:

1. `advance()` calls `_reap()` at line 291.
2. The lost-child branch calls `_fail("writer_lost")`; pending readiness
   becomes failed and storage becomes `unavailable` (lines 421–440).
3. The same `advance()` reaches lines 298–299. The PID still exists in the
   object and `reaped_at` is still `None`, so it calls `_kill()`.
4. `_kill()` sends `SIGKILL` to the former numeric PID even though the child
   is no longer waitable by this client. That PID may have been reused.

After readiness the same ownership loss makes storage `failed`, but a later
public `kill()` (lines 229–231) can still take the unsafe path. This contradicts
the spec's requirement to signal only the spawned, unreaped writer, and the
branch's own comment that the client cannot signal it again. A missing local
reap timestamp is not evidence that the original process is still owned.

Integration should rederive this path and make ownership loss terminal for
signalling. Its regression evidence should cover `ChildProcessError` during
pending readiness and after readiness, including later explicit `kill()` and
repeated turns. Assert that no signal follows ownership loss, while failure
state, pipe release, abandoned-record counts and bounded settlement remain
correct. `test_storage_client.py` currently has no lost-child/no-later-signal
case. This is a source-derived finding; it was not executed in this session.

## Other contract checks

- The parent pipe ends are nonblocking. Write work is capped at 256 KiB and
  acknowledgement reads at 64 KiB per `advance()`. Partial writes update the
  frame offset; `EAGAIN` returns without discarding it. The stopped-writer test
  covers a full pipe, repeated bounded turns and intact delivery after resume.
- Failure abandons queued frames by record name; rejected enqueues are counted
  by record name. Cap rejection precedes sequence allocation. Existing tests
  cover the cap and the absence of a sequence gap.
- The normal signal path uses the PID recorded from the client's spawn, never
  a PID from an acknowledgement or a process group. Reaps use only
  `waitpid(self.pid, WNOHANG)`, and the Popen destructor does not reap. F1 is the
  exception to the signal-ownership claim.
- Missing or failed readiness closes the channel and initiates KILL plus a
  two-second reap deadline. A setup `ok` processed at or after the readiness
  deadline is rejected in `_accept_ack()` before storage can become ready.
  A residual after the kill deadline cannot receive another signal because
  `killed_at` is already set. The tests cover missing readiness, but contain no
  dedicated late-`ok` case; the late-ack conclusion is from source inspection.
- Reap closes only the frame pipe. Acknowledgements then drain within the read
  budget, preserving a close acknowledgement written before exit. After reap,
  `EAGAIN` settles a pipe held open by a descendant without waiting for EOF.
  Both cases have focused producer tests. Writer death with queued frames
  fails storage and counts the remaining frames as abandoned.
- Spawn uses isolated Python, a separate session, cwd `/`, null stderr and
  `close_fds=True`. Parent pipes are non-inheritable and promoted above the
  standard descriptors. The descriptor test holds inheritable pipes in the
  controller and inspects the real writer through `report:PATH`.

## Required-control handoff

The wrapper is a useful base, but it does not yet supply every fault needed
by the nine controls. These are existing cutover/publication obligations, not
additional findings or reasons to expand this client repair.

| Control | Available seam and remaining owner |
| --- | --- |
| Stalled writer | `stall:NAME` stops on a named open after readiness; publication must drive the existing teardown and cutoff assertions. |
| Blocking storage call | Add a real first-P2 open on a FIFO without a peer for `storage-publication-k168`. `stall:NAME` runs after the real open and cannot establish this control. |
| Storage error | `storage-cutover-k166` needs coordinated removal of run-directory write permission after readiness and preservation of the real `EACCES`. Injected EIO does not satisfy this control. |
| Writer death | `exit:NAME` supplies death on a selected P2 record; cutover must retain observation, exact-PID reap and unchanged teardown assertions. |
| Outbox overflow | Named stall plus a small cap supplies the mechanism; publication must coordinate the stop before the output flood and verify record-name drops and forward stop. |
| Readiness failure | `stall:@start` withholds readiness; cutover must retain the empty ledger, no-worker and deadline-plus-two-second reap assertions. |
| Intent gate | `stall:intent-2.json` supplies the persistent fault; cutover also needs a controlled resume for its temporary-stall case, while publication owns the persistent cutoff case. |
| No controller storage calls | Keep the controller audit hook through publication; no new writer fault is needed. |
| Inherited state | `report:PATH` snapshots the writer; the later pipe/control slices must also retain fixture descriptor reports and verify both directions of descriptor isolation. |
