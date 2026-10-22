# linux-golden-disk-size-k57

## Goal

Give each clone of the Linux golden about 36 GB of free disk. `vm create-golden --platform linux` grows the setup VM's disk to 50 GB before its first boot. The golden then carries the size, and cloud-init in the guest grows `/` by itself.

## Context

- The design is the decision log of `clone-disk-size-k54`. Find it with `grove-llm resolve clone-disk-size-k54`. It holds the measurements, the rejected options and the reasons. Do not reopen them.
- The planning cut is in `clone-disk-size-increments-k56`. This is the only increment, and it is one slice. The size check fails today's 17 GB disk, so it cannot ship without the growth.
- The code is `cli-rs/crates/testanyware-vm/src/golden_linux.rs`.
  - `create_golden_linux` clones the vanilla image into the setup VM with `crate::tart::clone`. The growth goes between that clone and `run_detached`. The disk has never booted at that point.
  - `provision_and_finalize` holds the SSH steps. The sshd of the setup VM stays open until `finalize_and_shutdown` disables it. So the size check must run before that call.
  - The module doc says: pure helpers have unit tests, and a golden built on this Mac proves the live sequence. Keep that policy.
- `BASE_DISK_GB` in `cli-rs/crates/testanyware-vm/src/ipsw_base.rs` is the model for the new constant and its comment. `set_display_in` in `tart.rs` is the model for a `tart set <vm> --disk-size <GB>` wrapper. Follow its `_in` variant with a `ToolSearch`.
- tart grows a disk and never shrinks one. `tart get <vm> --format json` reports the current size. When a future Cirrus image has 50 GB or more, golden creation must keep that disk. So grow only when the current size is smaller. A pure helper can decide that.
- The check reads the size of `/` over SSH. It fails the build when `/` is smaller than 40 GB. The grown file system had 47.3 GB. Today's file system fits in an 18.9 GB partition. A pure helper reads the output and decides. Choose one unit for the output, state it in a comment, and keep both margins.
- When SSH answers, cloud-init may not have grown `/` yet. The design measured the growth at the boot of a clone, not at the first boot of the setup VM. So read the size after `cloud-init status --wait`, or after the apply-settings reboot. Do not trust a read taken at the first SSH contact.
- The user guide for goldens is `docs/user/golden-images.md`. Its Linux section gets the disk size and the remedy for an older golden.
- `Taskfile.yml` holds the live checks of the `vm:` prefix. `vm:create-golden-macos` is the model for a task that builds a golden under a new `NAME`. `scripts/vnc-keeper-live.sh` is the model for a script that starts clones and always stops them.

## Done when

- A named constant holds the 50 GB. Its comment says why 50, that tart can only grow a disk, and that the host pays for no space until the guest writes.
- Golden creation grows the setup VM's disk before its first boot, and keeps a larger disk as it is.
- Golden creation reads the size of `/` over SSH and fails the build when it is smaller than 40 GB. A failure tears down the setup VM, like every other failure of the flow.
- Unit tests cover the growth decision and the size check. They include the 47.3 GB and 18.9 GB cases and an output that is not a number.
- The Linux section of `docs/user/golden-images.md` states the size, and the command `tart set testanyware-golden-linux-24.04 --disk-size 50` for an older golden.
- A Taskfile task proves the sequence live. It starts a clone of a named Linux golden, reads `df` of `/`, and stops and deletes the clone on every path. The task is documented in the `desc` of the Taskfile entry.
- The live proof ran on this Mac. A new Linux golden was built under a scratch name. A clone of it reported at least 30 GB available on `/`. The decision log records the measured sizes and the times. The scratch golden and its clones are deleted afterwards.
- `cargo test`, `task cli:clippy` and `task cli:fmt` pass.
- The diff changes no command, flag, environment variable, exit code or `--json` field. `vm start`, the app and the app's protocol are untouched.

## Notes

- Do not run `tart set` on `testanyware-golden-linux-24.04` on this Mac. The owner decides that, because tart cannot shrink the disk again. Build the proof golden under a scratch name, so a rebuild cannot replace the owner's golden.
- The Linux host harness keeps `--no-cache-dir` in its OCR install. Leave it. It keeps the OCR band working on a golden that was built before this change.
- The design makes no change to a macOS clone or to a QEMU clone. Do not touch them.
- A golden build pulls an image and installs packages, so it needs the network and a long run. It starts a guest, so it needs a free macOS guest slot. Run it from a terminal that may reach guest IPs.
- The change ships with the next release of TestAnyware. The issue #8 release (v2.4.0) is already published. The owner decides when a later release happens, and a release for this one constant alone is not worth the cost of the release pipeline. So planning cut no release leaf. A release candidate built after this leaf contains the change.
- If a second in-session review is needed, cut a `review-impl` leaf.
