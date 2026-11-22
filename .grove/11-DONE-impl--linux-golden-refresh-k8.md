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

## Decisions (running log)

- 2026-11-23, pull semantics. The tart source of 2.42.0, the installed
  version, settles the image step. `tart clone` of a tag that the cache
  holds asks no registry, so it clones a stale image. `tart pull` always
  reads the manifest of the tag, downloads a new digest, and points the tag
  at it. Sources: `Sources/tart/Commands/Clone.swift` and
  `Sources/tart/VMStorageOCI.swift` at tag 2.42.0. The citation goes beside
  the pull in `tart.rs`.
- 2026-11-23, fresh package lists. The full update first runs
  `apt-get update` again as a fatal step. The first update runs before the
  desktop install, many minutes earlier.
- 2026-11-23, fatal seam. A pure function maps the exit of the full update
  to the build result, and `system_updates` calls it. The unit tests read
  that function.
- 2026-11-23, Linux dry run. The plan reads `tart list`, as the macOS plan
  does, and fails with `TART_FAILED` when the read fails. So it also fails
  with `BACKEND_UNSUPPORTED` on a host that is not macOS. The JSON gains
  `image` (`name`, `pull`) and `golden_exists`.
- 2026-11-23, controls. Three mutations failed the new unit tests: `clone`
  in place of `pull`, `apt-get upgrade` in place of `full-upgrade`, and an
  outcome that ignores the exit. The restored files passed them.
- 2026-11-23, mirror. The mirror under `website/docs/` had drifted before
  this leaf. It lacks the slot section and the disk section of the source
  page. This leaf changes only the Linux refresh text and the dry-run text
  in both pages, and it leaves the drift alone.
- 2026-11-23, suites. `task cli:test` passed: 63 suites, 1741 tests, 0
  failed, 13 live tests ignored. The changed files did not move during the
  run. Clippy reports nothing new in the changed code.
- 2026-11-23, plan seam. On this Mac, the Linux dry run named
  `ghcr.io/cirruslabs/ubuntu:24.04`, the pull, and the existing golden that
  the run replaces.
- 2026-11-23, refresh. `task vm:refresh-golden-linux` rebuilt
  `testanyware-golden-linux-24.04` in 13 minutes. The pull moved the cached
  tag from digest `51b493a0…` to `5a6dcb63…`. The full update passed, `/`
  was 47.3 GB, and the earlier golden went only at the final clone. The new
  golden has a 50 GB disk.
- 2026-11-23, live criteria. `task vm:linux-golden-disk-live` passed: a
  clone had 38.6 GB free on `/`. A clone reported Ubuntu 24.04.5 LTS and
  kernel 7.0.0-43-generic. `apt list --upgradable` listed no package with
  the lists of the build. After `apt-get update` (exit 0) in a clone, it
  still listed no package. `vm start` returned `ok: true`, which needs a
  Healthy agent, and `agent health` reported the agent reachable and
  accessible.
- 2026-11-23, review. This leaf cut no `review-impl` leaf. The pull carries
  a citation of the tart source, mutations failed the new tests, and a live
  build exercised each changed step. A wrong change is easy to reverse: a
  rebuild replaces the golden.
