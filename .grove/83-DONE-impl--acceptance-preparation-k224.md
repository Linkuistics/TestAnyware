# acceptance-preparation-k224

## Goal

Isolated production test builds and new goldens exist. The checker rejects
the four corruption controls on those builds. Every subject, ceiling and bound
is frozen before any measurement.

## Context

- Spec: section "Validation route", steps 2 to 5.
- Root brief: sections "Constraints" and "Acceptance inventory and bounds".
- The trial plan and checker of leaf k223, with the review repairs of leaf
  k242. The report `docs/experiments/exec-reliability-acceptance-plan.md`
  states the manifest fields and the remaining limits.
- Golden tasks: `vm:create-golden-macos` and `vm:create-golden-linux`.

## Done when

- Build a private Host CLI and app in an isolated jj workspace.
- Create a new macOS golden and a new Linux golden. Each uses
  `TESTANYWARE_AGENT_BIN_OVERRIDE` with the new agent.
- Record that each retained golden is unchanged and was not booted.
- Run the controller self-check against the private builds.
- Run the four corruption controls against the private builds. The checker
  rejects each one.
- Declare the boot, slot and export bounds. The boot bound limits the whole
  `vm start`, because no clone waits for a slot.
- Write a freeze manifest. It names each build hash, golden, image identity,
  export identity and agent version. It lists every ceiling and bound.
- Give the manifest the `agents` and `app` fields that `acceptance_k223.py`
  defines. Its `sha256` field pins the Host CLI, Tart and the app executable.
- Commit the manifest before any trial clone starts.

## Notes

Do not patch the global CLI or app. Do not stop the installed app without the
human's explicit authorisation. This leaf measures no trial clone.

## Decisions (running log)

- The private builds come from jj workspace `k224-build` at
  `/Users/antony/Development/AbstractingComputers/TestAnyware.k224-build`.
  Its parent is commit `fee86f92`, the k242 integration, which holds every
  reviewed implementation change. Later edits in this workspace cannot change
  the builds.
- The trial Host CLI is a release build. The shipped Host CLI and the app
  bundle are release builds too. The golden recipes use the debug Host CLI of
  the same commit, and the manifest records it apart.
- The boot bound is 420 s, as in leaf k19. The current `vm start` source has
  the same stage polls: 60 s for VNC, 60 s for the IP and 240 s at most for
  the agent. The remaining 60 s cover the clone.
- The slot bound is 30 s, one ordinary observation. No trial clone waits for a
  slot, so the trial does not exercise it.
- The export bound is 1800 s for one `tart export` of a new golden. Leaf k227
  audits an export resource, so each new golden gets one export. The SHA-256
  of the archive is the export identity.
- The image identity of a golden is the SHA-256 of its `config.json`,
  `nvram.bin` and `disk.img`. `image_audit_k224.py` records it, and it
  records the retained images before and after golden creation.
- The corruption controls need real records from the private builds. One
  preparation clone of each new golden supplies them. It runs a subset of the
  k223 cases with the k223 argv. It is not a trial clone.
- 2026-11-18: the human authorised the app handover for this leaf. The
  session quits `/Applications/TestAnyware.app`, runs the private
  `TestAnyware Dev.app`, and reopens the installed app at the end of this
  leaf. Leaf k225 asks for its own interval. The human runs both golden
  recipes from Terminal, because golden creation needs Local Network access.
- `self-check-01` passed in the frozen source state: lint, format, strict
  types and 1,260 tests in 128.5 s, with equal subject digests before and
  after the run. The 24 new controls include a mutation pass: the unmutated
  copy passes, and each of 12 mutants fails at least one control.
- 2026-11-18: the human built both new goldens from Terminal with the debug
  CLI of the build workspace. `testanyware-golden-macos-27-k224` took 12:00:14
  to 12:06:44 UTC, and `testanyware-golden-linux-24.04-k224` took 12:07:10 to
  12:18:58 UTC. Each log ends in `exit 0`.
- `compare-goldens.json` compares `images-before.json` with
  `images-after-goldens.json`. Each of the four retained images is unchanged
  and was not booted. The macOS recipe clones from `testanyware-base-macos-27`,
  and that read left the image unchanged.
- `exports/exports.json`: one `tart export` of each new golden passed within
  the 1800 s bound. The macOS archive has 26,552,642,627 bytes and took 25.0
  s. The Linux archive has 4,935,099,799 bytes and took 20.3 s. Both archives
  stay in `TestAnyware.k224-exports`, outside each jj workspace.
- `manifest.json` has the SHA-256 `c95dab85ab3a...` and 29 pins. Its source is
  commit `fee86f92` with no workspace changes. Each later step reads this
  manifest, and no step rewrites it.
- `prepare-macos-01` failed, and it stays as a failed sample. The `vm start`
  of `acquire` exited 7 after 20.4 s with `VM_BOOT_TIMEOUT`, stage `agent`
  and cause `local_network_denied`. The cleanup found no VM, and the disposal
  and inspection found the clone absent. `doctor-dev-02.json` still
  reports the Local Network grant of the private app as `not_yet_seen`. The
  next sample waits until the human allows Local Network access for
  "TestAnyware Dev". That grant is not a pinned input, so the manifest stays.
- 2026-11-19: the human reported that each TestAnyware, TestAnyware Dev and
  TestAnywareProbe entry under Local Network has the grant. The human wants
  to remove the stale entries. macOS 27.0.1 cannot remove an entry, because
  TN3179 says the remove button came in macOS 27.2. That cleanup waits until
  after this leaf.
- `prepare-macos-02` passed. The `vm start` took 16.4 s, and the guest agent
  hash is the manifest identity `7998bee77cf6...`. Each of the 16 cases
  passes. Each of the five damaged copies of the corruption controls is
  rejected, and the originals stay unchanged.
- `prepare-linux-01` passed. The `vm start` took 20.3 s. The guest listing
  of 11 files hashes to the manifest identity `29ae6a9ab412...`. Each of the
  16 cases passes, and each damaged copy is rejected. The verb
  `diagnostic:acceptance-prepare-check` gives the same verdict for both
  passing samples.
- `images-final.json` follows both preparation clones. Each retained image
  and each new golden is unchanged and stopped. Each of the 29 pins still has
  its manifest digest, as `pins-after-preparation.txt` records.
- 2026-11-19T05:37:27Z: the session quit only the private app, by its bundle
  identifier, and reopened `/Applications/TestAnyware.app`. The installed app
  answers as build 2.8.0, as `restore.txt` and `doctor-restored.json` record.
- `docs/experiments/exec-reliability-acceptance-preparation.md` is the
  durable report. It lists the residual resources for leaf k227.
- Inserted acceptance-preparation-k243 with kind review-impl before
  lifecycle-acceptance-k225. The freeze gates ten trial clones, so an
  independent review reads it first. The review changes no pinned file. This
  session has no provider evidence, so the review has no Creator line.
