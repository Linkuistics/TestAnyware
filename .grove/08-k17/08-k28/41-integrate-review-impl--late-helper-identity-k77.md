# late-helper-identity-k77

**Integrates:** complete-worker-scheduler-k76

## Goal

Triage the three findings of the k76 review of the k71 late helper identity slice. Apply each one that is real. Keep k75's full charter unchanged.

## Context

Read the findings from the k76 commit, not from this body. Reviewed commit: 122635c6d8b2 (complete-worker-scheduler-k71). Retained sources match k71-inputs-green-01 byte for byte. Source coordinates are in experiments/exec-reliability/worker_capture.py and test_worker_capture.py. Report: experiments/exec-reliability/evidence/late-helper-identity-k71/report.md.

Findings, in order of consequence:

1. **A valid identity that shares a read with overflow bytes is discarded.** The timely loop raises "worker channel cap exceeded" at worker_capture.py:503-506 before `consume_control(cleanup_only=False)` runs at :507. The `finally` path then calls `consume_control(cleanup_only=True)` at :577-578. That function returns at :401 once `control_bytes > 4096`. The cleanup read at :592-604 has the same order: it counts the bytes, then parses. A complete valid created line can arrive in the same chunk as the 4097th byte. It stays in `buffers[0]` and no code parses it. The worker then reports `unknown_helper_creation` true and no PID. The k71 slice says every distinct valid identity keeps its own obligation. This path loses a known one. The control `overflow` is at test_worker_capture.py:1575-1576. It puts the identity after the overflow line. No control puts a valid identity before it.
2. **Seven invalid-field controls and one scope assertion were never shown to fail.** `test_late_created_identity_rejects_invalid_fields` and the `signal_scope == "pid"` assertion at test_worker_capture.py:1646-1647 exist only in k71-inputs-green-01. RED (k71-red-01) drops every late line, so a rejected identity and a permissive parser both pass. The audit confirms the seven are new (audit.py:174) but not that they can fail. The `signal_scope` value in the select case comes from `owner.ready`, which the worker never sets late, and the worker is already reaped. The control therefore does not observe a PID-versus-group signal.
3. **The shutdown adapter does not match the production worker, and the report does not say so.** The control installs a SIGTERM handler at test_worker_capture.py:1562 and publishes after TERM. `reset_after_fork` restores SIG_DFL for every signal at inspection.py:458-459, so the real worker dies on TERM and publishes nothing after it. Real late bytes are only those already in the pipe. The report at lines 54-57 names the adapter but not the handler. It also counts the three shutdown cases as real late publication.

Verified without finding:

- `_created` and the control parser reject bool, wrong-token, wrong-parent, wrong-group, stale, future, deadline-mismatched and extra-field forms.
- Late readiness never sets `owner.ready`. Late fork-failure lines grant no no-child authority. The result channel is never decoded after failure.
- KILL and reap precede every cleanup read. The shutdown deadline is fixed. Each cleanup turn reads at most `min(TURN_CAP, 4097 - control_bytes)` bytes, without blocking.
- A helper obligation is removed only after a complete, reaped receipt (worker_capture.py:559-560). Later absence is not treated as reap.
- The audit counts match the files. Prior files: 71,616, unchanged. Subjects: 2,441 before and after. Late receipts: 19. Absent PIDs: 1,273. Scratch directories: 623. Two marker removals each followed an exact plan and byte match.
- RED has 335 cases with six failures, and GREEN has 342 with none. The six changed cases are the valid and distinct publications. The k71 RED worker_capture.py hash matches the pre-k71 runtime.

## Done when

- Classify each finding as a contract stated unclearly, a real issue, a visible trade-off or noise. Reproduce each one from source and a retained or new control before acting.
- Finding 1: if real, parse every complete buffered control line before honouring the cap. Keep the 4096-byte cumulative cap, the overflow-detection byte and the fixed deadline. Add controls for a valid and a distinct identity that arrive before the overflow bytes in one read. Run them over the read, select and shutdown delays. Show each control failing before the fix.
- Finding 2: run each of the seven invalid-field controls against a deliberately permissive parser, and show each one failing. Run the late-readiness scope assertion against an owner that widens scope. Retain that mutant condition. Make the scope control observe the signal the worker sends, or label it a flag test.
- Finding 3: label the shutdown cases as an adapter with a TERM handler. If the harness allows it, add a control that leaves the identity in the pipe before TERM. Run it with the production signal disposition. State in the report which late bytes the production worker can produce.
- Preserve every earlier evidence byte for byte. Freeze subjects and bounds for each new run. Do not widen k71. Keep k75 and every aggregate ancestor open. Ten-clone acceptance stays 0/10 unavailable. Acquire no privilege and touch no guest or installed application.

## Notes

No provider evidence was present in the review session. No Creator line was written.
