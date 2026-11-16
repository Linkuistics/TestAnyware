# storage-cutover-k176

**Reviews:** storage-cutover-k166

## Goal

Review the storage cutover adversarially before the pipe capture slice builds
on it.

## Context

Read the producer commit for `storage-cutover-k166`. The changed code is in
`experiments/exec-reliability/`: `controller.py`, `storage_client.py`,
`writer_faults.py` and `retention.py`. The changed tests are
`test_storage_controls.py`, `test_storage_client.py` and `test_retention.py`.

The spec is `docs/specs/diagnostic-evidence-storage.md`. Its sections "Phases
and bounds" (P2), "Forward stop and failure reporting", "Result snapshot" and
"Completion authority" give the contract. The producer leaf states the done
conditions and records each decision.

## Done when

- Check that `execute()` sends the plan, the subject digests, the intent
  records and the result through the writer. Name any run record that the
  controller still writes itself, apart from the call streams.
- Check that each acquisition waits for the acknowledgement of its intent
  record for at most the intent gate. Check that the wait runs as scheduler
  turns and keeps the worker and workload timers runnable.
- Check that every storage failure in the spec sets the forward stop. Include
  an error acknowledgement, `EPIPE`, an early writer exit, an outbox drop and
  a missed or late intent gate.
- Check that the forward stop blocks later acquisitions and workload calls.
- Check that the forward stop leaves cleanup, disposal, inspection and worker
  close unchanged. Include a resource whose intent gate fails.
- Check the storage section of the result against the spec field by field.
  Include the cutoff, the frame counts, the first failure, the digest entries
  and the writer-reported subjects-before pin.
- Check that `verify()` applies the completion check to every run directory.
  Check the decision for run directories from before the writer.
- Check that each switched control keeps its assertions. Check that each fault
  reaches the writer at the record that the control names.
- Check that each control that keeps its marker fails for its own gap. Check
  that it leaves no live writer and no stopped fixture process.
- Check that a test exception, a watchdog expiry or a controller exception
  cannot leave the writer unreaped.

## Notes

The close wait has no bound until `storage-publication-k168`. The call
streams stay files until `pipe-capture-k167`.
