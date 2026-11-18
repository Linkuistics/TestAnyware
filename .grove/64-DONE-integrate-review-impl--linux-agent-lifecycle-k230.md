# linux-agent-lifecycle-k230

**Integrates:** linux-agent-lifecycle-k229

## Goal

Triage the review of the Linux lifecycle owner and reconcile the accepted
issues before the Host CLI reader in k218 relies on the record.

## Context

- Review linux-agent-lifecycle-k229 and its committed findings.
- Producer linux-agent-lifecycle-k217, its commit and decision log.
- docs/specs/guest-command-lifecycle.md and ADR-0023/ADR-0024.

## Done when

- Each finding has a source-supported disposition under the integration
  skill's acceptance and material-risk threshold.
- Accepted issues are reconciled with the implementation and its controls;
  any rejected finding or accepted trade-off has a concrete reason recorded.
- task agent:linux:test runs and its result is recorded, including the pinned
  Linux controls. The review was inspection only and did not rerun the suite.
- The resulting record contract and verified handoff are ready for k218.

## Notes

This is a flat sibling of the producer and review. It shares their brief;
placing it immediately before k218 avoids drift in the reviewed source.
Read the findings from the review's commit, rather than treating this task as
a prescribed fix list. Cancellation, retention and transport work remain k220.

## Decisions (running log)

Findings come from review commit be6d55e659. Source lines refer to producer
commit c6084c28da.

- F1, bounded publication: a real issue, accepted in part. The owner sampled
  `finalisedAfterMs` before the SHA-256, the base64 text and the legacy
  decode. A late record could therefore report `withinBounds: true`. The spec
  puts the count and the digest in the worker (section "Capture", step 3).
  - Change: the worker now digests each chunk as it reads it. At the deadline
    the owner sets an abandon flag under the worker lock. The worker then reads
    nothing more after its current read returns.
  - Change: `StreamEvidence` derives its base64 text, and `ExecRecord` derives
    its legacy projection, when the owner builds them. The owner builds them
    before it samples `finalisedAfterMs`, so that time now covers them.
  - Rejected part: the HTTP JSON encoding is reply transport, not publication.
    The spec section "Bounds" says the controller measures each host phase. An
    agent field never replaces that measurement. Directory removal follows
    publication in the spec order, so it stays after the sample.
  - Accepted trade-off: the copy, the base64 text and the legacy decode stay
    linear in the captured prefix. The spec keeps output size unbounded (Out
    of scope). A very large prefix can therefore pass the bound. The record now
    reports that overrun truthfully, through `finalisedAfterMs` and
    `withinBounds`.
- F2, missing-file record validity: a real issue, accepted. The owner now
  opens a read descriptor of each capture file before the spawn. It takes
  `fileBytes` from `fstat` of that descriptor. The reader opens
  `/proc/self/fd/N`, so a command that unlinks its capture path cannot hide its
  bytes. A failed open of a descriptor before the spawn gives `spawn_failed`,
  like a failed capture directory. The schema needs no new variant.
  - Residual: `fstat` of a held regular-file descriptor fails only on kernel
    faults such as ENOMEM. That error propagates as an agent fault, after the
    owner ended the command. The owner never publishes a contradictory
    record for it.
- F3, unobserved tombstone order: a real control gap, accepted. The new
  control `test_no_nonzero_signal_reaches_the_group_after_the_reap` records
  each `killpg` call and the reap. It covers both leader-exit orders and a
  normal exit. It requires each SIGTERM and SIGKILL before the reap, with the
  leader PID still present. After the reap it allows only signal 0.
- New controls: the stalled-capture test now also requires that the abandoned
  worker reads nothing more. `test_the_published_record_needs_no_work_in_proportion_to_its_bytes`
  forbids the digest, base64 and projection functions during serialisation.
  Two unlinked-path tests cover a written stream and an empty stream.
- Discrimination: the producer's owner and models fail four of the new
  controls in the pinned container. A mutant that reaps the leader before
  SIGKILL fails the tombstone-order control at
  `leader-exits-before-sigkill`.
- `task agent:linux:test` on 2026-11-18: ruff passed. The host run gave 31
  passed and 39 skipped, because the host is darwin. The pinned Linux
  container, with `TESTANYWARE_LINUX_CONTROLS=required`, gave 70 passed in
  29.25 s.
- Handoff to k218: the serialised record shape is unchanged, so the 14
  recorded exec-record fixtures stay valid. Every stream from the owner now
  satisfies the rule that `bytes` is less than `fileBytes` for an incomplete
  stream. Cancellation, retention and transport remain with k220.
