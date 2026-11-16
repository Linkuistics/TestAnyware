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

## Review evidence

Inspected producer commit `c25003fb54c752aab875613b238481faf97aa840`
(`storage-cutover-k166`) against the current source, its recorded decisions,
and the storage spec. This review ran no tests, builds, lint or formatting
commands and changed no production or test code. Producer verification claims
were read as recorded evidence, not independently reproduced.

Tier 2 graph coverage was checked for the controller, client, writer, fault
wrapper, retention, scheduler and changed tests. The graph generation is
`2026-11-15T02:19:52Z`: the controller has `metadata_changed` freshness and
the other experiment files report `not_tracked`. Although there are no
recorded parse gaps, the graph's controller snippet is stale. The findings
below therefore rely on direct inspection of the current source and committed
diff, not on graph completeness.

## Finding F1 — rejected control frames bypass sticky storage failure

**Priority: medium; correctness and acceptance.**

At `experiments/exec-reliability/controller.py:1215`, `close()` ignores a
`None` return from `request_close()` and starts an unconditional settlement
wait. At `experiments/exec-reliability/storage_client.py:229`, that request
is an ordinary enqueue. Cap rejection at
`experiments/exec-reliability/storage_client.py:263` increments the drop count
and returns `None`; it does not fail storage or close the frame pipe.

The controller enqueues the result and immediately requests close, with no
intervening drain turn (`experiments/exec-reliability/controller.py:1384`).
If the result fits but leaves less room than the encoded close frame needs,
the close frame is dropped. A responsive writer drains all accepted frames
and then waits for more input: it receives neither close nor EOF. The client
remains ready and cannot settle, so `execute()` waits forever. This requires
queue pressure, not a stalled filesystem or stopped writer. The explicitly
deferred publication deadline in `storage-publication-k168` does not explain
the missing `outbox_drop` failure transition in this slice.

The direct subjects-after enqueue has the same missing failure transition at
`experiments/exec-reliability/controller.py:1133`. On rejection it calls only
`_stop()`, which records the still-ready client state as the reason. It leaves
`first_failure` unset and the transport open. By contrast, the record path at
`experiments/exec-reliability/controller.py:1073` calls
`client.fail("outbox_drop")` before stopping forward progress.

Integration should verify cap rejection for both direct control-frame paths
and make their failure reporting as sticky as ordinary record drops. Include
the case where the result fits but close does not, with a responsive writer;
it must not need a watchdog to escape a permanently open input pipe. This is
a source-derived finding, not an executed reproduction.

## Finding F2 — known intent failure identifiers disappear from the snapshot

**Priority: medium; result-snapshot acceptance.**

At `experiments/exec-reliability/controller.py:1162`, the snapshot obtains
every first-failure identifier exclusively from `client.failure_ack`. The
missed/late intent path at `experiments/exec-reliability/controller.py:1120`
calls `client.fail("intent_gate")` without preserving the intent's sequence,
operation or record name. A missed gate has no acknowledgement; a late `ok`
acknowledgement is not an error acknowledgement either. Both therefore produce
a first-failure object with null sequence, operation and record, even though
the gate knows the exact queued frame. An outbox drop similarly loses its
known operation and name; it need not invent a sequence for an unqueued frame.

The spec's Result snapshot section requires the first failure's sequence,
operation, record name and errno, and this producer explicitly owns that
snapshot. An errno can be absent for a timeout, but the available intent
identifiers should not be discarded merely because failure was detected by
the controller. The separate `intent_gates` phase map does not populate the
required first-failure fields. The returned in-memory snapshot has this defect
even when failure prevents writing the result itself.

Integration should retain the first failure's available context at detection,
without allowing a later error acknowledgement to replace it. Verify the
snapshot for both a missed gate and a late successful acknowledgement, as
well as the already-supported writer error acknowledgement.

## Finding F3 — the switched overflow control cannot exercise its named gap

**Priority: medium; control acceptance.**

At `experiments/exec-reliability/test_storage_controls.py:559`, the switched
overflow fixture stops the writer at `flood.json`, the receipt sent after the
flood call. In this slice the flood stream goes directly to a file, so it
cannot fill the outbox. The receipt itself is small, and the controller may
start the later call without any outbox drop. The current expected failure is
therefore a blocked close or the later-marker assertion, not evidence that an
outbox overflow stopped forward progress.

This trigger also sits after the stream it is meant to obstruct. Routing the
stream through the writer in `pipe-capture-k167` does not by itself make a
stop at the later receipt backpressure that preceding stream. A control that
will establish overflow needs the writer held before the flood is drained.
The current marker names only `storage-publication-k168`, obscuring the
independent missing stream-capture prerequisite.

There is also a pre-existing defect in the retained assertions: the call at
`experiments/exec-reliability/test_storage_controls.py:563` supplies two
arguments to `assert_killed_publication`, whose definition at
`experiments/exec-reliability/test_storage_controls.py:455` requires three,
including `worker_closed`. Once earlier failures are repaired, this raises
`TypeError`, which the marker deliberately does not accept. The producer did
not introduce that arity error, but its explicit control acceptance requires
checking the retained assertions and expected-failure causality.

Integration should reconcile the control's injection point, remaining-slice
ownership and worker-close assertion so it can eventually demonstrate a real
drop and the promised unchanged teardown. It need not implement pipe capture
or bounded publication in this repair. These conclusions come from source
inspection; the control was not run here.

## Other contract checks

- The setup carries plan bytes and subjects-before hashing to the writer.
  Intents, receipts and the result use the record outbox. Subject-after hashing
  also runs in the writer. Call stream files remain the declared exception.
- Intent waits run as turns alongside the inspection worker. A failed gate
  prevents acquisition and does not add an unacquired resource to the teardown
  list. Already-attempted resources still receive cleanup, disposal and
  inspection, followed by worker close. Ordinary record drops and observed
  storage failures stop subsequent acquisitions and workload calls; F1 covers
  the direct-enqueue exceptions.
- The snapshot keeps pending publication, controller byte lengths and digests
  for serialized records, including rejected records, excludes result/close
  from those digests, and includes the readiness pin for subjects-before.
  F2 identifies a field-level first-failure defect.
- `verify()` now requires completion for every directory. The recorded legacy
  decision and its test consistently reject directories without storage-close.
- The execute-level `finally` owns writer settlement after exceptions, and
  the control watchdog releases stopped writers before unwinding into it.
  The client fixture fails with `pytest.fail` for unreaped writers, preventing
  an expected-failure AssertionError marker from hiding that leak. These are
  inspected safeguards, not independent runtime evidence.

## Decisions (running log)

- F1–F3 meet the agreed acceptance/material-risk threshold and warrant an
  integration before `pipe-capture-k167` consumes this cutover. The integration
  remains a sibling under `evidence-writer-k162`: it has a finite handoff of
  triaged repairs and regression evidence under the existing contract.
- Keep stream routing with `pipe-capture-k167` and the general bounded close
  protocol with `storage-publication-k168`. Neither acknowledged omission is
  a separate finding against this producer.
