# complete-worker-scheduler-k60

## Goal

Reject contradictory admission, error/errno and ownership states in the interim worker receipt transport, preserving genuine success and failure receipts.

## Context

Read k48 and the process-inspection spec. k61 reviewed k58 in commit 88744097 and found that its unreaped-error control reports ECHILD while retaining ownership; the genuine producer sets ownership_lost for ECHILD. This increment closes that consistency gap and validates the complete admission record against inspection.fork_admission, without claiming independent identity authentication.

## Done when

- Validate the complete admission schema, strict booleans/positive counts/errno types, signal states, reason priority and observation-error consistency. Preserve coherent admission refusals as failed no-helper receipts; reject fabricated ready records.
- Validate positive errnos, meaningful error/remedy/close-error strings, error-bearing failure states, helper-creation coupling, ECHILD/ownership equivalence and residual ownership consistency against the genuine producer. Rejected evidence discards output and conservatively retains unknown helper creation.
- Exercise malformed and coherent synthetic helper receipts through real worker fork/reset/framing/channels/decoding/shutdown. Preserve genuine success, nonzero, missing executable and killed-helper controls; add a genuine helper admission refusal control. Label synthetic evidence and independently audit recorded process selectors outside the worker.
- Freeze subjects before each run, retain exact RED/GREEN case inventories, lint/format/strict types and hashes, preserve prior evidence byte-for-byte, extend the report and retain all failed attempts.
- Preserve the entire original runtime/control/final-review charter in flat sibling complete-worker-scheduler-k62. Insert this increment's independent review before k62; no empty integration. Keep every aggregate ancestor live.

## Notes

Persistent preacquisition bootstrap, gated publication, independent helper ownership, shared scheduler and every original real control remain k62; ten-clone acceptance remains 0/10 unavailable. No guests, installed app changes, privilege acquisition, unrelated process mutation or storage-stall claim.

## Decisions (running log)

- 2026-11-14: Flat sibling split follows Grove's session-sized execution rule. Admission and failure-state validation is independently useful and verifiable in the existing transport; it does not discharge the persistent-worker/scheduler contract. The original k60 charter and history were copied intact to k62 before narrowing this leaf.
- 2026-11-14: Re-derived k61's low finding from genuine _reap_helper: ECHILD always closes ownership. Replace the ECHILD/retained-owner positive control with EIO plus residual remedy, retain a coherent ECHILD/lost-owner control, and add explicit mismatches as negative controls.
- 2026-11-14: Implemented complete admission and failure-state validation. Fifty synthetic helper-publication modes exercise the real worker transport; a genuine two-thread helper-admission refusal now remains a known no-helper outcome. Worker-reported identity/reap is still unauthenticated and belongs to k62.
- 2026-11-14: Frozen RED has exactly 45 failures and 206 passes; first fixed run has all 251 tests passing but four strict-type narrowing errors. Final k60-fixed-02 has all 251 tests, lint, format and strict types passing. Every attempt is retained, with identical case sets and before-run source copies.
- 2026-11-14: Audit01 passed lint/format then failed before reconciliation because a witness glob also matched longer mode names. Audit02 uses exact names and passes: 348 worker controls across three conditions, 1,644 positive process selectors absent, 672 fixture directories reconciled, and all 50,043 input hashes unchanged, including 46,714 earlier evidence files. No selector was signalled; later absence does not repair unknown creation. See docs/experiments/exec-reliability-diagnostic-foundation.md and evidence/k60-audit-02/result.json.
- 2026-11-14: This narrowed leaf meets its Done when; the full persistent-worker/scheduler remainder and final aggregate review remain live in k62. Its independent admission/failure-receipt review must run first. No aggregate ancestor closes and acceptance stays 0/10 unavailable.
