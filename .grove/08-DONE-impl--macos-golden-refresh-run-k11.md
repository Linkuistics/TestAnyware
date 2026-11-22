# macos-golden-refresh-run-k11

## Goal

Refresh the macOS 27 golden of this Mac to macOS 27.2.1 with the code of
`macos-golden-refresh-k7`, and pass the live criteria that k7 handed over.
Delete the base, the IPSW and the goldens that ADR-0027 discards.

## Context

- `macos-golden-refresh-k7` changed `vm create-golden --platform macos` to
  the rule of ADR-0027, and it added `task vm:refresh-golden-macos`. Its
  running log holds the decisions and the measurements.
- On 2026-11-22, before any refresh, the dry run on this Mac named macOS
  27.2.1 (26B131), the missing base `testanyware-base-macos-27.2.1-26B131`,
  the older base `testanyware-base-macos-27` and the existing golden. That
  is the golden-plan seam of the root brief, and it passed.
- The first refresh stopped at Setup Assistant with a ZRLE fault.
  `setup-assistant-zrle-fault-k10` fixes it first. The failed build kept
  the earlier golden and the older base, as the design requires.
- `setup-assistant-zrle-fault-k10` fixed the ZRLE fault and a race in the
  Setup Assistant driver. `task vm:setup-assistant-live` then passed on a
  never-booted 27.2.1 VM. Its running log holds the evidence.
- The cache holds `macos-27.2.1-26B131.ipsw` (26 GB) and
  `macos-27.0.1-26A434.ipsw` (27 GB). The refresh needs no download.
- The auto-mode permission layer of the k7 session refused
  `testanyware vm delete testanyware-golden-macos-tahoe`. Ask the owner to
  allow that deletion, or to run it.

## Done when

The split of 2026-11-22 narrowed these criteria. The refresh criteria moved
to `macos-golden-refresh-run-k13`, after `vision-ocr-hang-k12`.

- `testanyware-golden-macos-tahoe` is deleted.
- The Setup Assistant driver opens Terminal without a race, and
  `task vm:setup-assistant-live` passes on a never-booted 27.2.1 VM.
- A hung Vision request fails the build with `OCR_TIMEOUT`, and it does not
  wait for ever.
- `task cli:test` passes.

## Notes

- With the IPSW cached, the refresh takes about 25 minutes. The install takes
  about 4, Setup Assistant about 5 and golden creation about 15.
- Golden creation asks the TestAnyware app for a slot. Keep both macOS
  slots free, and allow the Local Network alert if it appears.
- Keep the build logs in scratch, not in the repository.

## Decisions (running log)

- 2026-11-22, tahoe golden. The owner approved the deletion of
  `testanyware-golden-macos-tahoe` in this session.
- 2026-11-22, plan before the run. The dry run gave the plan of k7 again:
  27.2.1 (26B131), a missing base, the older base
  `testanyware-base-macos-27` and a golden to replace.
- 2026-11-22, second refresh attempt. Setup Assistant passed all 24 screens
  and reached the desktop. Then the build opened Terminal through Spotlight,
  and "Last login" did not show within 60 s. The build deleted its
  `-building` VM. The earlier golden and the older base stayed.
- 2026-11-22, placement. The fix stays in this leaf. The refresh already
  passed through one handoff, and a second split would hand on the same
  acceptance with no new evidence.
- 2026-11-22, instrument. `RecoveryError::TextNotFound` now carries the OCR
  read of its last frame, as the stall report of Setup Assistant does.
- 2026-11-22, Vision hang. Live run 1 of `task vm:setup-assistant-live`
  stopped at "account creation in progress" for 20 minutes. A `sample` of
  the test showed the OCR thread blocked in `VNRecognizeTextRequest`, on a
  semaphore in the private framework `TextRecognition`. The unified log
  shows the last Neural Engine request of the test at 19:17:55, and `aned`
  served other processes after it. The macOS engine calls Vision in
  process through `spawn_blocking`, with no time bound, so the driver
  waited for ever. I stopped the run. This fault is separate from the
  Terminal step.
- 2026-11-22, Terminal step. Live run 2 printed OCR reads around the
  step. Two seconds after Cmd-Space, Spotlight was not open yet. After the
  typing, Spotlight showed a first-use panel ("Run Actions and Shortcuts",
  a consent text for search suggestions, and "Continue"), and its field
  showed the placeholder "Spotlight Search". So on a new account the fixed
  pauses of the step race a slow Spotlight and its panel. Run 2 opened
  Terminal, probably because each OCR probe added about a second.
- 2026-11-22, the fix of the Terminal step. The driver opens Terminal from
  the Utilities folder of Finder: Cmd-Shift-U, a click on the label
  "Terminal" when OCR sees it, and Cmd-O. It waits for what each step
  needs, and it tries the route 3 times. Spotlight is not used.
- 2026-11-22, Local Network. Run 2 enabled Remote Login, and then SSH
  failed with `No route to host`. The DHCP lease showed that the guest
  took its address in that boot. With the kept VM running, `/usr/bin/nc`
  reached port 22, and the ad hoc signed Homebrew `python3` got errno 65
  at the same time. So this agent session has no Local Network grant. By
  ADR-0019, golden creation connects directly, so the refresh must run
  from a context with the grant, such as Terminal.
- 2026-11-22, tahoe golden deleted. The first `vm delete` refused with
  `GoldenInUse`, because a tart clone of live run 3 was running. That check
  refuses while any clone runs, by design. With no clone running, the delete
  passed without `--force`, and `tart list` no longer names the golden.
- 2026-11-22, Vision hang again. Live run 3 hung at the same screen,
  "account creation in progress", with the same stack. So the hang hit 2 of
  3 runs, and it blocks the refresh in any context.
- 2026-11-22, the fix of the hang. The macOS engine runs Vision on a
  detached `std::thread` and waits at most 30 s for the result on a
  `oneshot` channel. A timeout gives `OCR_TIMEOUT`. It does not use
  `spawn_blocking`. In tokio 1.52.1 a started blocking task cannot be
  aborted, and the drop of a runtime waits for ever for it (docs.rs, cited
  in `engine.rs`). The screen read of the recovery session first tried 3
  times after a timeout. The message of
  `ResponseTimeout` now names OCR, not the daemon.
- 2026-11-22, live run 4 passed. With the Finder route and the Vision
  deadline, `task vm:setup-assistant-live FRESH=k11-fresh-27.2.1` passed in
  403 s from this session. Terminal opened on the first try, SSH answered,
  and the guest reported macOS 27.2.1. Vision did not hang, so this run did
  not exercise the retry.
- 2026-11-22, Local Network, a second result. The test binary of run 4 reached
  the guest over SSH from this session, and the binary of run 2 did not.
  So this session is not denied in every case. The cause of the difference
  is unknown. The entry above about the grant stays as the record of run 2.
- 2026-11-22, refresh 2 failed on the hang. The second refresh hung at
  "account creation in progress". All 3 OCR requests of the retry timed
  out, so a hung request blocks the later requests of the process. The
  deadline failed the build after about 90 s. The earlier golden and the
  older base stayed. So the hang hit 3 of 5 runs, each time on that screen.
- 2026-11-22, split. The refresh has a demonstrated blocker with a finite
  handoff: a fix of the Vision hang. Its cause is still open, and the hunt
  needs live runs of its own. So, as in k7, the work splits into flat
  siblings, which share the context of the root. `vision-ocr-hang-k12`
  owns the hang. `macos-golden-refresh-run-k13` owns the three refresh
  criteria, unchanged. This leaf retires against the narrowed criteria.
- 2026-11-22, suites. `task cli:test` passed: 63 suites, 1727 tests, 0
  failed, 12 live tests ignored. The diff did not change during the run.
- 2026-11-22, review. This leaf cut no `review-impl` leaf. The tokio call
  carries a docs.rs citation, live run 4 passed the Terminal route, and the
  refresh of k13 tests the whole build again.
- 2026-11-22, clean-up. I kept the never-booted VM `k11-fresh-27.2.1` for
  the live runs of k12, which deletes it. I deleted the VM
  `testanyware-base-livetest`.
- 2026-11-22, no retry. The retry did not recover in refresh 2, and
  it delayed the failure by about 60 s. So the screen read makes one
  request, and the 30 s deadline stays.
- 2026-11-22, final suites. On the final state, without the retry,
  `task cli:test` passed again: 63 suites, 1727 tests, 0 failed, 12 live
  tests ignored. A digest of the diff was the same before and after the run.
