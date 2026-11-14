# complete-worker-scheduler-k52

## Goal

Reject impossible or out-of-window worker query times at the real controller transport boundary, retaining failed protocol outcomes and independently checkable worker cleanup evidence.

## Context

The unchanged worker-scheduler-k48 charter and spec remain aggregate requirements. The full original k52 contract is carried verbatim in sibling complete-worker-scheduler-k55. k51/k54 provide only the interim per-query worker and its reviewed transport repairs.

## Done when

- Validate required query timestamps as finite numeric values, with controller dispatch <= worker start <= worker finish <= controller receipt <= the immutable query deadline. Reject missing required times, reversed or predispatch intervals and future finishes before accepting helper output. Preserve optional create/kill/reap times only when coherent within the query interval and validate the capture cutoff within the query allowance.
- Demonstrate the actual nonblocking fork/pipe/frame/decode path rejects crafted impossible receipts and still accepts ordinary real-helper observations. Keep rejected receipts failed with unknown helper creation; worker reap never proves helper absence. Retain raw payloads and parent timing witnesses for these controls.
- Freeze new failing/passing conditions through the reusable runner, preserve every earlier evidence byte, independently audit exact known worker/helper/fixture identities, and update the durable report with precise limits.
- Cut a specific review-impl complete-worker-scheduler before k55, Reviews: complete-worker-scheduler-k52, for independent protocol/timing/ownership review. No empty integration leaf.

## Decisions (running log)

- 2026-11-14: Use flat sibling decomposition under k48 because receipt validation is independently useful in the current runtime and shares the parent's scope; no new context boundary is needed. k55 owns the full original contract, including persistent worker, gated helper publication, all shared scheduling and shutdown, outstanding controls and final bounded-inspection review before k21. All ancestors remain open.
- 2026-11-14: Graph Tier 2 generation 2026-11-14T10:16:48Z: _decode's sole direct caller is worker_capture.capture_helper. Relevant runtime/test/config paths have metadata_match/no_recorded_issue coverage (best effort only); source confirms parent overwrites worker finish after decode, so validation must occur before that overwrite and carry an explicit parent receipt witness.
- 2026-11-14: New timing controls use synthetic worker publications through real fork/reset/nonblocking pipes/frame/decode/shutdown; no helper is created by these adapters. Preserve payload/frame bytes as hex alongside verdict receipts. Parent dispatch and complete-publication receipt witnesses will be retained separately from the worker-reported interval before final parent finish overwrites it. Capture cutoff may follow early query finish, but must stay within [worker start, immutable query deadline]. Optional create/kill/reap times must lie within [worker start, worker finish] in causal order; none grants helper identity authority.
- 2026-11-14: k52-red-01 reproduces 12 accepted impossible timing receipts (166 pass). k52-fixed-01 rejects those receipts; two malformed-frame controls fail their new parent-receipt-witness assertion (177 pass). Record complete-channel receipt before frame-length validation so rejected complete frames retain the same observation witness. Preserve both failed conditions unchanged and verify as k52-fixed-02.
- 2026-11-14: k52-fixed-02 passes Ruff lint/format, strict Pyright and all 179 tests. k52-audit-01 validates all retained condition outcomes, raw timing publications, source-copy/runner digests and earlier evidence hashes; its external oracle observes 1,451 exact retained identities absent and reconciles 455 allowlisted fixture directories. Later absence does not repair original unknown helper creation. See docs/experiments/exec-reliability-diagnostic-foundation.md, k52 section, and evidence/k52-{red-01,fixed-01,fixed-02,inputs-01,audit-01} under experiments/exec-reliability/. No guest/app/privilege or acceptance claim; 0/10 unavailable.
- 2026-11-14: Independent adversarial protocol/timing/ownership review is required before k55 because accepted child times influence process-inspection evidence. Cut review-impl complete-worker-scheduler as the last work act, with this leaf as producer; do not substitute the routine resource audit for that review. k55 and all ancestor completion criteria remain live.
