# storage-controls-k170

**Reviews:** storage-controls-k163

## Goal

Review the nine storage controls adversarially before the writer slices rely on
them as their acceptance gates.

## Context

Read the producer commit for `storage-controls-k163`. The controls are in
`experiments/exec-reliability/test_storage_controls.py`. The task
`diagnostic:storage-controls` runs them. The producer leaf records the observed
failure of each control and its cause. The spec is
`docs/specs/diagnostic-evidence-storage.md`, in "Test seams and required
controls".

On this build no writer exists. Each fault fixture wraps the storage functions
that the controller calls today. A stall is a FIFO open that a signal
watchdog interrupts. Two adapters, `storage_outcome` and
`writer_descriptors`, return `None` until a later slice binds them.

## Done when

- Check that each control asserts the final behaviour of the spec and not a
  property of the fixture. A control that passes for the wrong reason after the
  repair is a finding.
- Check that each recorded failure comes after the fault. It must be a
  behavioural assertion or a watchdog expiry, not a setup error.
- Check that the marker of each control names the slice that the node brief
  gives it.
- Check that the fixtures can switch to the writer wrapper without a change to
  the assertions. Name any assertion that a later slice must change.
- Check the watchdog bounds against the phase bounds that the controls declare.
- Check that a stalled control leaks no resource marker or process.

## Notes

Four controls use watchdog expiry as their recorded failure. They are stalled
writer, blocking storage call, outbox overflow and intent gate. Readiness
failure also uses it, with a shorter bound. Judge whether a watchdog expiry is
specific enough to show the gap that each marker names.

The no-controller-storage-calls control uses an audit hook that stays
installed for the rest of the process. It records only while a log is set.
