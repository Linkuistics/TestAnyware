# linux-ocr-first-call-k15

## Goal

Find why `screen find-text` of the aarch64 Linux binary ended with
`OCR_TIMEOUT` in the third harness run of the v3.0.0 candidate. Then remove
that failure from the release gate of `host-requirement-release-k4`.

## Context

- The failed run is in the running log of `host-requirement-release-k4`.
- The Linux path uses the EasyOCR daemon through `OcrChildBridge`. Its
  first-call deadline is 15 s, in `OcrChildBridgeConfig::new` of
  `cli-rs/crates/testanyware-ocr-client/src/bridge.rs`. The ready handshake has
  its own 8 s deadline. A miss of the handshake reports another error.
- This grove did not change the Linux OCR path. It changed the Vision path of
  macOS, and the text of the timeout error. It also refreshed the Linux golden
  (`linux-golden-refresh-k8`) and the macOS golden
  (`macos-golden-refresh-run-k13`).
- `harness-ocr-recognizer-k55` (commit `f39941d93f`) measured a cold
  `find-text` at 12.6 s, a margin of 2.4 s. It recorded: "If a slower host
  fails on it, the deadline needs its own leaf."
- On 2026-10-21 a session raised this deadline to 60 s for the same symptom.
  It then found that the deadline was not the cause, and it reverted the change
  (commit `315d22b727`).
- `check_find_text` in `linux-host-harness.rs` runs the call. The harness
  provisions the venv with `provision_ocr`, and then a macOS guest runs beside
  the Linux guest.
- In the failed run, the macOS guest showed a Finder window `Utilities` and a
  `MenuBarAgent` dialog. So the frame may hold more text than the frames of
  earlier releases. No measurement tests this.

## Done when

- A measurement of cold `find-text` calls under harness conditions records
  the ready time, the first-read time and the total of each call. Harness
  conditions are the current Linux golden, torch 2.12.0, and a macOS guest
  beside it with a 1920x1080 frame.
- The task file names the cause from that measurement, or says that the
  measurement did not find one.
- If the first-call deadline is the cause, a change sets it from the
  measurement, and a test pins it. `docs/reference/` and the comment in
  `check_find_text` state the new value.
- If another fault is the cause, this leaf repairs it, or it asks the owner.
- `task cli:test` passes when this leaf changes product code.

## Notes

- Do not raise the deadline before the measurement names it as the cause.
- `task release:linux-harness` needs a candidate. Use a direct measurement
  while the cause is open.
- The state of this Mac is in the last entry of the running log of
  `host-requirement-release-k4`. Read it before you start the app or a guest.
- A product change moves the release commit. The release leaf then replaces
  the local tags and rebuilds its candidates.

## Decisions (running log)

- 2026-11-23: **the code path.** The CLI captures the frame first, and then
  `recognize` starts the daemon. The daemon sends `ready` before it imports
  torch. So the 15 s first-call deadline holds the torch import, the model load
  and the first read. The claim in `vision/stages/text-ocr/README.md` that the
  load overlaps the capture is false.
- 2026-11-23: **the first measurement.** The conditions were the current Linux
  golden with 4 vCPUs, torch 2.12.0, and a macOS guest beside it with a
  1920x1080 frame. Each of 12 calls started a new daemon on the same frame and
  read 49 detections.
  - The ready time was 0.022 s to 0.214 s, so the 8 s deadline is not at risk.
  - With the page cache kept, the first reads took 11.3, 16.2, 22.7, 20.4, 15.1
    and 13.6 s.
  - With the page cache dropped, they took 14.7, 31.5, 17.7, 14.7, 13.4 and
    10.4 s.
  - Six of the 12 calls exceeded 15 s. The cause of the spread is still open.
- 2026-11-23: **the content of the frame is not the cause.** Eight cold calls
  on the full frame (49 detections) had a median of 9.3 s. Eight on a copy with
  the Finder window painted out (10 detections) had a median of 8.2 s. None of
  the 16 exceeded 15 s. Guest CPU time cannot show starvation here, because it
  rose with wall time (correlation 0.98).
- 2026-11-23: **the cause is CPU contention on the host, against a deadline
  with too little margin.** Twenty-four cold daemon calls ran on the full frame,
  each joined to the idle CPU of the host from `iostat`.

  | Host idle CPU during the call | Calls | Median read | Slowest | Over 15 s |
  |---|---|---|---|---|
  | Under 10% | 6 | 16.6 s | 21.9 s | 4 |
  | 10% to 25% | 4 | 11.5 s | 13.3 s | 0 |
  | 25% or more | 14 | 9.1 s | 10.1 s | 0 |

  - The correlation of read time with idle CPU is -0.84.
  - The slowest call of all 36 daemon calls took 31.5 s. Its run had a median
    one-minute host load of 100.
  - Other work of the owner loaded this Mac during each run.
  - No idle-CPU record exists for the failed harness run. So this measurement
    reproduces the failure mode, and it does not observe that run.
- 2026-11-23: **the first-call deadline of the EasyOCR daemon is now 60 s.**
  This is 1.9 times the slowest measured call. The ready deadline stays 8 s,
  and the deadline of each later read stays 15 s. The Vision engine sets its
  own deadlines, so this change does not affect it.
- 2026-11-23: **two points stay open, and this leaf does not change them.**
  - Nobody measured a warm read under contention. A warm read may miss its 15 s
    deadline on a host with no idle CPU. The harness makes one read, so this
    does not affect the release gate.
  - The remediation of `OCR_TIMEOUT` says to increase the OCR deadline, but no
    CLI option or environment variable sets that deadline.
- 2026-11-23: **verification.** `task cli:test` passed with 1752 tests, 0
  failures and 13 ignored. The 9 tests of `ocr_analyzer` passed. No Linux
  harness ran on a binary with the new deadline, because that needs a new
  candidate. The release leaf runs it.
