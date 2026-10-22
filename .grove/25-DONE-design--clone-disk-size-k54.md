# clone-disk-size-k54

## Goal

Decide how a clone of a golden gets more disk space than its golden has. The owner asked for it on 2026-10-21, while `harness-golden-hut-k49` ran the OCR band.

## Context

- The Linux golden `testanyware-golden-linux-24.04` has a 17 GB root file system. A clone of it starts with 8.8 GB free.
- `pip install easyocr` in a clone filled that space. The wheels of torch and the CUDA libraries took 3 GB in pip's cache and about 6 GB in the venv. The install failed with `No space left on device`. `harness-golden-hut-k49` works around it with `--no-cache-dir`. The install then ends with 3.3 GB free.
- The owner chose a separate delivery on 2026-10-21. Two other options were on the table. One grows every tart clone in `vm start`. The other rebuilds the golden larger.
- The golden is a tart guest. Cirrus images grow the root partition on boot, so a larger disk may need no guest step. Check that on the Linux golden.
- `vm start` is a command with a fixed meaning (the brief). A change to it must keep every flag, exit code and `--json` field.

## Done when

- A design records where the size is set. The candidates are an option of `vm start`, the `config.rhai` file, a fixed rule for the Linux platform, and a rebuild of the golden. It records the rejected options and the reason.
- The design says whether a macOS clone changes, and what a QEMU clone does.
- The design says what the guest does with the extra space, and how a test proves it.
- The spec or an ADR carries the decision, if the decision changes a behaviour that they state.

## Notes

- The tree places this leaf after `app-release-k21`, because `leaf-add` appends at the end. The owner may want it before the release. That is a decision for the next planning session. The OCR band passes without it.
- This leaf is outside the issue #8 delivery. It exists so the request is not lost.

## Decisions (running log)

- **The guest grows its own root file system, and no guest step is needed.** I measured it on this Mac on 2026-10-22. I cloned `testanyware-golden-linux-24.04` to a scratch VM and ran `tart set --disk-size 50` on the copy. Then `vm start --base` started a guest from it through the app. cloud-init's `growpart` module grew `/dev/vda1` from 18.9 GB to 48.9 GB in 0.46 s. Its `resizefs` module then ran `resize2fs` in 0.06 s.
- **The result: `df -h /` reported 45G with 36G available.** A clone of today's golden has 8.8G available (`harness-golden-hut-k49`). The start took 18.8 s, which is the usual time. I stopped the guest and deleted the scratch VM afterwards.
- **cloud-init grows the disk, not `cloud-initramfs-growroot`.** That package is absent from the golden, so the tart FAQ names the wrong mechanism for it. Cirrus Labs keeps cloud-init enabled with no data source for this reason, in `/etc/cloud/cloud.cfg.d/99_cirruslabs.cfg`. Both modules run at each boot. Golden provisioning keeps cloud-init and `cloud-guest-utils`, the package that holds `growpart`.
- **A larger disk costs the host no space until the guest writes.** tart's `disk.img` is a sparse raw file. After the growth to 50 GB, the copy still had 9 GB allocated, the same as the golden.
- **Golden creation sets the size.** `vm create-golden --platform linux` grows the setup VM's disk to 50 GB before its first boot. The golden then carries the size, and each clone inherits it. When a future Cirrus image has 50 GB or more, golden creation keeps that disk, because tart can only grow a disk.
- **Why 50 GB.** It is the disk of the macOS base VMs (`BASE_DISK_GB`) and of the Cirrus vanilla macOS images, and the default of `tart create`. The Windows golden has 64 GB. The Linux golden is the only golden with Cirrus's 20 GB, because golden creation clones the Ubuntu image and never resizes it.
- **The decision keeps every command, flag, exit code and `--json` field.** `vm start` and the app's protocol do not change. No `--json` field reports a disk size.
- **Rejected: an option of `vm start`, such as `--disk-size`.** On a macOS host the option is a new field of the app's start request, so the app and its protocol change too. It works only for Linux on tart. A macOS guest cannot grow its APFS container outside recoveryOS, and a Windows guest does not extend its partition by itself. The need has one consumer today, the OCR band of the Linux host harness, and 50 GB serves it. A later option stays possible as an addition.
- **Rejected: a key in `config.rhai`.** A host without the file, such as a new Mac, would still give its clones 8.8G available. So the harness could not rely on the key. The app reads the file only when it starts. ADR-0021 limits the file to tool locations and directories, and it adds policy only with the deliveries that need it.
- **Rejected: a fixed rule for the Linux platform in `vm start`, which grows each tart clone.** On a macOS host the app runs the start, so the rule changes the app. A clone would then differ from its golden, and each start would grow the disk again. Its one gain is that an older golden gets the space without a rebuild. The next decision gives that gain with one command.
- **An older golden gets the space with one command and no rebuild.** `tart set testanyware-golden-linux-24.04 --disk-size 50` grows its disk. cloud-init then grows the file system at the first boot of each clone. My measurement is that case: a grown copy that had never booted. The Linux section of `docs/user/golden-images.md` states the command. I did not run it on this Mac's golden, because tart cannot shrink a disk again.
- **A macOS clone does not change.** Its golden already has a 50 GB disk. macOS can grow its APFS container only from recoveryOS (tart FAQ, "Disk resizing"). So a larger disk alone gives a macOS guest no space.
- **A QEMU clone does not change.** It is a qcow2 overlay with the virtual size of its golden. The Windows golden has 64 GB. TestAnyware builds no Linux qcow2 golden, so a supplied image keeps the size that its supplier chose.
- **How a test proves it.** Golden creation reads the size of `/` over SSH before it clones the setup VM to the golden. It fails the build when `/` is smaller than 40 GB. The grown file system had 47.3 GB, and today's file system fits in an 18.9 GB partition. So 40 GB separates the two with margin on each side.
- **The tests of that check.** A unit test covers the pure helper that reads the size and decides. That is the policy of `golden_linux.rs`: pure helpers have unit tests, and a golden built on this Mac proves the live sequence. The live proof is a new Linux golden, then a clone whose `df -h /` reports at least 30G available.
- **The spec and the ADRs do not change.** No spec, ADR or user document states the disk size of a golden. So the decision changes no recorded behaviour. It also fails the ADR test, because a constant and a rebuild reverse it. Its record is a comment at the size constant, as at `BASE_DISK_GB`, and the Linux section of the user guide for goldens.
- **The harness keeps `--no-cache-dir`.** The flag costs nothing on a 50 GB golden. It keeps the OCR band working on a golden built before the change.
- **The follow-on is a `planning` leaf, `clone-disk-size-increments`.** A design session does not cut implementation leaves. The issue #8 release is already published, and the owner chose a separate delivery for this work. So planning also decides when the change ships.
