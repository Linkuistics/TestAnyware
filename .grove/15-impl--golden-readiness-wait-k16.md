# golden-readiness-wait-k16

## Goal

Find why the macOS readiness wait of the Linux host harness fails in some runs.
Then repair the cause, or give the owner the evidence to decide the release.

## Context

- `wait_for_golden_ready` in `linux-host-harness.rs` polls `agent snapshot` for
  120 s. It needs a `File` item in a window of type `menuBar`.
- `focusedAppMenuBar` in `agents/macos/Sources/testanyware-agent/AgentServer.swift`
  returns the menu bar of the focused app. The error names Finder, but the wait
  does not check which app has focus.
- The running log of `host-requirement-release-k4` and commit `8fbb7568` hold
  the records of these runs:

  | Candidate | Harness run | Observer | Readiness wait |
  |---|---|---|---|
  | v2.9.0 | 2 | no | failed |
  | v2.9.0 | 3 | yes | passed |
  | v3.0.0 at `ee1b9e88` | 2 | no | failed |
  | v3.0.0 at `ee1b9e88` | 3 | yes | passed |
  | v3.0.0 at `f4cd0c3b` | 1 | no | failed |
  | v3.0.0 at `f4cd0c3b` | 2 | yes | passed |

- Run 1 of v2.9.0 and run 1 at `ee1b9e88` stopped earlier, at the Local Network
  grant. They did not reach the wait.
- In this table, each run without an observer failed and each run with one
  passed. Six runs cannot prove a cause. The observer is a second client that
  polls `vm list` and `agent snapshot`, so it may change the result.
- Three diagnostics for v2.9.0 polled a macOS clone and saw `File` within 4 s.
  They did not include the OCR provisioning that runs before the wait.
- No failed run kept a snapshot or a screen of its wait.

## Done when

- The harness runs with no second poller, and a failed wait keeps its last
  snapshot and its screen. If no run fails, the task file says so.
- The task file states the number of runs and names the cause from that
  evidence, or it says that the measurement did not find one.
- If a fault in the harness or the product is the cause, this leaf repairs it,
  and a test pins the repair.
- `task cli:test` passes when this leaf changes product code.
- The release leaf has a passing harness run with no observer, or the owner
  accepts the run that had one.

## Notes

- Do not change the wait before the evidence names the cause.
- `task release:linux-harness TAG=v3.0.0` compiles the harness from the working
  tree and runs it on the binary of the candidate. So a temporary diagnostic in
  the harness needs no new candidate.
- A committed change outside `.grove/` moves the release commit. The release
  leaf then replaces its tag and rebuilds its candidates.
- Keep the evidence small. The owner wants few retained test artifacts.
- The state of this Mac is in the last entries of the running log of
  `host-requirement-release-k4`. Read them before you start the app or a guest.
