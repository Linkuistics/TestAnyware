# clone-disk-size-increments-k56

## Goal

Cut the increments that give each clone of the Linux golden about 36G of free disk, by the design of `clone-disk-size-k54`.

## Context

- The design is the decision log of `clone-disk-size-k54`. Find it with `grove-llm resolve clone-disk-size-k54`. It holds the measurements, the rejected options and the reasons.
- The design in short: `vm create-golden --platform linux` grows the setup VM's disk to 50 GB before its first boot. cloud-init in the guest then grows `/` by itself. `vm start`, the app and its protocol do not change.
- Golden creation for Linux is in `cli-rs/crates/testanyware-vm/src/golden_linux.rs`. The size constant of the macOS base, `BASE_DISK_GB` in `ipsw_base.rs`, is the model for the new constant and its comment.
- tart grows a disk with `tart set <vm> --disk-size <GB>`, and it cannot shrink one. `tart get <vm> --format json` reports the current size.
- The user guide for goldens is `docs/user/golden-images.md`. Its Linux section gets the size and the remedy for an older golden.
- `Taskfile.yml` holds the live checks of `vm start`, under the `vm:` prefix. The live proof fits there as a task. Such a task starts a clone of the Linux golden and reads its free space.

## Done when

- The tree holds the increments of the design, each a vertical slice that a session can prove alone. One `impl` leaf may be enough.
- Each increment states its proof: the unit test of the size check, and a live build of the Linux golden on this Mac.
- The tree records when the change ships. The issue #8 release (v2.4.0) is already published, so the change needs a later release or waits for one.

## Notes

- The owner chose a separate delivery for this work on 2026-10-21. It is outside the issue #8 delivery.
- The design keeps every command, flag, exit code and `--json` field. Keep it so: many projects drive TestAnyware through its CLI.
- An older golden needs no rebuild. `tart set testanyware-golden-linux-24.04 --disk-size 50` gives its clones the space. Ask the owner before a session runs it on this Mac, because tart cannot shrink the disk again.
- The Linux host harness keeps `--no-cache-dir` in its OCR install. The flag keeps the OCR band working on an older golden.
