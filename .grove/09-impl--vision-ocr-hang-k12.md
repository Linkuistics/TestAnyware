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
