# linux-disconnect-availability-k208

**Integrates:** linux-disconnect-availability-k207

## Goal

Triage the committed findings of `linux-disconnect-availability-k207` against the producer's contract and evidence. Settle the accepted scope before later leaves consume the Linux disconnect report.

## Context

- The review handle is `linux-disconnect-availability-k207`; read its findings from that review's commit. Its durable artifact is `docs/experiments/exec-reliability-linux-disconnect-availability-review-k207.md`.
- The producer is `linux-disconnect-availability-k6`, with report `docs/experiments/exec-reliability-linux-disconnect-availability.md` and tooling under `experiments/exec-reliability/`.
- The brief chain supplies the measurement, ownership and evidence acceptance contract. Retained evidence is under `experiments/exec-reliability/evidence/linux-disconnect-availability-k6/`.

## Done when

- Each review finding is independently checked and classified against agreed acceptance and material risk. Rejected findings and accepted trade-offs have an explicit rationale.
- Valid findings have a coherent disposition in the tooling, report and evidence limits, with verification appropriate to the accepted changes. Preserve retained failed samples and distinguish any newly authorized measurement from earlier attempts.
- The durable report states what later evidence consumers may rely on and what remains unproved. Any required work beyond this bounded integration is named precisely and sequenced before its consumer.

## Notes

## Decisions (running log)

- Finding 1 (handler lower bound) is a real issue. In `live_handler`, a missing alive sighting passes the upper bound alone to `handler_class`. Then any end near 20 s becomes `deadline`. The checker now keeps the term end as an upper bound with class `gone-by`. The deadline reply supports `deadline` in `control` and `concurrent`, with basis `deadline-reply`. The unused constant `QUEUE_RELEASE` and its claim go. The 18.067 s inspection latency still supports `blocked` by condition 3.
- Finding 2 (live numeric-PID cleanup) is a real issue. The k204 shell cleanup checks `sleep 120` once and then signals the numeric PID twice. The k6 live cleanup becomes a guest Python cleanup. It opens a pidfd first, and it binds the PID to the start time of the child by the mtime of `child.pid`. Then it sends TERM and KILL through that pidfd. The retained cleanups stay recorded as successful TERM cleanups, with no wrong-process kill observed. `linux_k204.owned_cleanup` keeps the same limit for k204, a finished leaf. The report names that limit for any later reuse.
- Finding 3 (live provenance) is a real issue for later runs and an accepted limit of the retained runs. The live plan now hashes the app executable, its `Info.plist` and every installed Linux agent module before and after. The runner records the app signature and paired installed hashes. The retained runs keep their gap, and nobody substitutes today's hashes for their after-state.
- Finding 4 (replay path) is a real issue. Both checker entry points now resolve the run directory inside the selected OUT by the base name of the recorded path, as `linux_k5_check.run_dir` does. The recorded absolute path stays as provenance. The checker takes an optional result name, so a re-judgement of a retained OUT does not overwrite the retained `check.json`.
- No new measurement is authorised. The corrections change inference, cleanup and provenance, not the measured subject. The retained outputs stay, and the re-judgement by the corrected checker has the distinct name `check-k208`.
- Verification: `diagnostic:linux-disconnect-tools` passes, with 30 tests. A container control in the pinned k5 image exercised the pidfd cleanup outside any measured run. The re-judgement in `rejudge-k208/` exited 0 for all three OUT directories, and the digests of the 114 retained files were equal before and after.
- No further leaf is needed in this grove. The report names three unproved items for `lifecycle-evidence-k8`: the `term` handler end on the clone, the retained live cleanup targets, and the retained live app and after-state identity. A later live run must use the corrected tooling under a new identity.
