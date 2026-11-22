# linux-golden-refresh-k8

## Goal

Make `vm create-golden --platform linux` install the latest release of
Ubuntu 24.04, as ADR-0027 decides. Then rebuild the Linux golden of this Mac
with the agent of this checkout.

## Context

- The decision is
  `docs/adr/0027-each-golden-holds-the-latest-release-of-its-major.md`.
- `golden_linux.rs` clones `ghcr.io/cirruslabs/ubuntu:<version>`. tart uses
  the image in its cache when the cache holds one. On this Mac the tag
  `24.04` points to a digest that tart pulled on 2026-10-21.
- `system_updates` in `golden_linux.rs` runs `apt-get upgrade` through
  `exec_tolerant`. `apt-get upgrade` holds back each update that needs a new
  package, such as a new kernel. A failed upgrade still builds a golden.
- Boot 1 in `golden_linux.rs` deletes a golden of the same name before it
  clones.
- On this Mac, the disk of `testanyware-golden-linux-24.04` last changed on
  2026-06-08. It still has the 20 GB disk of the Cirrus image, so it
  predates the disk growth to 50 GB.
- `macos-golden-refresh-k7` adds the refresh recipe to the Taskfile, and it
  moves the replacement of the macOS golden to the final clone.
- `macos-golden-refresh-k7` named its recipe `vm:refresh-golden-macos`. It
  takes no platform, so the Linux form goes beside it.
- `docs/user/golden-images.md` has a section "Keeping goldens current". Its
  Linux item says only that the build installs the current Ubuntu updates.
  Replace that item with the pull and the full update. The mirror under
  `website/docs/` comes from the same text.
- The shared `fake-tart` lists `testanyware-base-macos-27.2.1-26B131`, and
  `TestApp::add_golden_tools` in the CLI test harness puts a fake `curl` on
  `PATH`.

## Decisions for this leaf

- **Image.** Run `tart pull ghcr.io/cirruslabs/ubuntu:<version>` before
  the clone, so the build starts from the current image of the tag.
- **Updates.** Install every pending update with `apt-get full-upgrade`,
  keeping the present Dpkg options. Run it through `exec_checked`, so a
  failed update fails the build.
- **Replacement.** Delete the earlier golden just before the final clone,
  not in boot 1. A failed build then keeps the earlier golden.
- **Dry run.** The Linux plan names the source image, and it says that the
  build pulls that image first. It says when the run replaces a golden.
  Update the schema to match.

## Done when

- Unit tests cover the pull command, the `full-upgrade` command and its
  fatal handling. Name the test seam of each in the code.
- `cli-contract.rs` shows that the Linux dry run names the image and the
  pull.
- The refresh recipe of `macos-golden-refresh-k7` also refreshes
  `testanyware-golden-linux-24.04`. If that recipe takes no platform, add
  the Linux form beside it.
- `task vm:linux-golden-disk-live GOLDEN=testanyware-golden-linux-24.04`
  passes on the new golden.
- In a clone of the new golden, `apt list --upgradable` lists no package,
  and `/etc/os-release` names a 24.04 point release. `vm start` of that
  clone reaches a Healthy agent.
- `docs/user/golden-images.md` and its mirror under `website/docs/` describe
  the pull and the full update. The section on the 20 GB golden goes, since
  no such golden remains.
- The existing suites pass, and no suite gains a compatibility case. Each
  changed prose page passes the controlled-English checker, or it carries a
  marked departure.

## Notes

- The seams in this leaf are this design's proposal. The root brief records
  the golden seams that the owner agreed, and those name only the macOS
  golden.
- A Linux build takes about 20 to 40 minutes, and the pull downloads the
  image again when the tag moved.
- Keep the build logs in scratch, not in the repository.
- The release leaf follows this leaf.
