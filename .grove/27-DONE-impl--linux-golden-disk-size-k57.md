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

## Decisions (running log)

- **The code, the tests, the guide and the two Taskfile tasks are written; the live proof is not.** `LINUX_DISK_GB` (50) and `MIN_ROOT_FS_BYTES` (40 GB, decimal) sit in `golden_linux.rs`. `tart.rs` gains `parse_disk_gb`, `disk_gb` and `set_disk_size`, each with an `_in` variant for the injected `ToolSearch`. `tart get --format json` reports `Disk` as an integer in GB; I read that on tart 2.40.1 from the owner's golden (`"Disk" : 20`), not from memory.
- **The growth decision is `needs_disk_growth(Option<u32>)`.** It grows a disk below 50 GB, keeps one at 50 GB or more, and tries the growth when the size is unknown, because tart never shrinks a disk. A failed growth removes the setup VM and fails the build.
- **The size check reads bytes, not a human unit.** The command is `timeout 300 cloud-init status --wait >/dev/null 2>&1; df -B1 --output=size /`. The `;` is deliberate: a degraded cloud-init must not hide the size. `check_root_fs_size` takes the last non-empty line, so a header line or a trailing newline does not matter, and an output that is not a number gives its own error. It runs after the agent-health gate and before `finalize_and_shutdown`, while sshd is open.
- **A second Taskfile task was needed: `vm:create-golden-linux`.** It mirrors `vm:create-golden-macos` and builds under `NAME`, with `TESTANYWARE_AGENT_BIN_OVERRIDE` set to `agents/linux`. The leaf named only the live-check task, but the proof golden needs a way to be built from this checkout, and a scratch name keeps it off the owner's golden.
- **`task cli:fmt` cannot pass on these two files, with or without this change.** The Taskfile says the repo has pre-existing drift and forbids repo-wide runs. I formatted every line I added as rustfmt wants and left the older drift alone. `task cli:clippy` shows the old `result_large_err` warning for `provision_and_finalize` (same signature as before) and no warning from my lines. I removed one of mine: `Option::is_none_or` needs Rust 1.82 and the crate's MSRV is 1.81.
- **Live run 1 on this Mac, 2026-10-22, from an agent session: the build reached the growth and then failed at SSH.** `tart get` showed the setup VM at 50 GB after `Growing the disk of … from 20 GB to 50 GB...`, and the guest got an IP. Then `connect 192.168.64.16:22: No route to host (os error 65)`. That is the Local Network denial of this session, and `vm create-golden` is the one command that stays on the direct path. The failure path tore the setup VM down, as designed. `tart list` shows no leftover.
- **So the live proof needs a terminal that may reach guest IPs.** The leaf's own notes said so. The sizes and the times are not measured yet, and the leaf stays live.
- **Live run 2 passed, from Terminal on this Mac, on 2026-10-22.** The owner pointed out that a `.command` file opened with `open -a Terminal` runs outside the Local Network denial of an agent session. I built `k57-scratch-golden` that way. The disk went from 20 GB to 50 GB before the first boot, and the size gate read `/` as 47.3 GB. The build took 997 s and ended with `Golden image 'k57-scratch-golden' created successfully`.
- **A clone of the scratch golden reported 47310487552 bytes on `/`, with 38850174976 bytes (38.9 GB) available.** The design predicted 36 GB available, and the leaf asks for at least 30 GB. The clone started in 20 s. `vm:linux-golden-disk-live` stopped and deleted every clone. A negative control with `MIN_AVAIL_GB=60` failed with the expected message, so the threshold check can fail. I deleted the scratch golden afterwards. `tart list` shows the same VMs as before the run, and the owner's `testanyware-golden-linux-24.04` still has 20 GB.
- **Two bugs of the live task showed only on the live run.** The first was in `read`: `IFS` was newline and tab, so a line of two numbers did not split. The second was a stray backslash that I typed in the fix. Both are fixed. The earlier failed runs also proved the cleanup path: no clone stayed after a `fail`.
- **`cargo test` passes in the whole workspace, and `task cli:clippy` adds no warning.** The one warning in `golden_linux.rs` is the old `result_large_err` of `provision_and_finalize`. `task cli:fmt` fails on the older drift of the two files, as the Taskfile says it may. My added lines are rustfmt-clean.
- **The change ships with the next release of TestAnyware.** The v2.4.0 release is published without it. No release leaf exists, by the decision of `clone-disk-size-increments-k56`. A candidate built after this commit contains it.
- **The owner's new concern became a separate leaf.** The owner asked on 2026-10-22 for a minimal configuration file to be installed when none is found. It reopens ADR-0021, so I cut `default-config-file-k58` as a `design` leaf and did not build it here.
