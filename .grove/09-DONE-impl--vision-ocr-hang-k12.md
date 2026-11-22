# vision-ocr-hang-k12

## Goal

Make the macOS base build pass the Setup Assistant screen "account creation
in progress" on macOS 27.2.1. On that screen, the Apple Vision OCR of the
host hangs in 3 of 5 runs, and the base build fails.

## Context

- `macos-golden-refresh-run-k11` found the hang. Its running log holds the
  evidence. These are the facts:
  - The hang occurred in 3 of 5 runs: live runs 1 and 3 of
    `task vm:setup-assistant-live`, and the second refresh. Each time, it
    was at "account creation in progress". Live runs 2 and 4 and the first
    refresh passed that screen.
  - A `sample` of the hung process showed the OCR thread in
    `VNRecognizeTextRequest`. It waited on a semaphore in the private
    framework `TextRecognition`. Two `ANEServicesThread` threads waited for
    Mach messages. The unified log showed the last request as a frame of
    1024x768, and nothing after it. `aned` served other processes.
  - k11 bounded the Vision call. `recognize_with_vision` in
    `testanyware-ocr-client/src/engine.rs` runs Vision on a detached thread
    and waits 30 s at most. In the second refresh, `read_screen` in
    `testanyware-vm/src/recovery.rs` tried 3 times, and all 3 requests timed
    out. So a hung request blocks the later requests of the same process.
    k11 then took that retry out.
- The cause is not known. These are open questions, not findings:
  - Does one frame of that screen make Vision hang? A frame saved at the
    first timeout, and an OCR of it in a new process, can answer this.
  - Does a request that does not use the Neural Engine avoid the hang?
    Vision has a compute device setting for each request. Verify its API
    and version in the source before use (`grove-impl`).
  - Can a request in a new process recover after a hang? ADR-0003 chose
    Vision in process. A child process that the driver can stop would
    change that decision.
- The never-booted VM `k11-fresh-27.2.1` exists for
  `task vm:setup-assistant-live FRESH=k11-fresh-27.2.1`. A run takes about
  7 minutes, and it needs no reinstall.

## Done when

- The log records the cause of the hang, or an offline reproduction from a
  saved frame, or a statement that neither was found and why.
- The OCR engine avoids the hang, or the driver survives it. A unit test
  covers the new behaviour where a seam allows one.
- `task vm:setup-assistant-live FRESH=k11-fresh-27.2.1` passes 3 times in a
  row, with no OCR timeout. Each run completes `verify_base`.
- If the fix changes the decision of ADR-0003, the ADR set records the new
  decision.
- `task cli:test` passes. The VM `k11-fresh-27.2.1` and the VM
  `testanyware-base-livetest` are deleted at the end.

## Notes

- A run can also fail with `No route to host` at its SSH step. In k11 the
  test binary of live run 2 got that error, and the binary of live run 4
  did not. The platform `/usr/bin/nc` reached the guest in both cases
  (ADR-0019). That failure is not this hang.
- Keep the logs and the saved frames in scratch, not in the repository.

## Decisions (running log)

- 2026-11-22, offline reproduction. A scratch harness repeats the Vision
  call sequence of `vision.rs` on one synthetic 1024x768 PNG. In one
  process, request 444 hung, and in another, request 205 hung. A sample of
  the hung harness shows the stack of k11: the serial queue
  `VN.detectorSyncTasksQueue.VNCRImageReaderDetector` waits on a semaphore
  in `TextRecognition`, and the two `ANEServicesThread` threads wait for
  Mach messages. A second request after the hang also hung for 30 s. So the
  hang is a chance event for each request, and no frame of the screen
  causes it. The screen "account creation in progress" shows the hang
  most often because the driver polls it for about 30 s, with about one
  request each second.
- 2026-11-22, the first request of a new binary path. A copy of the
  harness at a new path took more than 30 s for its first request, and
  its second run took 15.6 s. A sample shows `TextRecognition` in
  `_ANEClient compileModel`, a synchronous XPC call to `aned`. A rebuild
  at the same path took 205 ms. So `aned` caches the compiled model for
  each binary path. The CPU compute device and `usesCPUOnly` do not stop
  this compile.
- 2026-11-22, the CPU device does not avoid the hang. Vision documents
  `setComputeDevice(_:for:)` and `MLCPUComputeDevice` from macOS 14. With
  the CPU on the main stage, five runs of up to 3000 requests hung at
  requests 1576, 1576 and 2157, and two ran clean: 3 hangs in about 12700
  requests. The device of Vision gave 4 hangs in about 6100 requests. The
  digests of the harness and the frame were the same before and after. So
  that setting is not a fix, and the engine does not use it.
- 2026-11-22, no recovery in the process. In two runs, after the hang
  at requests 433 and 4467, the probe sent five more requests, each with
  a 20 s bound: a minimum text height, a custom word, revision 2, no
  language correction, and the setting of the engine. All five hung. Each
  setting had returned in less than 250 ms before the stress. So the hang
  wedges Vision for the whole process, and only a new process recovers.
  Each harness run after a hang started clean.
- 2026-11-22, the driver survives the hang in a new process. The macOS
  engine runs Vision in a worker child: the `testanyware` binary with the
  hidden command `_vision-ocr`, as `_vnc-keeper` was. The child speaks the
  protocol of `OcrChildBridge`, so the bridge owns the deadlines, the kill
  after a timeout and the new child on the next call. The engine retries
  a request once after a timeout or a crash, on a new child. A test binary
  is not the CLI, so `TESTANYWARE_VISION_WORKER` names the worker binary,
  and the live tasks set it. This changes the "in process" part of
  ADR-0002 and ADR-0003, so a new ADR records it.
- 2026-11-22, deadlines. The first call of a worker may compile the model
  for a new binary path, which took 28 s uninterrupted. So the first call
  has 90 s, and a later call has the 30 s of k11.
- 2026-11-22, the compile also follows a rebuild. The first request of the
  rebuilt `target/debug/testanyware _vision-ocr` took 36 s, and the next
  process took 0.2 s from start to answer. A rebuild of the harness at the
  same path had been fast, so the key of the cache of `aned` is not known.
  The 90 s first deadline covers both cases.
- 2026-11-22, suites. On the final sources, `task cli:test` passed: 63
  suites, 1733 tests, 0 failed, 13 ignored. The new tests are the three of
  the worker, the three of the engine, and the ignored stress test. A
  digest of the diff was the same before and after the run. Without the
  retry, the test `vision_survives_a_hung_worker_on_a_new_worker` failed.
  Builds for Linux and Windows and clippy of the OCR client were clean.
- 2026-11-22, live runs. `task vm:setup-assistant-live
  FRESH=k11-fresh-27.2.1` passed 3 times in a row, in 436 s, 453 s and
  424 s. Each run completed `verify_base` and reported macOS 27.2.1. No run
  printed a worker failure, so no OCR timeout occurred, and the retry did
  not run live. The digest of `target/debug/testanyware` was the same
  before and after. A first loop stopped after one boot, because I edited
  a comment of `recovery.rs` while it ran.
- 2026-11-22, review. This leaf cuts no `review-impl` leaf. The design
  keeps the bridge of the EasyOCR daemon, which its own tests cover, and
  the retry has a test that fails without it. ADR-0002 states how to go
  back to Vision in process. The refresh of k13 tests the whole build
  again.
- 2026-11-23, clean-up. The permission system refused `tart delete`. The
  owner approved the deletion, and I deleted the VMs `k11-fresh-27.2.1`
  and `testanyware-base-livetest`. `tart list` names neither.
