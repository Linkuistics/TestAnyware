# storage-publication-k180

**Reviews:** storage-publication-k168

## Goal

Review the publication slice adversarially before node k162 closes.

## Context

Read the producer commit for `storage-publication-k168`. The changed code is
in `experiments/exec-reliability/controller.py`: `Plan.publication_window`,
`_validate()`, `_RunStorage.begin_publication()`, `subjects_after()`,
`_bounded_turn()`, `close()`, `finish()`, `_publication_outcome()`,
`outcome()`, `RunOutcome`, `execute_outcome()` and `execute()`. The client
changes are `kill()`, `expect_exit()` and `_reap_due()` in
`experiments/exec-reliability/storage_client.py`. The changed tests are
`test_storage_controls.py` and `test_storage_client.py`.

The spec is `docs/specs/diagnostic-evidence-storage.md`. Its sections "Phases
and bounds" (P3), "Terminal outcome", "Timing conditions" and "Test seams and
required controls" give the contract. The producer leaf states the done
conditions and records each decision.

## Done when

- Check that publication starts after the last teardown action and the
  settled worker close. Check that P is S plus W, and that W is at most
  10 seconds.
- Check that the subjects-after cutoff is S plus W/2, and that a late digest
  is `unavailable:storage`.
- Check that the controller sends one close frame and drains the outbox
  until P without a block.
- Check that an unfinished writer receives KILL at P. Check that each path
  sets the reap deadline to P plus 2 seconds. Check that a residual never
  blocks the controller.
- Check that each publication value matches the spec's "Terminal outcome"
  definition. Look for a path that reports `complete` without an `ok` close
  acknowledgement before P.
- Check that the verdict requires `complete`, and that `execute()` still
  returns the run directory as a `Path`.
- Check that the audit hook control covers file system and import events.
  Its window must run from readiness to the end of publication.
- Check that each control in `test_storage_controls.py` passes without a
  marker. Check that each review control fails on the build that lacks its
  rule.
- Check that the foundation report subsection "Delivered behaviour (k162)"
  matches the delivered code and states its limits.
- Report each finding with its file, line and a failure scenario.

## Notes
