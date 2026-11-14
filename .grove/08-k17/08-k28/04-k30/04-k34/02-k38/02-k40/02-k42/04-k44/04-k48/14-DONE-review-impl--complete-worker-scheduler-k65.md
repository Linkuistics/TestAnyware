# complete-worker-scheduler-k65

**Reviews:** complete-worker-scheduler-k62

## Goal

Independently challenge the raw helper fork/reset/exec and local exact-child ownership shipped by k62. Re-derive the boundary from the committed producer artifact and current source before k64 relies on it.

## Context

Read k62, k48 and the aggregate spec, then experiments/exec-reliability/evidence/raw-helper-fork-k62/report.md. The final run is green03; red01, green01, green02 and stdio-red01 are intentionally retained. Their source snapshots match their subject hashes. The change replaces inspection._HelperProcess's Popen subclass with a plain owner and explicit _spawn_helper/_helper_child; transport test seams changed, worker_capture runtime did not. This is a bounded runtime increment, not completion of k64 or any ancestor.

## Done when

- Trace actual admission, fork, signal mask, child reset/exec/exit, descriptor creation and closure, owner registration/recovery, exact-child reap/KILL authority and residual lifetime across success and all material failures. Inspect waits, destructors and concurrency boundaries in inspection.py, worker_capture.py and controller.py; distinguish production behavior from fixture reconciliation. Challenge ordering and closed-stdio behavior, including actual stdin inheritance through exec and parent failure after a known fork.
- Inspect actual child observations and receipts for inherited handlers/exit hooks, wakeup and evidence FDs, pending real signals, reset/exec failure, normal/slow/overflow capture, and terminal transport receipts. Fork errno, parent mask restoration and simulated wait non-observation are labeled adapters: do not promote them to real resource-exhaustion or unreapability proof.
- Verify frozen run subjects, source snapshots, checker budgets/results and evidence integrity. Reconcile the 50,053-file prior disk inventory with tracked and ignored evidence as applicable; no commit-only claim of ignored-file preservation. Retain newly produced verification evidence without altering earlier runs.
- Check status-70 ambiguity and unchanged limits are explicit: no child-error channel, persistent worker, gated publication, authenticated helper identity, controller-owned helper reap acknowledgement or shared scheduler. Confirm k64 preserves the entire original contract and all aggregate ancestors stay open; ten-clone acceptance remains 0/10 unavailable and privileged controls unavailable without authorized prerequisites.
- Report actionable findings with exact paths/lines and evidence in this review leaf. Cut integrate-review-impl before k64 only if findings earn it; no empty integration. Do not implement the full scheduler here or acquire privileges/change guests, applications or unrelated processes.

## Notes

Provider evidence was absent in the finishing producer's environment. No Creator is fabricated; caller review policy may require the owner's declaration separately.

## Decisions (running log)

- This review is inspection only. No test, lint, format or type command ran. No production or test file changed. The commit read is k62, 25b74d9ad97d. The only commands run were read-only: `cmp`, hash comparison against the retained inventories, and file reads.
- No `GROVE_PROVIDER_EVIDENCE` was present in this session's environment. No Creator line is written on any review.
- Fork and child path: `_spawn_helper` (experiments/exec-reliability/inspection.py:192-234) blocks all signals before `os.fork`. The parent registers the owner and `receipt.pid` at lines 215-218 before the `finally` restores the mask at line 220. `_helper_child` (lines 174-189) calls `os.dup2` onto 1 and 2, then `reset_after_fork({1, 2})`, then `os.set_inheritable(0, True)`, then `os.execvp`. The `finally` calls `os._exit(70)` on every failure path. `_capture_pipe` (lines 157-171) keeps both endpoints at descriptor 3 or above, so the child's dup2 targets cannot collide with a source. `dup2` clears close-on-exec on the targets, and `set_inheritable(0, True)` covers the stdin that reset reopens. The closed-stdio test (test_inspection.py:833-862) has the helper read EOF from stdin after exec, which confirms the inheritance fix.
- Parent failure after a known fork: `capture_helper` recovers the owner by `receipt.pid` (inspection.py:322-324). The retained mask-restore adapter shows the child killed and reaped. The only unguarded window is the single dict assignment at line 216. A KeyboardInterrupt exactly there would leave an unregistered child. That is the same class of window as any fork, and it is accepted.
- Reap and KILL authority: `_reap_helper` (lines 237-255) uses only `waitpid(pid, WNOHANG)`. `capture_helper` sends `os.kill(process.pid, SIGKILL)` only while the child is unreaped and ownership is not lost (lines 328-334). No Popen, destructor, context-manager wait or `communicate` remains in inspection.py. `worker_capture.py` has no change in the commit.
- Real versus adapter: the pre-exec observation (green03 run receipt `raw-helper-reset-real-preexec-observation`) is a real child state. It shows default USR1/INT/PIPE handlers, an empty mask, wakeup -1, GC disabled, an empty owner table, closed evidence and wakeup descriptors, and inheritable stdio. Fork EAGAIN, mask-restore EIO and the waitpid non-observation control are adapters, and the report labels them so. None of them is promoted to proof of resource exhaustion or unreapability.
- Evidence integrity: all five runs have equal before and after subject hashes. Every run's `sources/` snapshot matches 12 of its 14 subjects byte for byte. The two subjects without a snapshot are the interpreter binary (`.venv/bin/python`) and `/bin/ps`. Their hashes are recorded but cannot be re-read from a snapshot. The green03 snapshots of inspection.py, worker_capture.py, controller.py and test_inspection.py equal the tree. Pytest XML counts agree with the report table for all five runs (255, 258, 258, 258, 258 tests; 4, 1, 0, 2, 0 failures).
- Inventory reconciliation: `prior-evidence-before.json` and `prior-evidence-after.json` are identical and hold 50,053 entries. Re-hashing every entry against the disk found 0 mismatches. `artifact-inventory.json` holds 5,549 entries and re-hashing found 0 mismatches. In the parent commit 37,730 of the 50,053 entries are tracked. The other 12,323 are ignored or untracked. The preservation claim therefore holds for the disk only, as the report says, and the commit alone does not carry it.
- Limits: the report and the k62 log state that status 70 is ambiguous, that no child-error channel exists, and that no persistent worker, gated publication, authenticated helper identity, controller-owned reap acknowledgement or shared scheduler exists. k64 carries the whole original contract in its Done when. k48 and every aggregate ancestor stay open. Ten-clone acceptance stays 0/10 unavailable. Privileged ps-KILL and UID-EPERM controls stay unavailable.

## Findings

One actionable finding. integrate-review-impl complete-worker-scheduler-k66 was cut before k64.

### Finding 1 (medium): the helper now starts its own session, and no document, control or report records that this leaves the worker's group

- **Source.** `_helper_child` calls `reset_after_fork` (inspection.py:181), and `reset_after_fork` ends with `os.setsid()` (inspection.py:387). The pre-k62 Popen helper never called `setsid`. In 3aed82eb the only `setsid` is the one in `reset_after_fork`, which only the worker child used. The helper is now a session and group leader: pid, pgid and sid are equal.
- **Contract conflict.** docs/specs/diagnostic-process-inspection.md:31-32 says the helper runs "in the worker's group". Lines 39-43 let the controller signal the worker group while the worker leader is unreaped. The k62 charter says to reset "session before exec", so the producer followed the charter. But the charter and the spec disagree, and the k62 log does not record the conflict or choose a side.
- **Consequence.** A controller group KILL of the worker no longer reaches a live helper. A hung helper outlives the worker, as its own session with a dead parent, until its own exact-PID KILL, which only the dead worker could send. Before k62, that group KILL did reach it. The existing text, "worker death never establishes helper death" (spec line 129), stays true. But the containment that the old behaviour gave for free is gone, and no control shows the new behaviour.
- **Evidence.** test_inspection.py:700 asserts `state["pid"] == state["pgid"] == state["sid"] == capture.pid`. The retained green03 observation shows pid 9602, pgid 9602, sid 9602, ppid 7885. The assertion encodes the change as intended. No test signals the worker group while a real helper lives. `test_review_worker_retains_timing_for_real_killed_helper` (test_worker_capture.py:916) kills a separate Popen session, not the raw helper. The k62 report lists the recorded SID but does not say the helper left the group.
- **What the integrate session decides.** Either amend the spec so the helper is a detached session whose only containment is exact-PID ownership, or stop the helper calling `setsid` and keep it in the worker's group. Whichever it chooses, record it, and add a retained control that sends a real group KILL to a worker with a live helper and labels what happens to the helper. k64 already owns "track each known helper until reap acknowledgement independently of worker death/group KILL", so the choice must not widen k64's contract.

### Non-actionable observations, for k64 to remember

1. Two of 14 frozen subjects (the interpreter and `/bin/ps`) have a recorded hash but no snapshot. The report sentence "the `sources/` snapshot in each run matches its recorded source hashes" is true for sources and does not cover those two binaries.
2. The report says the 50,053 files include ignored ones. It does not give the tracked and ignored split (37,730 and 12,323 at the parent commit). A reader must not take the commit as the preserved set.
3. The stdin and exec ordering is correct today because `os.execvp` is called after `set_inheritable(0, True)`. The pre-exec test works by patching `os.execv`, and `os.execvp` resolves `execv` at call time, so that seam depends on CPython's `_execvpe` internals.
4. If `pthread_sigmask` restore fails in the parent after fork, the parent mask stays fully blocked. The report labels this an adapter. Real kernel failure of that call is not expected.
