# macos-completion-review-k184

**Reviews:** macos-completion-k3

<!-- tc: off because the creating session wrote this charter and a review does not rewrite it -->

## Goal
Adversarially review the first k3 slice: the retained-evidence manifest, the
old-path GCD-starvation detector and its detector control, before later
leaves build on them.

## Context
- Read the producer commit for `macos-completion-k3`, this grove's root brief
  and `docs/experiments/exec-reliability-macos-completion.md`.
- Tooling is under `experiments/exec-reliability/evidence/macos-completion-k3/`:
  `retained.py`, `old_path.py` and `probe/main.swift`. The Taskfile tasks are
  `diagnostic:macos-completion-retained`, `diagnostic:macos-completion-old-path`
  and `diagnostic:macos-completion-tools`.
- Raw runs are `old-path-01` and `single-hop-01` in that directory. The
  retained originals are in the Ravel-Strategies checkout named by
  `retained-manifest.json`.

## Done when
- Check whether the probe could report `completed-then-timed-out` for a
  reason other than the nested-dispatch ordering, such as calibration that
  does not saturate, blockers that leak into the run, or a 3 s timeout that
  the host misses.
- Check whether the single-hop control truly removes only the second block,
  and whether its `process.isRunning` polling depends on the global queue.
- Check the classification and verdict rules against every raw receipt, and
  check that `EXPECT` semantics keep the known-defect probe outside a passing
  gate.
- Check every identity, timing and provenance claim in the report against
  the retained receipts and the manifest. Decide whether the report keeps the
  cause unestablished wherever the evidence is not causal.
- Check the owned-resource audit, and check that the probe always releases
  its blockers, also on calibration failure.
- Report each finding with its evidence. Do not edit the producer's files.

## Notes

## Review findings

Reviewed producer commit `2328c738e9568a960551f7373e9f7d73215a186a`
(`macos-completion-k3`). This was an inspection-only review: no producer
files changed and no tests, builds, diagnostics, lint or formatting ran.

### F1 — P2: keep completion and dispatch-phase claims within the observations

At `experiments/exec-reliability/evidence/macos-completion-k3/old_path.py:148`,
`completed-then-timed-out` requires output, a timeout flag and exit -1, but
no observation of when the child exited. The verdict at
`experiments/exec-reliability/evidence/macos-completion-k3/old_path.py:173`
therefore cannot distinguish a missed completion from a child or host that
actually misses the 3 s deadline after printing. The probe at
`experiments/exec-reliability/evidence/macos-completion-k3/probe/main.swift:134`
records only the returned result and total run duration, without observing
the child exit or either dispatch block.

The controls provide useful contrary evidence: all five old free-1 trials
took 6.0298–6.0755 s inside the probe, while the other old conditions were
prompt; every single-hop trial was prompt. However, the replacement at
`experiments/exec-reliability/evidence/macos-completion-k3/old_path.py:49`
also replaces `waitUntilExit()` with `isRunning` polling. It changes both
the extra dispatch and the exit-observation primitive. The five successful
single-hop free-1 samples establish that this polling control can finish
under the recorded condition; they do not independently locate the old
wait's stalled phase.

Consequently, the assertion that the local recovery wait "also starved" at
`docs/experiments/exec-reliability-macos-completion.md:130` exceeds the
recorded phase evidence. The historical assertion that the second wait
"succeeded promptly" at
`docs/experiments/exec-reliability-macos-completion.md:38` likewise treats
the present source's recovery path as observed, although the historical
binary's source remains unproved. The report correctly leaves the
historical cause open elsewhere, but these assertions bypass that limit.
This matters to the agreed causal inquiry and to interpreting a future
`EXPECT=clean` result. Integration should keep receipt-shape evidence and
mechanism inference explicit, and either obtain the missing phase/control
evidence within the existing bounded inquiry or narrow the causal claims.
The retained data strongly supports the starvation hypothesis; this finding
does not assert a different cause.

### F2 — P2: the calibration-cap failure releases zero held blockers

The loop at
`experiments/exec-reliability/evidence/macos-completion-k3/probe/main.swift:88`
can reach `maxBlockers == 512` with every blocker started and holding the
gate. In that path `saturatedAt` remains nil, and calibration correctly
fails at
`experiments/exec-reliability/evidence/macos-completion-k3/probe/main.swift:129`.
Cleanup at
`experiments/exec-reliability/evidence/macos-completion-k3/probe/main.swift:149`
then computes `(saturatedAt ?? 0) - min(freed, saturatedAt ?? 0)`, signals
zero times and records `released: 0`, despite the 512 held blockers.
This directly misses this review's explicit requirement to check release
on calibration failure and contradicts the unconditional release claim at
`docs/experiments/exec-reliability-macos-completion.md:100`.

Cleanup needs to account for blockers actually admitted even when saturation
is not established. All 20 retained saturated trials reached 80 and record
the expected 79 or 78 releases; this failure was not observed in those runs.
Process exit reclaims the probe's threads, so this is not a claim that those
threads survive process exit or leaked into the recorded trials.

### F3 — P2: the outer timeout lacks ownership and cleanup of probe resources

At `experiments/exec-reliability/evidence/macos-completion-k3/old_path.py:117`,
`subprocess.run(..., timeout=20)` bounds the direct probe. On timeout the
handler at
`experiments/exec-reliability/evidence/macos-completion-k3/old_path.py:120`
only retains partial output and marks a harness failure. It has no recorded
identity or cleanup for the probe's bash child, and no probe-owned temporary
directory to recover the capture files. The runner creates those files at
`agents/macos/Sources/TestAnywareAgent/ProcessRunner.swift:43` and removes
them in a defer; killing the probe prevents that defer from running. A child
still alive at the bound is not covered by killing only the direct probe.

The root contract at `.grove/_BRIEF.md:295` requires positively identified
processes, unique temporary directories and an independent bound including
failed wait/inspection cleanup. A stalled probe is precisely the failure
this fixture must contain, so ownership and bounded cleanup/audit must also
cover that path. This finding is about the reusable harness contract, not a
request to alter production wait semantics. No retained trial reached the
20 s bound, and no surviving child or capture-file leak was observed in this
review. The successful-run audit at
`docs/experiments/exec-reliability-macos-completion.md:169` does not establish
the unexercised timeout cleanup behavior.

## Evidence checked and limits

- All 319 originals named by `retained-manifest.json` and all 32 copied
  receipts match their recorded hashes. All ten historical ordinary
  operations use the recorded command and 30 s timeout; only operation 08
  has `timed_out: true`, and the report's rounded durations match.
- All 30 trial JSON receipts match their raw stdout; their stderr files are
  empty. Both summaries agree with all per-condition classifications.
  All successful saturated calibrations record 80, a surplus arrival,
  and 79/78 releases; the unsaturated cases dispatch no blockers.
- Current runner/probe/script hashes match both plans and summaries. Both
  local probe binaries match their summary hashes. The generated single-hop
  source differs from the runner by exactly the declared replacement.
- Source history supports the report's unchanged `ProcessRunner.swift` and
  `handleExec` claims since April 2026. That does not establish the retained
  guest binary's source. The attempted packaged-agent lookup used an absent
  local path and contributes no new identity evidence; the report's broad
  assertion that no local build matches was not freshly established here.
- `Taskfile.yml` keeps these explicit diagnostics separate from the normal
  passing gate, and documents that default `EXPECT=defect` accepts the known
  defect. The single-hop variant remains a detector control.
- Tier 2 graph verification used project
  `Users-antony-Development-AbstractingComputers-TestAnyware.gh-issue-14`,
  coverage generation `2026-11-15T02:19:52Z`. Production symbols/snippets and
  traces were checked. Coverage was checked for all 144 evidence paths,
  including the binaries; the new evidence was untracked by that generation
  and two metadata checks were changed, so direct source/raw-file reads and
  hashes supplied the evidence. No graph completeness claim is made.

## Decisions (running log)

The three findings meet the acceptance/material-risk bar: they affect the
causal conclusion or explicit calibration/owned-fixture acceptance. Record
them for independent triage rather than fixing producer files in a review.
The finite handoff is one sibling integration before the later live macOS
work, using the producer's bare stem `macos-completion`. It consumes this
review's commit, assesses each finding and owns any repair and verification.
The remaining live siblings keep the root open; no parent close is due.
