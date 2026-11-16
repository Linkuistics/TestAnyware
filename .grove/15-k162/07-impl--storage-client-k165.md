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
