# helper-fork-error-k74

**Integrates:** helper-fork-error-k72

## Goal

Triage the four findings of the k72 review of the k69 helper fork-error slice. Apply each one that is real. Keep k71's full original charter unchanged.

## Context

Read the findings from the k72 commit, not from this body. Reviewed commit: adf436d473df (complete-worker-scheduler-k69). Retained sources match k69-inputs-02 byte for byte. Source coordinates are in experiments/exec-reliability/worker_capture.py and inspection.py. Report: experiments/exec-reliability/evidence/helper-fork-error-k69/report.md.

Findings, in order of consequence:

1. **Partial evidence after a valid fork failure does not revoke the no-child classification.** At worker_capture.py:476-480 the guards "incomplete worker publication" and "invalid worker frame length" raise before `creation_conflict = fork_failure is not None` runs at :483. Partial trailing control bytes (`buffers[0]` non-empty) or a truncated or oversized terminal frame therefore leave `creation_conflict` false. Then `confirmed_failure` (:546) stays true and `unknown_helper_creation` is false. A truncated created-identity line after a failure line is contradictory. It carries a PID that the parent never decodes. The code revokes for a complete contradictory line (:460) and a complete invalid frame (:483), but not for the partial forms. The same gap applies when the loop stops on the deadline with unread control bytes in the pipe. `confirmed_failure` does not require control EOF. No control covers partial trailing bytes, a truncated terminal or an unread tail.
2. **A failing callback replaces the fork errno.** `publish_fork_error` runs inside the `except OSError` handler (inspection.py:263-266). If the write raises, the new exception replaces the fork error. `TimeoutError` has errno `None`. `BrokenPipeError` has EPIPE, and `capture_helper` stores that as `error_errno` (:404-405). The terminal then reports no PID and a wrong or missing errno, with no failure line. The parent counts it as a complete no-child result without the original errno. k71 requires the fork errno to survive. No control makes the callback fail.
3. **Only the first valid created identity becomes an obligation.** At worker_capture.py:469-471 a second valid created line with a different PID raises a conflict but never reaches `_helper_obligations` or `worker["helper_identity"]`. The `duplicate` control sends one identical line twice, so it cannot show this. An honest worker never sends two identities. The contract still says a known helper stays tracked until its own reap acknowledgement.
4. **Two controls are weaker than their names claim.** In `test_review_worker_rejects_invalid_fork_failure` (test_worker_capture.py:1255-1257) mode `late` sleeps past the deadline and the worker's own `_write` then times out. No failure line is published. The parent-side bound `stamp <= now < query_deadline` (:234) and the late-receipt guard (:423) are therefore untested for a failure line. No control pairs a confirmed failure with an unclean worker exit code. No control pairs one with an unreaped worker.

Verified without finding:

- Only the `os.fork` OSError reaches the callback. Pipe, setup, admission and exec failures cannot publish a failure line.
- Schema and type validation reject bool, zero, negative, string, missing, extra, NaN, wrong-token and wrong-deadline forms.
- Created-then-failure and failure-then-created both retain the known identity.
- Injected EAGAIN and EMFILE are described as boundary injection. Synthetic PID 987654321 is never signalled.
- The audit does not treat absence as a reap acknowledgement.
- The count of 23 controls (4 + 19) and the case counts 311, 288 + 4 and 309 + 2 match k69-audit-01/result.json. Final sources match k69-inputs-02.

## Done when

- Classify each finding as a contract stated unclearly, a real issue, a visible trade-off or noise. Reproduce each one from source and a retained or new control before acting.
- Finding 1: if real, revoke the no-child classification after a failure line. Trigger it on leftover control bytes, a partial frame or an unread pipe tail. Add controls for partial trailing control bytes, a truncated terminal and an oversized frame length. Label which parts are real and which are adapters.
- Finding 2: if real, keep the original fork errno when the callback fails. Leave the clean-path message unchanged. Add a control where the callback raises `TimeoutError` and one where it raises `BrokenPipeError`.
- Finding 3: if real, retain every distinct valid created identity as its own obligation. Otherwise record the dropped one in the worker evidence. Add a control with two distinct synthetic PIDs and never signal either.
- Finding 4: rename or repair the `late` control so it tests what it says. Add the missing unclean-exit and unreaped-worker controls.
- Preserve every earlier evidence byte for byte. Freeze subjects and bounds for each new run. Do not widen k71. Keep every aggregate ancestor open. Ten-clone acceptance stays 0/10 unavailable. Acquire no privilege and touch no guest or installed application.

## Notes

No provider evidence was present in the review session. No Creator line was written.
