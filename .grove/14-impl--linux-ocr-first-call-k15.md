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
