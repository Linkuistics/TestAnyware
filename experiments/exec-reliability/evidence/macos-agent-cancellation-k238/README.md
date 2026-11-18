# Evidence of leaf macos-agent-cancellation-k238

This directory holds the runs of leaf `macos-agent-cancellation-k238`. That
leaf repairs the four findings of review `macos-agent-cancellation-k237`
against the macOS agent of leaf `macos-agent-cancellation-k221`. The running
log in the task file states each decision.

`measure.sh` recorded each run. It
records the source, the dirty diff, the command, the output, the exit and the
time. It also records the digests of every `agents/macos` file and the shipped
agent before and after the run. Each run had equal digests.

| Run | Command | Result |
|---|---|---|
| `corrected-pass-01` | `mise exec -- task agent:macos:test` | Passed, 83 tests, no skip. One test comment was not yet final |
| `old-code/run-01` | `reverts.py` on a scratch copy | Each of 7 cases gave its expected exit |
| `corrected-pass-02` | `mise exec -- task agent:macos:test` | Passed, 83 tests, no skip, on the final source |
| `repeat-01` to `repeat-03` | The five affected test classes | Passed, 47 tests each, on the final source |

## The old-code controls

`reverts.py` puts back one producer fragment at a time in a scratch copy of
`agents/macos`. It runs the new control and restores the file. The copy had
equal digests before and after the run. The run used the test comment that
`corrected-pass-01` used. The final source changed that comment only, and the
two recorded dirty diffs show it.

| Case | Old fragment | Observed failure |
|---|---|---|
| `r1-long-wait-without-a-wake-source` | Each `kevent` wait lasts up to 60 s | Owner lateness 2688 ms and 2684 ms, bound 500 ms |
| `r2-blocking-open` | A blocking open of the download path | The open and the HTTP download of a FIFO each blocked for 1 s |
| `r3-waiter-reads-the-id` | The waiter reads `entries[id]` | The waiter returned `exited(23)` of the second command |
| `r4-startup-skips` | The fixture skips at a startup failure | The control saw `XCTSkip` |
| `r4-log-removed-first` | The fixture removes the log first | The failure lost the agent's output |
| `r4-old-gate-with-a-broken-agent` | The old fixture, with `/usr/bin/false` as the agent | Exit 0, all 11 transport tests skipped |
| `r4-new-gate-with-a-broken-agent` | None, with `/usr/bin/false` as the agent | Exit 1, all 11 transport tests failed |

The R1 control shows the old owner still chose `cancelled`, because the
cancellation time was earlier than the deadline. It woke only at the 3 s
deadline, so its lateness broke the bound.

## Limits

These runs are host tests. No live guest ran. Guest acceptance stays
**0/10, unavailable** until the ten-clone trial of leaf k225.

The Linux agent opens a download path with a blocking `open` too. Its
threading HTTP server gives each request its own thread. A FIFO there holds
only its own request, so it does not block health or another request. This
leaf does not change the Linux agent.
