# incremental-result-framing-k80

**Integrates:** complete-worker-scheduler-k79

## Goal

Triage the two findings of the k79 review of the k75 incremental result framing slice. Apply each one that is real. Keep k78's full charter unchanged.

## Context

Read the findings from the k79 commit, not from this body. Reviewed commit: f537c5f (complete-worker-scheduler-k75). Retained sources match the frozen snapshots byte for byte: `inputs-green-repaired` equals the f537c5f blobs, and `inputs-red` equals the f537c5f^ runtime. Source coordinates are in experiments/exec-reliability/worker_capture.py and test_worker_capture.py. Report: experiments/exec-reliability/evidence/result-framing-k75/report.md.

Findings, in order of consequence:

1. **The RED runs never showed the bounded-read and early-TERM properties failing, and one old assertion was weakened.** `test_result_framing_rejects_before_control_eof` (test_worker_capture.py:1735) asserts the error text first. That line is :1780, `"worker frame length" in receipt.error`. The original receiver fails that line first. The early-TERM timing (:1781), shutdown deadline (:1782), finish bound (:1783) and `os.read` byte counts never run. The retained RED JUnit files (result-framing-k75/red-01 and red-02) show it. Four modes fail with "worker query deadline exhausted". The overflow mode fails with "worker channel cap exceeded". So no run showed the read counts 4, 2, 5, 7 and 4+FRAME_CAP+1 able to fail. No run showed the TERM-before-0.45s bound able to fail either. audit.py:141-161 reconciles failing test names only, not failure reasons. Separately, `test_review_worker_rejects_hostile_channel` now exempts "oversized-frame" and "partial-frame" from the `receipt_monotonic` check. The exemption is an `or fault in {...}` clause at test_worker_capture.py:910-915, and it weakens the old assertion. The test does not assert that early rejection leaves `receipt_monotonic` as None. It also asserts no error text for those two faults, because `expected_errors` omits them.
2. **Zero and boundary declarations are untested, and a zero-length declaration is accepted at the header.** worker_capture.py:522-525 rejects only `result_size > FRAME_CAP`. A declaration of 0 gives `read_cap` 1 at :500. The parent then waits for both EOFs. It fails only in `_decode` on an empty body, or at the query deadline if the channels stay open. That is the wait k75 removes for the other malformed frames. No control declares zero. No control accepts a complete valid body at exactly FRAME_CAP, since the overflow mode uses FRAME_CAP only as a negative. The fragmentation control (test_worker_capture.py:1799) splits a valid frame as 1, 3, 13 and the rest. No control fragments an oversized header.

Verified without finding:

- Reads are bounded. The result read is at most four bytes until the header is complete. After that it is the declared remainder plus one detection byte, and at most 32 KiB per turn (worker_capture.py:494-500). The control read keeps `min(TURN_CAP // 2, 4097 - control_bytes)`. The post-turn cap check at :531-534 raises before that value can reach zero. The cleanup read at :620 is unchanged and reads the control channel only.
- Terminal success still needs both clean EOFs: `not streams` at :536. Query and shutdown deadlines, TERM and KILL ordering, and the cleanup loop are unchanged. No new wait, no renewed grace.
- The new guard at :637-640 closes the creation-accounting gap. Every discard path (the detection byte, control bytes past 4096, unread pipe bytes) leaves a channel out of `closed_channels`. `confirmed_failure` at :641-643 therefore stays false, and cleanup never adds to `closed_channels`. This invariant is sound but undocumented. Valid late identities stay cleanup-only. A timely fork-error line left unread by an early error is treated as late, which is conservative.
- The frozen mappings are coherent. The audit.py:15-21 table and the per-condition snapshot choice (:116-131) match the verdict files. The named failure sets (:141-161) match them too. Snapshot hashes match the git blobs. The inputs-red and inputs-red-final snapshots equal the parent runtime. The inputs-green and inputs-green-final snapshots equal the first change without the guard. The inputs-green-repaired snapshot equals f537c5f.
- The retained conditions match the report. Red-01 exits 1, green-controls-01 exits 0 and red-02 exits 1. Green-full-01 fails types and pytest. Green-full-02 passes everything in 44.15 seconds, within the 60-second budget.
- The audit counts match the files. They are 1,397 exact PIDs absent, 670 scratch directories and two plan-owned marker removals. The oracle controls give `[false, true, false, false]`. The audit excludes the simulated PIDs 123456789, 987654321 and 987654322. Later absence grants no historical reap authority.
- The exploratory red.xml is excluded from the five-condition census as stated. The docs entry and the k78 rename diff match the evidence, and k78's charter is preserved.

## Done when

- Classify each finding as a contract stated unclearly, a real issue, a visible trade-off or noise. Reproduce each one from source and a retained or new control before acting.
- Finding 1: if real, reorder or split the assertions. Each new property must fail alone against the original receiver. The properties are the read byte counts, the first TERM before the 0.45 s cutoff and the finish bound. Retain a new RED condition that shows each property failing for its own reason. Assert `receipt_monotonic is None` and the "worker frame length" text for the two hostile faults. Extend the auditor to compare failure reasons, not only test names.
- Finding 2: classify the zero-length declaration. If it is a real issue, reject sub-minimal declarations at the header without a new wait. Show a zero-length control failing before that change. Add a control that accepts a complete valid body at exactly FRAME_CAP, and one that fragments an oversized header. If it is a trade-off, document it in the report and cover it with a control that states the accepted behaviour.
- Preserve every earlier evidence byte for byte, including result-framing-k75. Freeze subjects and bounds for each new run. Keep the original query and shutdown deadlines, and add no wait. Do not widen k75 or k80. Keep k78 and every aggregate ancestor open. Ten-clone acceptance stays 0/10 unavailable. Acquire no privilege and touch no guest or installed application.

## Notes

No provider evidence was present in the review session. No Creator line was written.
The review ran by inspection only. It ran no test, build or lint command. Every
claim above comes from direct source reads, retained JUnit and verdict files, and
git blob hashes.
