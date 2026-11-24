> Historical report. Paths and commands refer to the [archived checkout](../../README.md).

# Evidence of leaf macos-agent-cancellation-k221

This directory holds the runs of leaf `macos-agent-cancellation-k221`. The
leaf's running log in the task file states what each run shows. `measure.sh`
recorded each run except the k7 run. The k7 task keeps its own records in
`download-k7-01-outer`.

| Run | Command | Result |
|---|---|---|
| `corrected-pass-01` | `mise exec -- task agent:macos:test` | Passed, 77 tests, with the first disconnect control |
| `old-fail-01` | `AgentProcessTests` against the shipped agent | 8 of 10 failed, with the first disconnect control |
| `download-k7-01` | `task diagnostic:download-consistency-local` | Exit 0. The shipped agent gave failure-D bytes, and the checkout agent gave whole versions |
| `mutants/run-01` | `mutants.py` on a scratch copy | Each of 5 mutants failed its control |
| `repeat-01`, `repeat-02` | The new and owner test classes | Passed, with the first disconnect control |
| `repeat-03` | The same | Failed the half-close case. The control read the record too early |
| `corrected-pass-02` | `mise exec -- task agent:macos:test` | Passed, 77 tests |
| `old-fail-02` | `AgentProcessTests` against the shipped agent | 8 of 10 failed |
| `repeat-04` to `repeat-06` | The new and owner test classes | Passed, 61 tests each |

The shipped agent is
`/opt/homebrew/opt/testanyware/share/testanyware/agents/macos/testanyware-agent`.
Its SHA-256 is in each `subjects-before.sha256`. The `old-fail-*-outer`
directories hold process snapshots before and after the old-fail runs.

## The recorded source diffs

Each run recorded `jj diff --git` of the whole working copy. That diff also held
this directory, so each run copied the earlier runs' records, and the files
doubled from run to run. `filter_dirty_diff.py` later removed every section
under this directory from each `source-dirty.diff`. Each `source-dirty.diff.note`
keeps the original size and SHA-256.

The filtered diffs fall into two source
states. The first six runs share one, and the five runs from
`corrected-pass-02` on share the other. `measure.sh` now leaves this directory
out of the diff.
