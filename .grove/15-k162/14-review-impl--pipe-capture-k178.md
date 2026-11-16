# pipe-capture-k178

**Reviews:** pipe-capture-k167

## Goal

Review the pipe capture adversarially before the publication slice builds on
it.

## Context

Read the producer commit for `pipe-capture-k167`. The changed code is in
`experiments/exec-reliability/controller.py`: `_Stream`, `_Capture`, `_call()`,
`_supervise()`, `_RunStorage.stream()` and the teardown executable check in
`_validate()`. The changed tests are `test_storage_controls.py`,
`test_retention.py` and `test_controller.py`.

The spec is `docs/specs/diagnostic-evidence-storage.md`. Its section
"Workload and teardown descriptors" gives the contract. The section "Channel
and records" gives the frame and outbox rules. The producer leaf states the
done conditions and records each decision.

## Done when

- Check that the controller opens no file for a call after readiness. Check
  that it keeps no write end of a stream pipe after the spawn.
- Check that every pipe end is non-inheritable in the controller. Check that
  the workload, a teardown call, the worker and the helper receive no writer
  pipe end.
- Check that a turn reads at most 64 KiB from each stream. Check that the
  running length and SHA-256 digest cover exactly the bytes in memory.
- Check that the final drain runs after the termination decision and the
  leader reap. Check that it makes at most 16 reads on each stream and never
  waits.
- Check the three completeness values. Check that only `eof` passes, and that
  `writer_open` and `cutoff` fail the call.
- Check that the stdout match reads only the bytes in memory.
- Check the stream frames. Each frame must stay within `writer.MAX_PAYLOAD`,
  and each stream must have one create frame before its append frames.
- Check that an outbox drop on a stream record fails storage and sets the
  forward stop. Check that a stream frame after storage failure counts as
  abandoned.
- Check the digest entry of each stream in the storage section. It must come
  from the bytes in memory, including for a dropped stream.
- Check the teardown executable rule. It must refuse a relative path and a
  path under the evidence root by lexical comparison, with no file system
  access. Look for a path that escapes the comparison, such as `..` segments
  or a root with a trailing separator.
- Check that each new review control fails for its own gap on a mutated build.
  Check that the inherited writer control leaves no live descendant.
- Check that the rewritten retention tests still cover each property that the
  file-backed tests covered.

## Notes

The close wait has no bound until `storage-publication-k168`, so the outbox
overflow control keeps its marker. `execute()` still calls
`evidence_root.resolve()` before the validator runs. That call predates this
slice. The producer did not measure the capture under a stalled writer with
large output beyond the overflow control.
