# 20. The TestAnyware app keeps one executable UUID and ships as a cask

Date: 2026-10-20

## Status

Accepted (grove `gh-issue-8`, leaf `vm-broker-design-k3`). The evidence is
`docs/research/testanyware-app-local-network-grant.md`. The area's spec is
`docs/specs/testanyware-app.md`.

## Context

The TestAnyware app holds the Local Network grant (ADR-0019). An agent must never
wait for a person after the one-time setup, so an upgrade must keep that grant.

TN3179 says that Local Network privacy tracks a program by its code signature,
and that it uses the main executable's UUID. The prior-art survey found tools
that lost the grant on each update. In each one, a helper moved to a new
versioned path, such as a Homebrew formula's Cellar path. In those reports, the
path and the build changed together.

On 2026-10-20 a probe app measured each input apart, on macOS 27.0.1:

- A new main executable UUID gave a refusal of about 2 s, three times out of
  three. Once, it raised the alert again.
- An unchanged UUID never did. This held with a new signature, at a new path,
  and with new code whose UUID was set to the old value.

TestAnyware ships today as the formula `testanyware`, whose path changes with
each version. Koine, in the same tap, ships as a cask with a notarised app in
`/Applications`.

## Decision

**The app's main executable carries one pinned UUID in every release, and the
app ships as the cask `testanyware-app`. The formula `testanyware` stays the
Host CLI.**

- The build sets the pinned UUID in the app's executable before it signs the
  bundle. The release checks the UUID and refuses a different one.
- The Host CLI's executable keeps a UUID of its own, which changes with each
  build. The two executables never share a UUID. TN3179 warns that a shared
  UUID confuses Local Network privacy.
- A development build has its own bundle id and its own pinned UUID, so it never
  shares a Local Network row with a release.
- The bundle id is `dev.antony.TestAnyware`, with a Developer ID Application
  signature of team `TA43A4RUP3`. The bundle is notarised and stapled.
- The cask installs `TestAnyware.app` into `/Applications`, and it depends on
  the formula. It keeps the quarantine attribute. Its `uninstall quit:` names the
  bundle id, so a Homebrew upgrade quits the app and opens it again.

Homebrew 7.0.9 carries the approval of the first start to the upgraded copy when
the signing identity is unchanged. So an upgrade asks neither the Local Network
question nor the "downloaded from the Internet" question again.

## Considered options

- **A natural UUID for each build.** Rejected. Each upgrade gave a refusal of
  about 2 s, and one in three raised the alert. An agent would then wait for a
  person after an upgrade.
- **A launcher that never changes, which spawns a helper that changes.** It
  worked in the measurements, and it needs no edit of a Mach-O header. Rejected,
  because it costs a second process. The launcher would receive the quit event
  and the alert's attribution, while the helper would own the menu bar item.
- **The app inside the formula's keg.** Rejected. Its path changes with each
  version, and nobody tested a new path together with a new UUID.
- **One cask that holds the app and the Host CLI.** Rejected. Linux needs the
  formula, and existing users have the formula installed. A cask and a formula
  with one name in one tap make every install ambiguous.
- **A cask that strips the quarantine attribute**, as Modaliser's does.
  Rejected. Koine's cask keeps it, because a strip hides a failed notarisation.
  Homebrew's carry-over of the approval makes a strip unnecessary.

## Consequences

- A crash report of the app cannot tell two releases apart by their UUID. The
  release profile strips symbols anyway.
- If the pinned UUID ever changes, each user answers the alert once more. The
  release check exists to stop that by mistake.
- An existing user takes one new step: the cask install and the one-time setup.
- The live check of an upgrade in the Taskfile proves the release build. The
  probe that this decision rests on was not notarised or quarantined.
