# complete-worker-scheduler-k126

## Goal

Drive parent-side worker request waiting and failure cleanup through bounded
shared-driver turns. Preserve the original request deadline, channel caps,
late identity retention, unknown-creation accounting and owner-lifetime reap
and signal admission. The complete original charter and historical handoffs
are preserved verbatim in complete-worker-scheduler-k129.

## Done when

- Actual persistent and standalone request transports yield between bounded
  nonblocking channel turns and failure-retirement turns through `run_turns`.
- Request work is ordinary; failure retirement is safety work. Competing ready
  callbacks demonstrate both registrations, bounded reads, terminal behavior,
  and fixed cutoff admission. Real-process regression controls retain exact
  fixture ownership and audit results.
- Frozen RED/GREEN and mutation controls, diagnostic full suite/static checks,
  per-item comparisons and preservation hashes are retained in
  `experiments/exec-reliability/evidence/request-turns-k126/`.
- Cut an independent review-impl sibling for this load-bearing seam. k129
  retains the entire aggregate; k28/k17 remain open, guest acceptance 0/10.

## Decisions (running log)

- 2026-11-15: Flat sibling split because the full scheduler exceeds a focused
  session. k129 preserves the entire original charter/history; this leaf ships
  the independently verifiable request transport driver integration. No deeper
  node is needed because k28 already owns the joint runtime acceptance.
- 2026-11-15: Keep the existing owner guards and fixed query/shutdown reserve.
  Move waiting to bounded callbacks and central sleeping; no authority or
  retry policy change. Production-wide concurrency and teardown overlap remain
  k129. Ordinary verification stays here; independent challenge will be a
  separate review leaf, so no in-session adversarial reviewer is spent.

- 2026-11-15: Frozen RED: all four actual request paths bypass the driver.
  Corrected the persistent stalled fixture to install its creation fault before
  worker fork; successful controls now publish 128 KiB output to exercise
  multi-turn result reads. Original RED inputs/outcomes remain retained.
- 2026-11-15: Callback scope is version-invariant standard-library plumbing;
  no new library/API contract is introduced. Query readiness uses zero-timeout
  select and returns an immediate wake after ready bytes, otherwise <=5 ms.
  Retirement retains its fixed kill time, <=1 ms retry and lifetime guards;
  an already-terminal owner cannot force a stale kill wake into a busy loop.
  Repeated terminal callbacks perform no channel polling/reads. Existing
  setup/first-reap/late-publication controls remain part of verification.

- 2026-11-15: Requested-read accounting exposed observer subprocess I/O in the
  first strict control. Retain controls-ready and debug-read-budget failures.
  Exclude only the external identity observer from runtime accounting. Final
  controls-verified passes 90 cases. All five frozen mutants fail at their
  intended priority, zero-timeout, read-cap or terminal-poll assertion.
- 2026-11-15: First interleaved full pair preserves the same 879 case identities.
  Parent fails the four added transport controls. Candidate passes all 879.
  The second pair remains pending. Historical aggregate failures stay open.

- 2026-11-15: Second interleaved full pair also preserves all 879 identities:
  parent 875 passed / four added controls failed; candidate 879 passed. Ruff,
  format and strict Pyright passed in both pairs and the final frozen static
  run. Consolidated artifact/identity/per-item audit passed. All 206,230 old
  evidence paths are unchanged. Report retains failed controls and leaves
  historical aggregate failures, concurrency and overlap to k129.
- 2026-11-15: Independent review is placed immediately before k129 and owns
  challenge of the bounded request seam. No current GROVE_PROVIDER_EVIDENCE is
  present in this session; the review receives no Creator line, as retire.md
  requires. No ancestor closes because the review and full aggregate remain.
