# complete-worker-scheduler-k81

## Goal

Record signal-0 probe timing separately from the validated table-query interval in the live group-observation receipt.

## Context

The full original aggregate charter, prior slice decisions and every remaining requirement now live in the flat sibling complete-worker-scheduler-k83. Read ancestor briefs and docs/specs/diagnostic-process-inspection.md. Keep k28/k17 open. This slice is independently useful evidence: consumers can distinguish a table observed by the helper from the later controller permission probe. The existing overall started/finished interval describes the whole controller operation; validated query timestamps already survive under helper.worker.published_times.

## Done when

- GroupObservation records nullable probe_started_monotonic and probe_finished_monotonic around only the attempted killpg(pid, 0), including successful and failed attempts; skipped probes have neither timestamp.
- Preserve validated table-query times and overall operation timing. Explain the three intervals and avoid claims of an atomic process snapshot or privilege proof.
- Exercise success, EPERM on live and zombie-only tables, ESRCH, deadline expiry during the probe, invalid tables and suppressed observations. Retain RED/GREEN controls, a real owned-process query/probe receipt, before/after subject digests and final formatting/lint/typecheck/full-suite outcomes, without altering prior evidence.
- Schedule review-impl of this slice before k83. No persistent worker/shared scheduler or aggregate runtime completion is claimed; every remaining original requirement is owned by k83.

## Decisions (running log)

- 2026-11-15: Flat sibling decomposition isolates the independently observable probe interval. k83 retains the full original charter byte-for-byte apart from its handle, with all outstanding runtime/control/review obligations. This session changes receipt evidence only, preserving signal and ownership behavior.
- 2026-11-15: Use two nullable monotonic timestamps; set start immediately before killpg and finish in finally. Null/null represents no attempt. The table interval remains helper.worker.published_times.started_monotonic through finished_monotonic, already checked by the interim transport; the outer observation timestamps keep their whole-operation meaning. Independent adversarial challenge is routed to a separate review-impl leaf under Grove execution policy.

- 2026-11-15: RED controls in evidence/probe-intervals-k81/red-02 failed individually on the missing probe field (ten selected cases, including the real worker). A nested finally timestamps return/exception before outcome classification; query and overall intervals retain their prior meanings. Prior-evidence inventory distinguishes symlink text from file bytes; the initial setup attempt stopped before tests and is not credited as RED.
- 2026-11-15: The identical ten GREEN cases and all 424 full-suite cases pass; lint, formatting and strict pyright pass. Retained real receipts order query/receipt/probe and record helper, worker and independently waited fixture reap. The deterministic audit reconciles individual cases and unchanged before/after subjects, artifacts and all 75,628 prior entries. The report explains lifecycle-envelope timing, injected permission controls and the remaining aggregate obligations; no new architecture decision or ancestor closure follows from this evidence-only slice.
