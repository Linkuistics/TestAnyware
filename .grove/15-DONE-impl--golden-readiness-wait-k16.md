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

## Decisions (running log)

- 2026-11-23: **the measurement.** A temporary diagnostic in the wait recorded
  each change of state from the polls of the wait. It added no client until the
  wait ended. Three harness runs at `f4cd0c3b` had no observer:

  | Run | Result | First poll | Front process at the end |
  |---|---|---|---|
  | 1 | failed, 137 polls | no menu bar | "Accessibility", pid 557 |
  | 2 | passed, 1 poll | Finder with `File` | not recorded |
  | 3 | failed, 134 polls | no menu bar | `com.apple.AccessibilityUIServer` |

- 2026-11-23: **the observer is not the cause.** Run 2 passed with no observer.
- 2026-11-23: **the cause.** In a failed run, LaunchServices reports
  `AccessibilityUIServer` as the front app. This Apple helper has the type
  `UIElement` and `CanBecomeFrontmost=true`. The agent found no menu bar for the
  focused app in each poll of the failed runs. The screen of run 1 showed Finder
  with its `File` menu, so the guest was ready and the wait read the wrong app.
- 2026-11-23: one poll of run 3 exited 1 with only the build notice on stderr.
  The next poll succeeded. The wait tolerates this, and it is not the cause.
- 2026-11-23: **the repair is in the agent.** When the focused app has no menu
  bar, `agent snapshot` uses the frontmost regular app. A harness-only change
  would leave the same gap in `agent snapshot` for each user of the agent.
- 2026-11-23: the working copy holds the repair and its 5 unit tests in
  `MenuBarOwnerTests`. `task agent:macos:test` failed once, in
  `testADisconnectChangesNothingAndTheRecordIsRetained`, at a load average above
  70. That test passed 3 times alone, and the next full suite passed.
- 2026-11-23: **the live test of the repair is blocked.** It needs a golden with
  the new agent. `task vm:refresh-golden-macos` from this session exited 201 at
  its first SSH connection: `No route to host` (errno 65). With a Linux guest
  running, `/usr/bin/nc` reached port 8648 and Homebrew `python3` got errno 65.
  So this session has no Local Network access for a direct connection. The
  earlier golden and the base stayed, and no setup VM stayed.
- 2026-11-23: **the refresh from Terminal also failed, at a later step.** Its
  first SSH connection succeeded, and boot 1 and the `csrutil disable` boot
  ended. After the reboot, the 60 SSH attempts ended with errno 65. The last
  `tart run` started at 07:34:14Z. The DHCP lease of the guest implies a
  request at about 07:34:18Z, if the lease lasts 1 hour. The earlier golden and
  the base stayed. The cause is not known. A Local Network refusal and a guest
  that did not answer can both give errno 65.
- 2026-11-23: the `csrutil result` line of that run is not a fault. The driver
  matches the confirmation prompt too, and `csrutil status` over SSH is the gate.
- 2026-11-23: a watcher in the scratch directory records `/usr/bin/nc` to port
  22 of the setup VM during the next refresh. `nc` is exempt from the Local
  Network check, so it separates the two causes.
- 2026-11-23: **the second refresh by the owner did not pass boot 1.** The
  setup VM started at 07:45:17Z. `/usr/bin/nc` reached its port 22 from
  07:45:25Z until the VM went at 07:48:23Z. The VM never stopped for Recovery,
  and the golden did not change. The output of that build ends with errno 65
  for the same address. So macOS refused the connection of the build, and the
  guest was not at fault.
- 2026-11-23: `herdr` is a background service, and this session runs inside it.
  `CONTEXT.md` says that nothing under such a service can receive the Local
  Network permission. It is not known which terminal ran each refresh by the
  owner, or why one run passed its first SSH connection.
- 2026-11-23: the owner ran the refresh in a Terminal.app window, outside
  `herdr`. It passed in 8 min 4 s and replaced `testanyware-golden-macos-27`.
  One run in that context does not prove that `herdr` caused the refusals.
- 2026-11-23: the agent in the new golden has the sha256 of this build,
  `b111b2c8…`, and the guest reports macOS 27.2.1.
- 2026-11-23: **the harness passed 2 of 2 runs on the repaired golden, with no
  observer.** Each run saw Finder with `File` at its first poll. After the wait,
  the front app was Finder in run 1 and `AccessibilityUIServer` in run 2.
- 2026-11-23: **a direct test proves the repair.** On 3 fresh guests, a command
  read the front app just before and just after one `agent snapshot`:

  | Boot | Front app, before and after | Menu bar in the snapshot |
  |---|---|---|
  | 1 | `AccessibilityUIServer` | Finder with `File` |
  | 2 | Finder | Finder with `File` |
  | 3 | `AccessibilityUIServer` | Finder with `File` |

- 2026-11-23: while the helper is in front, the snapshot marks no window as
  focused. This leaf does not change that field, because the wait does not use it.
- 2026-11-23: `testanyware-setup-dbf7834f` runs with no build process. It started
  at 07:51:29Z. This session did not start it and did not change it.
- 2026-11-23: on the final code, with the diagnostic removed, `task
  agent:macos:test` passed 83 XCTest and 52 Swift Testing tests. `task cli:test`
  passed: 63 suites, 1752 tests, 0 failed, 13 ignored.
- 2026-11-23: **the release gate moves to the release leaf.** The two passing
  harness runs used the earlier candidate and the temporary diagnostic. The
  release leaf rebuilds its candidates, so it must get its own passing run with
  no observer.
- 2026-11-23: **when the refresh ends.**
  - Keep the temporary diagnostic in the harness for the live runs.
  - Run the harness with no observer until a pass shows
    `com.apple.AccessibilityUIServer` as the front app after the wait.
  - Remove the diagnostic, and do not commit it.
  - Run `task agent:macos:test` and `task cli:test`.
  - The commit moves the release commit, so the release leaf replaces its tag
    and rebuilds its candidates.
