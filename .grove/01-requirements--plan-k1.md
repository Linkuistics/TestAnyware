# plan-k1

## Goal

Establish what this grove builds, in the owner's words. Then make the release
v2.5.0 in this session, as the root brief describes.

## Context

- The previous grove's root brief, items 5 and 6 of its decomposition. Read it
  with `git show 95a3484^:.grove/_BRIEF.md`.
- The v2.4.0 release leaves in the same commit, under `.grove/24-k21/`. The
  leaf `release-publish-k37` shows the approval question and the publication.

## Done when

- The decision log records the owner's words and each decision.
- The "Done when" of the root brief holds.

## Notes

- The release changes no code, so it needs no new test seam. It runs the
  existing Taskfile live checks of the spec.

## Decisions (running log)

- **The requirement.** On 2026-10-24 the owner said:
  <!-- tc: off because this quotes the owner verbatim -->
  "I wanted you to do a release, but you ended the grove after deleting the grove files".
  <!-- tc: on -->
  The previous grove's leaf `default-config-file-increments-k59` recorded that
  its work ships with the next release, together with `linux-golden-disk-size-k57`.
  It cut no release leaf. Then `finish-k61` removed the tree. So this grove
  publishes the next release.
- **No tree.** The path to done is clear. The pipeline that published v2.4.0 is
  in the Taskfile, and `task release:doctor` passed every check on 2026-10-24.
  The tap clone at `~/Development/homebrew-taps` is clean and on `main`. The
  owner is present for the approval. So this leaf makes the release, and it cuts
  no leaf.
- **No grilling.** Three questions were open: the version, the evidence of the
  upgrade, and a free macOS guest slot. Their answers do not depend on each
  other, so the threshold of three interdependent questions is not met. The owner
  answered each one.
- **What ships.** Twelve local commits follow `v2.4.0` (`345faf54`). Outside
  `.grove/` and the documents, they change the app's code and tests, the
  configuration crate, the golden creation code, `Taskfile.yml` and three live
  check scripts. No command, flag, environment variable, exit code or `--json`
  field changes. One value changes: on a macOS host, the config check of
  `doctor` reports `loaded` after the first start of the app, where it reported
  `absent` before.
- **The version is v2.5.0.** The owner chose it. The release adds two features
  and keeps the CLI surface, and the releases v2.1.0 to v2.4.0 were all minor.
  Before the tag, neither this checkout nor the remote had a tag `v2.5*`. The
  control `v2.4*` listed `v2.4.0` on the remote.
- **The upgrade evidence is the real upgrade of this Mac, after publication.**
  The owner chose it over a run of `release:upgrade-check` with a throwaway
  release before publication. Eleven files name the bundle id. Of these, only
  `Taskfile.yml`, `scripts/app-bundle-live.sh` and
  `scripts/upgrade-identity-live.sh` changed since `v2.4.0`, and the build reads
  none of them. The build inputs that set the identity did not change:
  `build-app.sh`, `release-build.sh`, `notarize-app.sh`, `verify-app.sh`,
  `release_candidate.py`, the templates and the two `bundle.rs` files. The build
  refuses another bundle id or team. The owner accepts that a fault of the
  upgrade shows only after publication.
- **The guest slot.** Both macOS slots held a guest: `k26-live`, which tart last
  accessed two days earlier, and `koine-verify-54176`, which was active. The
  Linux harness and the herdr acceptance run each start a macOS guest. The owner
  consented to stop `k26-live`.
