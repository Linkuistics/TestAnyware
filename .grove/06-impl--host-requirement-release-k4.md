# host-requirement-release-k4

## Goal

Publish a TestAnyware release that carries the work of this grove. Install it
on this Mac, then move and push `main`.

## Context

- The Taskfile of the project holds the release tasks. The leaf
  `lifecycle-release-k226` of the previous grove published v2.9.0 the same
  way. Commit `8fbb7568` holds its record.
- The close rule of the owner names the facts that this leaf proves.

## Done when

- `gh release view` shows the new release.
- `brew list --versions` shows the formula at that version.
- With the TestAnyware app running, `testanyware doctor` shows the Host CLI
  and the app at that build.
- `origin/main` holds the final commit.
- The release notes state the new host requirement and the removal of the
  keeper.

## Notes

- The owner chooses the version number. Without the keeper, a host on an
  earlier macOS 27 release loses its guests after an idle minute. So the
  release may count as a breaking change.
- Each design leaf inserts its implementation leaves ahead of this leaf.
