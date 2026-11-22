# macos-golden-refresh-run-k13

## Goal

Refresh the macOS 27 golden of this Mac to macOS 27.2.1 with the code of
`macos-golden-refresh-k7`, and pass the live criteria that k7 handed over.
Delete the base and the IPSW that ADR-0027 discards.

## Context

- `macos-golden-refresh-run-k11` owned this goal first. Its second refresh
  passed the fast checks and the install, and then the OCR of the host hung
  at Setup Assistant. `vision-ocr-hang-k12` fixes that hang first. The
  failed builds kept the earlier golden and the older base, as the design
  requires.
- k11 also changed the Setup Assistant driver. It opens Terminal from the
  Utilities folder of Finder, not through Spotlight. Live run 4 of k11
  passed that step.
- The cache holds `macos-27.2.1-26B131.ipsw` (26 GB) and
  `macos-27.0.1-26A434.ipsw` (27 GB). The refresh needs no download.
- k11 deleted `testanyware-golden-macos-tahoe`.

## Done when

- `task vm:refresh-golden-macos` rebuilds `testanyware-golden-macos-27`.
  Afterwards `testanyware-base-macos-27.2.1-26B131` exists, and
  `testanyware-base-macos-27` and `macos-27.0.1-26A434.ipsw` are gone.
- A clone of the new golden reports `27.2.1` from
  `sw_vers -productVersion`. The live gate in `live-vm-gate.rs` passes on
  that golden with `TESTANYWARE_LIVE_VM=1`. This is the golden-build seam
  of the root brief.
- After the refresh, `vm create-golden --platform macos --dry-run` says
  that the base exists, and it names no older base.

## Notes

- With the IPSW cached, the refresh takes about 25 minutes. The install takes
  about 4, Setup Assistant about 7 and golden creation about 15.
- Golden creation connects to the guest directly (ADR-0019), so it needs
  Local Network access. In k11, a binary from the agent session reached the
  guest once and was refused once. If SSH fails with `No route to host`,
  compare with `/usr/bin/nc`, and ask the owner to run the refresh from
  Terminal.
- Golden creation asks the TestAnyware app for a slot. Keep both macOS
  slots free, and allow the Local Network alert if it appears.
- Keep the build logs in scratch, not in the repository.

## Decisions (running log)

- 2026-11-23, plan before the run. The dry run gave the plan of k11 again:
  27.2.1 (26B131), a missing base, the older base
  `testanyware-base-macos-27` and a golden to replace. `doctor` showed the
  app running at 2.9.0, and no VM ran, so both macOS slots were free.
- 2026-11-23, refresh 1. I started `task vm:refresh-golden-macos` from
  this agent session, with the build of commit 395af4c5.
- 2026-11-23, the worker retry ran live. Between the screens "written and
  spoken languages" and "accessibility", the log of refresh 1 printed
  `the Vision worker failed (OCR did not respond in time); trying a new worker`.
  The next screen followed, so the new worker answered. This is
  the first live retry, because no live run of k12 had a hang. "Account
  creation in progress" then passed with no failure.
- 2026-11-23, base ready. Refresh 1 completed Setup Assistant, opened
  Terminal through Finder, and reached SSH from this agent session. It
  reported `testanyware-base-macos-27.2.1-26B131` ready, and then it
  deleted `testanyware-base-macos-27` and `macos-27.0.1-26A434.ipsw`.
- 2026-11-23, refresh 1 failed in Recovery. Boot 1 of golden creation
  passed. In boot 2, the recovery driver waited for the first frame, and
  the decoder failed with
  `ZRLE: palette index 121 out of range (size 64)`. The build exited 201.
  The earlier golden stayed, and the new base stayed. This is a new ZRLE
  fault, not the stale rectangle of k10, and Recovery is a new place.
- 2026-11-23, first recording. A scratch probe calls `capture_frame`
  through a transport that records each server byte. A separate Python
  parser reads the recording. On a Recovery boot of a clone of the new
  base, 81 captures passed. The sequence was the clean sequence of k10:
  1280x720, `DesktopSize` 1024x768, and full rectangles that parse
  exactly. So the fault is intermittent.
- 2026-11-23, placement. The Recovery fault stays in this leaf. This leaf
  already carries the unchanged acceptance of k11 after one handoff, so a
  bounded fix belongs here. A split needs a demonstrated finite unblock.
- 2026-11-23, six more Recovery boots. Six boots after `tart stop` gave no
  fault and the same sequence. The timing shows a gap of about 11 s after
  the first frame. In that gap the probe asked for 1280x720 frames, and
  the server answered nothing. Then `DesktopSize` and full 1024x768
  rectangles came. So the stale header of k10 equals the region of the
  open request at the old size.
- 2026-11-23, reproduction. Eight Recovery boots followed a normal boot of
  75 s and `tart stop`, and the probe used the backoff of `wait_for_text`.
  Boots 1 and 5 failed with the error of refresh 1, at capture 5. Seven
  boots without a normal boot had not failed.
- 2026-11-23, root cause. In both failures, update 4 has the header
  `0,0 1280x720`, and the framebuffer is already 1024x768. Its 95108
  inflated bytes parse exactly only as a grid of 1024x768. The header
  size, the clip of k10 (1024x720) and 1280x768 all fail. A clean boot
  sent the same 95108 bytes under the header `0,0 1024x768`. Captures 2
  and 3 timed out at 1280x720 and left their requests open. So the server
  answered an open request from before the resize. It wrote the region of
  that request in the header, and it encoded its whole framebuffer. In
  k10 the tiles covered only the part inside the framebuffer, and a
  second rectangle covered the rest. So the clip of k10 is one case of
  this behaviour, not all of it.
- 2026-11-23, fix. Only a rectangle whose header reaches outside the
  framebuffer changes. The decoder inflates it once and tries two grids
  in order: the clip of k10, then the framebuffer from the origin of the
  rectangle. It takes the first grid that parses exactly, and otherwise it
  fails as before. A rectangle inside the framebuffer keeps one grid. When
  each tile of the last tile row is solid, both grids can parse, and the
  clip wins. That costs at most one frame with a stale band, because each
  capture asks for a full frame.
- 2026-11-23, test. The fixture test
  `a_late_answer_to_a_request_from_before_a_resize_decodes_the_whole_framebuffer`
  plays the recorded shape with palette RLE tiles. It failed with
  `ZRLE: RLE run overflows tile` before the fix, and it passes after it.
  The test of k10 passes unchanged.
- 2026-11-23, replay. A scratch program fed the recordings of boots 1, 5,
  2 and 3 through the fixed client. All four decoded update 4, and the
  digest of the framebuffer after update 4 was the same in all four. So
  the late answer decodes to the screen that a clean boot sends.
- 2026-11-23, suites. The first run of `task cli:test` failed one test,
  `a_call_that_does_not_answer_within_the_bound_fails_and_leaves_no_process_behind`
  in `testanyware-vm`. Its script must write a PID file within 300 ms.
  Alone, it passed 5 times. The second run passed: 63 suites, 1734 tests,
  0 failed, 13 ignored. The digest of the diff was the same before and
  after each run. `cargo fmt` rewrote files with old drift, so I restored
  them, and I checked my files with `rustfmt --check`.
- 2026-11-23, refresh 2 passed. With the fix, `task vm:refresh-golden-macos`
  used the existing base and rebuilt `testanyware-golden-macos-27`. Both
  Recovery boots passed. In the second one, a Vision worker timed out
  again, and a new worker answered. Afterwards `tart list` named
  `testanyware-base-macos-27.2.1-26B131` and no
  `testanyware-base-macos-27`, and the cache held no
  `macos-27.0.1-26A434.ipsw`. That is the first criterion.
- 2026-11-23, version. A clone of the new golden, `testanyware-a6b2050f`,
  reported `27.2.1` and `26B131` from `sw_vers`. I stopped the clone.
- 2026-11-23, plan after the run. The dry run said that
  `testanyware-base-macos-27.2.1-26B131` exists, and it named no older
  base. The JSON had `base.exists` true and `base.older` empty. That is
  the third criterion.
- 2026-11-23, live gate. `TESTANYWARE_LIVE_VM=1` with the `live-vm-gate`
  test passed all 5 checks on the new golden in 76 s. ZRLE, Tight and Raw
  captures were byte-identical. The OCR check found "File" with the
  engine `vision`. Its comparison with the EasyOCR daemon was skipped,
  because that daemon is not on this host. The test stopped its clone.
  That is the second criterion.
- 2026-11-23, docs. A sweep of the docs, the website docs, `README.md`,
  `LLM_INSTRUCTIONS.md` and `CONTEXT.md` found no text about the stale
  rectangle. Its positive control found the comment in `connection.rs`
  and the fixture, and its cross-tree control found the log of k10. So no
  document changes.
- 2026-11-23, review. This leaf cuts no `review-impl` leaf. The change
  touches one decoder path, which runs only for a header outside the
  framebuffer. A fixture test failed before it, the replay of the real
  bytes matched a clean boot, and the live gate compared three encodings.
- 2026-11-23, clean-up. I deleted the VM `k13-recovery-probe`. The probe,
  the recordings and the logs stay in scratch.
