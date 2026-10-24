# release-2-5-0-k3

## Goal

Publish TestAnyware v2.5.0 with the fix of `guest-stream-end-k2`, as the root
brief describes. Then upgrade this Mac and record that upgrade.

## Context

- The decision log of `plan-k1`: the version, the upgrade evidence, the void
  candidate and the harness failure.
- The leaves of the v2.4.0 release, in the parent of commit `95a3484`, under
  `.grove/24-k21/`. `candidate-rebuild-k53` moved an unpushed tag and rebuilt.
  `release-publish-k37` asked for the approval and published.
- The Taskfile tasks `release:*` and `app:herdr-acceptance`.

## Done when

- Before the rebuild, the remote has no tag `v2.5.0` and no release `v2.5.0`.
  If either exists, stop and ask the owner, because the version is then
  published.
- `task release:doctor` passes.
- The local tag `v2.5.0` names the commit to build, and that commit holds the
  fix. HEAD is that commit, and the tree is clean. Before the build,
  `target/candidates/v2.5.0` and `target/candidates/v2.5.0.checks` are gone.
- `task release:candidate TAG=v2.5.0` and `task release:candidate-verify
  TAG=v2.5.0` pass. The unzipped bundle passes `task release:app-verify`. The
  decision log records the tag, the commit and the sha256 of each artifact.
- `task release:linux-harness TAG=v2.5.0` passes. The log records the hashes
  that it prints.
- The herdr acceptance run passes on the candidate's bundle, with the CLI of the
  candidate's macOS archive.
- The owner approves the publication. The question names the tag, the commit
  and the sha256 of the artifacts. It also proposes to push the commits to
  `main` before the tag, as for v2.4.0.
- `task release:publish TAG=v2.5.0` publishes the recorded candidate and
  rebuilds nothing.
- This Mac runs release 2.5.0 of the formula and of the cask, and
  `testanyware doctor` reports the app.
- The decision log records the real upgrade from 2.4.0 to 2.5.0:
  - the signing identity before and after, and the executable UUID
  - the first-start approval, and any Local Network alert
  - the adoption of the running guests, and the first command.
- The spec ("Packaging and identity", "Open risks") and ADR-0020
  ("Consequences") record release 2.5.0 and the second sample of an upgrade.

## Notes

- The local tag `v2.5.0` names `f0108944`, a void candidate without the fix.
  Never publish it. In the v2.4.0 release, the auto-mode classifier refused the
  deletion of an unpushed tag, and the owner ran the command. Expect the same.
- The herdr acceptance script refuses to run while another app answers. Quit
  the installed app first, and open it again afterwards. The candidate app has
  a new executable UUID, so macOS may show the Local Network alert. Then the
  owner allows it, and the run goes again.
- For the real upgrade, the installed app runs first. Then Homebrew quits it and
  opens the new one. Guests keep running through the upgrade.
- The Linux harness and the acceptance run each start a macOS guest, and the
  acceptance script needs fewer than two running tart guests. Other sessions
  can hold both slots. Ask the owner before you stop a guest that this grove did
  not start.
- `origin/main` is `345faf54`. The commits after it are on no remote branch.
