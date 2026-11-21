# host-requirement-docs-k6

## Goal

State the host requirement in the docs, as ADR-0018 decides. Remove the VNC
keeper and its manual workaround from the user, agent and architecture docs.

## Context

- ADR-0018 is the decision:
  `docs/adr/0018-a-macos-host-runs-the-latest-macos-27-and-tart.md`. The
  glossary terms "Host requirement" and "VNC keeper" (historical) give the
  wording.
- The requirement, as the owner set it: a TestAnyware host on macOS runs the
  latest release of macOS 27 and the latest release of tart. A guest is outside
  the requirement.
- These pages state the requirement:
  - The README "Requirements" table. The macOS row names the latest release of
    macOS 27 on Apple Silicon. The tart row names the latest release of tart. A
    sentence under the table says that a guest is outside the requirement.
  - `docs/user/quick-start.md` "Prerequisites", with the same two facts.
  - `website/index.md`, which says "macOS 14+ host" today.
  - `LLM_INSTRUCTIONS.md`. Its "Common mistakes" bullet on the 60-second fault
    becomes the requirement and the symptom on an older host.
- These pages lose the keeper:
  - `docs/user/troubleshooting.md`. The entry "A tart guest dies when `screen`
    or `input` runs after 60 idle seconds" names the symptom on a host below
    the requirement. It names the remedy, an update of macOS and tart, and
    `task vm:tart-vnc-idle-repro` as the check. The keeper text and the manual
    `nc` holder go. The pointer to the research note stays.
  - The README "Troubleshooting" paragraph, in brief.
  - `docs/architecture/vm-lifecycle.md`. Its "VNC keeper" section goes, and
    its meta and stop text loses the keeper.
  - `docs/architecture/cli-design-contract.md`. The bullet on a cancelled start
    loses "its keeper".
- `website/docs/` mirrors `docs/` with a title header, and no script syncs it.
  Change each mirrored page the same way: the troubleshooting guide, the
  lifecycle page and the quick start.
- History stays as written. This covers the research notes, the context of
  ADR-0018, the historical glossary entry and released changelog entries. It
  also covers finished task files and `experiments/exec-reliability/evidence/`.

## Done when

- The README, the quick start, `website/index.md` and the LLM instructions
  state the host requirement in the wording above.
- No page names "macOS 14+" as the host.
- A sweep finds `keeper` and the manual holder only in the history listed
  above. The holder patterns are `tail -f /dev/null` and `nc 127.0.0.1`. The
  sweep enumerates each hit over `docs/`, `website/`, `README.md` and
  `LLM_INSTRUCTIONS.md`, then classifies it. It has a positive control and a
  cross-tree control.
- Each changed page passes the controlled-English checker, or it carries a
  marked departure.

## Notes

- `keeper-removal-code-k5` changes the code and the specs first. This leaf
  describes the result.
- The release leaf writes the release notes. They state the host requirement
  and the removal of the keeper.
- `testanyware llm-instructions` prints `LLM_INSTRUCTIONS.md`, so its change
  reaches agents with the release.

## Decisions (running log)

- 2026-11-22, conversion. Seven pages failed the controlled-English checker
  before the edit. A preparatory change converted them without a change of
  meaning. It also converted the amendment of 2026-10-20 in the CLI design
  contract and removed its pending marker.
- 2026-11-22, scope. The sweep found two pages that the context did not list.
  `docs/architecture/overview.md` and its mirror drew the host as "macOS 14+".
  The README component table said "macOS 14+ host only". Both now name no old
  release.
- 2026-11-22, mirror. The website lifecycle page lacked the "Start records"
  row that an earlier leaf added to `docs/`. This change adds the row, so each
  mirror equals its source apart from the title header.
- 2026-11-22, troubleshooting. The entry names the symptom on a host below the
  requirement, both faults of macOS 27.0.1, and three update steps. The
  `HOLD=1` sentence went with the manual holder, because it showed only the
  effect of a held connection.
- 2026-11-22, lifecycle. A short "VNC connections" section replaces the keeper
  section. It says that each command opens its own connection and that no
  process holds one between commands.
- 2026-11-22, sweep. A script enumerates each hit of `keeper`, the two holder
  commands, `macOS 1[0-9]` and the names Sonoma, Sequoia and Ventura over the
  scope, and classifies each hit. The research notes and ADR-0018 hold the
  keeper hits. The other hits name refused guest tags, the QEMU flag
  `-vnc 127.0.0.1` and a macOS 15.5 feature. On a scratch export of the parent
  revision, the same script reports each class that this leaf removed. The
  positive control finds the keeper in ADR-0018, and a mutated pattern makes it
  read zero.
- 2026-11-22, checks. Each changed page has no hard finding of the checker.
  The two contract tests of `llm-instructions` passed.
- 2026-11-22, review. This session cut no `review-impl` leaf. The change
  rewrites docs to match code and decisions that earlier leaves settled, and
  the sweep with its controls checks the claim.
