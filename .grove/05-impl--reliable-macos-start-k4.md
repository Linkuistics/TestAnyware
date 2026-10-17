# reliable-macos-start-k4

## Goal

Find why macOS starts on the macOS 27 host fail while the guest's agent is in
fact healthy (issue #3, finding 1), fix it, and prove the fix against the
grove's done line: 10 consecutive healthy starts and a passing Koine
verification run.

## Context

- Decisions in the retired `plan-k1`: D1 (the done line), and D6's live
  checks (c) and (d).
- The diagnostics `start-failure-tart-k2` added: every failed start reports
  its stage and its last probe error with the OS errno, `--keep-on-failure`
  keeps the VM for a post-mortem, and the Taskfile's reliability loop repeats
  starts.
- The root brief's Notes list the hypotheses the first failure's error kind
  should tell apart, and what k2 observed: the first failure was already
  `No route to host (os error 65)` while `/usr/bin/curl` reached the same
  healthy agent — start there.
- Koine: `scripts/vm-verify-lib.sh` (`start_vm`, `guest`) and
  `task app:vm-verify-homebrew`, in `~/Development/Koine`.

## Done when

- The cause of finding 1 is identified from evidence — the probe's error kind
  plus a post-mortem of a kept VM — and fixed, with a regression test at the
  nearest seam where one exists.
- The Taskfile reliability loop passes: 10 consecutive
  `vm start --platform macos` reach a Healthy agent.
- Koine's `task app:vm-verify-homebrew` passes once.
- A healthy macOS start also shows the resolution switch applied
  (`testanyware screen size` reports 1920×1080). `start-failure-tart-k2` could
  not check this live: from its harness no start reached a Healthy agent, so
  the criterion "a normal macOS start still succeeds with the switch applied"
  was handed to this leaf.
- If the failure never reproduces with the diagnostics in place, the leaf says
  so plainly — bar met, root cause unknown — instead of claiming a cause.

## Notes

- Cloning and starting a VM is cheap, so repeat freely, but stop every VM you
  start: the macOS guest limit is two.
- Leave #2 (the VNC-idle crash) alone unless the evidence points at it.
