# macos-completion-k187

**Integrates:** macos-completion-review-k184


## Goal
Triage the review of the first `macos-completion-k3` slice and resolve the
findings that meet agreed acceptance or identify a material risk, leaving a
sound boundary for the remaining macOS work.



## Context
- Read the findings from the commit for `macos-completion-review-k184` and
  recheck them against the producer artifact and root contract.
- The artifact is `docs/experiments/exec-reliability-macos-completion.md`
  and its tooling/evidence under
  `experiments/exec-reliability/evidence/macos-completion-k3/`.
- The later live siblings `macos-process-fixtures-k185` and
  `macos-completion-live-k186` retain their existing acceptance obligations.

## Done when
- Classify every review finding against the source as an unclear contract,
  actionable issue, accepted trade-off or noise, recording the evidence and
  acceptance/material-risk reasoning.
- Repair accepted issues within this integration's scope and perform the
  required bounded verification through the project Taskfile. Preserve
  previous evidence and distinguish any new conditions and measurements.
- Keep the report consistent with the established evidence and leave the
  remaining live acceptance work explicitly owned by its existing leaves.

## Notes
This integration is the next sibling because it consumes findings anchored
to the review's commit. Its charter is triage, not acceptance of the
reviewer's conclusions. Substantial redesign follows the Grove procedure.

## Triage

Findings come from review commit `ebe904ccd4` (`macos-completion-review-k184`).
Each finding was rechecked against producer commit `2328c738e9`.

- **F1, completion and phase claims: actionable issue, accepted.** The probe
  records only the returned result and the total duration. It does not observe
  the child exit or either dispatch block. The report stated two phases as
  observed: the retained recovery wait "succeeded promptly", and the local
  recovery wait "also starved". The single-hop control also replaces
  `waitUntilExit` with `isRunning` polling. This affects the causal
  conclusion, which the root contract keeps separate from repair. Repair: the
  report now states only the observed durations. The probe duration of about
  6 s is consistent with two 3 s timeouts, and the report names that limit.
  Guest phase observation stays with `macos-completion-live-k186`.
- **F2, release after a failed calibration: actionable issue, accepted.**
  `probe/main.swift` computed the release count from `saturatedAt ?? 0`. At
  the 512 cap with every blocker held, it released 0. The review charter and
  the report both require release on failure. Repair: the probe counts
  admitted blockers, releases every admitted blocker that it did not free and
  records `admitted`. An optional `MAX_BLOCKERS` argument lets the harness
  exercise that path. Every run now starts with a cap check at 8 blockers.
- **F3, ownership after the trial bound: actionable issue, accepted with one
  visible trade-off.** `old_path.py` killed only the direct probe at the
  bound. The capture files of `ProcessRunner.runShell` and any child stayed
  behind. The root brief requires positively identified processes and bounded
  cleanup that covers a failed wait. Repair: each probe runs in its own
  session. At the bound, the script records identity (start time and command)
  of the probe and its descendants and the capture files they hold open. It
  kills the group and each identified process, moves the held files into
  `OUT/NAME.captures/` and audits within 5 s. Each trial also compares the
  capture files in the per-user temporary directory before and after it.
  Trade-off: the brief asks for unique temporary directories. Darwin
  Foundation ignores `TMPDIR` for `FileManager.default.temporaryDirectory`,
  and the runner must stay unchanged. Open handles name the files instead. A
  concurrent run in the same directory can make a trial unclean, which fails
  the run and does not hide a leak.

No finding needs a redesign, so no new review chain is due. The changes are
bounded diagnostic tooling, and Taskfile runs verified them.

## Verification

All runs went through the project Taskfile on the k19 host.

- `diagnostic:macos-completion-tools`: ruff check, ruff format and pyright
  pass on both scripts.
- `old-path-03`: verdict `defect`, cap check passed, owned resources clean,
  exit 0. `free-1` probe durations 6.04–6.08 s.
- `single-hop-03`: verdict `clean`, cap check passed, owned resources clean,
  exit 0.
- `cleanup-check-02` (`TRIAL_BOUND=4`): all five `free-1` probes reached the
  bound. Each cleanup killed the probe and moved its two capture files into
  the run. No survivor, no unattributed file, at most 0.28 s each. Verdict
  `mixed`, exit 1, as the report states.
- The per-user temporary directory held 0 capture files before and after
  these three runs.

The first attempt at F3 redirected `TMPDIR`. Its runs `old-path-02`,
`single-hop-02` and `cleanup-check-01` remain as evidence. `cleanup-check-01`
reported clean but left 10 capture files, which are now in
`cleanup-check-01/leaked-captures/`. The report marks the capture audits of
those runs as not valid. The `-01` runs remain unchanged.

## Decisions (running log)

- Kept the old `-01` and intermediate `-02` evidence, and labelled each run
  with the script and probe hashes it used.
- Did not change `ProcessRunner.swift`, production wait semantics or the
  verdict rules. The remaining live work stays with
  `macos-process-fixtures-k185` and `macos-completion-live-k186`.
