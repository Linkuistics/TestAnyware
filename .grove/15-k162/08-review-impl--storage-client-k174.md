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
