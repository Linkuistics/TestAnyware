# storage-client-k165

## Goal

Build the controller side of the storage channel. The controller can start the
writer, wait for readiness and send frames without blocking a turn.

## Context

The spec sections are "Ownership", "Phases and bounds" (P0 and P1),
"Channel contract" and "Test seams and required controls". The turn scheduler
in `turns.py` already accepts read and write descriptors as readiness sources.

## Done when

- The client accepts the writer command as an internal parameter. The default
  is the real writer from `storage-writer-k164`.
- The client spawns the writer before ownership starts. If readiness does not
  arrive within 5 seconds of the spawn, the client sends KILL and reaps the
  writer within 2 seconds. Storage is then `unavailable`.
- The outbox is a turn. It writes at most 256 KiB in each turn and reads at
  most 64 KiB of acknowledgements in each turn.
- On `EAGAIN`, the outbox keeps the frame. On `EPIPE`, the outbox fails.
- The outbox cap defaults to 64 MiB. Drops are counted by record name.
- The writer process receives only its standard descriptors and its
  run-directory descriptor.
- Tests drive the client against the real writer and a writer stopped with
  SIGSTOP. The stopped writer does not block a turn.
- A wrapper command can run the real writer with one storage fault. The later
  controls use this wrapper.
- The full suite passes, and `execute()` does not change.

## Decisions (running log)

- The client is `StorageClient` in `experiments/exec-reliability/storage_client.py`.
  Its first argument is the writer command. The default is `WRITER`, which runs
  `writer.py` in isolated mode. The readiness bound defaults to 5 seconds. The
  client refuses a bound above that ceiling. The outbox cap defaults to 64 MiB.
- `spawn()` uses one `Popen` with the spec's spawn shape: its own session, the
  working directory `/`, a null standard error and `close_fds`. Both pipes are
  non-inheritable and above descriptor 2 in the controller. The parent ends are
  nonblocking. A `Popen` subclass has an empty destructor, so only the
  client's exact-PID `waitpid(WNOHANG)` reaps the writer.
- `start()` spawns, queues the setup frame and drives turns until readiness
  is decided. On a missing, late or failed setup acknowledgement, storage
  becomes `unavailable`. The client then closes both pipes, counts the queued
  frames as abandoned and sends KILL to the exact PID. Turns then reap the
  writer within 2 seconds. If the reap misses the grace, the writer becomes a
  residual, and the client never signals it again.
- Storage states are `starting`, `ready`, `unavailable`, `failed` and `closed`.
  `first_failure` names the first cause. The causes are `EPIPE`, another errno
  name, `ack_eof`, `protocol`, `writer_exit` and `writer_lost`. An `ok` close
  acknowledgement makes storage `closed`. A later writer exit then changes
  nothing.
- `enqueue()` serialises the frame at once and never blocks. A frame over the
  cap is dropped and counted in `drops` by record name. A frame queued after
  failure is counted in `abandoned` by record name. Frames that a failure
  removes from the outbox also go to `abandoned`. The cap check comes before
  the sequence number is used, so a drop leaves no gap in the sequence.
- One turn writes at most 256 KiB. A frame can stop part-way through. `EAGAIN`
  keeps the frame and its offset. A turn reads at most 64 KiB of
  acknowledgements. Every acknowledgement must name a sent sequence number and
  its operation, and a duplicate is a protocol failure.
- Reap and drain are separate events. A reap closes only the frame pipe. The
  turn then reads the last acknowledgements until EOF, or until `EAGAIN`,
  because the dead writer finished all its writes before it exited. So a close
  acknowledgement written just before the exit still counts. A grandchild that
  holds the acknowledgement pipe cannot delay the settle. The first version
  closed both pipes at reap time. The test
  `test_close_acknowledgement_survives_a_reap_in_the_same_turn` fails on that
  version.
- `writer_faults.py` keeps its EIO faults and adds three. `stall:NAME` sends
  SIGSTOP to the writer when it opens NAME. `exit:NAME` ends the writer with
  status 70 at the same point. The name `@start` applies either fault before
  the first frame. `report:PATH` appends a descriptor snapshot at start and
  before each acknowledgement. The descriptor test uses it. The writer holds
  0, 1 and 2 at start, and one directory more after setup. An inheritable
  pipe in the controller does not reach the writer.
- The stopped-writer test sends SIGSTOP to a ready writer and queues 8 MiB. It
  checks that each of 50 turns takes less than 0.1 s, that the pipe fills to
  `EAGAIN` and that no frame is dropped. After SIGCONT the frame lands intact.
- `execute()` does not use the client yet. The nine required controls keep
  their markers. `storage-cutover-k166` connects the client to `execute()`,
  adds the intent gate and switches `StorageFault` to the wrapper faults.
- `storage_client.py` joins the strict pyright list, and
  `test_storage_client.py` joins the test paths. The suite gives 1054 passed
  and 9 expected failures.
