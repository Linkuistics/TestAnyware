# 20. The TestAnyware app keeps one signing identity and ships as a cask

Date: 2026-10-20

## Status

Accepted (grove `gh-issue-8`, leaf `vm-broker-design-k3`). Reworked in leaf
`vm-broker-design-k6`, which removed the pinned executable UUID. The evidence
is `docs/research/testanyware-app-local-network-grant.md`. The area's spec is
`docs/specs/testanyware-app.md`.

## Context

The TestAnyware app holds the Local Network grant (ADR-0019). An agent can do
nothing on a macOS guest while macOS waits for a person to allow that grant. So
the question is what an upgrade does to the grant, and to the approval of the
first start.

TN3179 says that Local Network privacy tracks a program by its code signature.
It also uses the main executable's UUID, and TN3179 says that this UUID must be
unique. TN3178 says: "Each distinct Mach-O image must have its own unique build
UUID." It also says that no Apple command sets a build UUID after the link.

The prior-art survey found tools that lost the grant on each update. In each
one, a helper moved to a new versioned path, such as a Homebrew formula's Cellar
path. In those reports, the path and the build changed together.

On 2026-10-20 a probe app measured each input apart, on macOS 27.0.1. The probe
had a Developer ID signature, but no notarisation:

- A new main executable UUID gave a refusal of about 2 s, three times out of
  three. Once, it raised the alert again.
- An unchanged UUID never did. This held with a new signature, at a new path,
  and with new code whose UUID was set to the old value.
- System Settings showed one row for the app throughout.

TestAnyware ships today as the formula `testanyware`, whose path changes with
each version. Koine, in the same tap, ships as a cask with a notarised app in
`/Applications`.

## Decision

**The app keeps one bundle id and one Developer ID signature in every release.
It ships notarised, as the cask `testanyware-app`. Each release keeps the UUID
that its link gives its executable. The formula `testanyware` stays the Host
CLI.**

- The bundle id is `dev.antony.TestAnyware`, with a Developer ID Application
  signature of team `TA43A4RUP3` and the hardened runtime. The bundle is
  notarised and stapled. The release checks the bundle id and the team, and it
  refuses a different one.
- Nothing edits the UUID of an executable. The Host CLI and the app are two
  executables, so their links give them two UUIDs.
- A development build has its own bundle id, so it never shares a Local Network
  row with a release.
- The cask installs `TestAnyware.app` into `/Applications`, and it depends on
  the formula. It keeps the quarantine attribute. Its `uninstall quit:` names the
  bundle id, so a Homebrew upgrade quits the app and opens it again.

An upgrade does three things to the app's permissions:

- Homebrew 7.0.9 carries the approval of the first start to the upgraded copy
  when the signing identity is unchanged. So an upgrade does not ask the
  "downloaded from the Internet" question again.
- The unchanged signature keeps one Local Network row for the app.
- The new UUID makes macOS evaluate the grant again. The first guest connection
  can fail for about 2 s, and the 5 s retry of a guest stream covers that
  (ADR-0019). macOS can also raise the alert again. Then a person allows
  TestAnyware once more. The owner accepts this, because other apps on this Mac
  ask in the same way after an update.

## Considered options

- **One pinned UUID in every release.** The build would set the old UUID in each
  new executable before it signs the bundle. In the measurements it kept the
  grant with no refusal and no alert. Rejected: it gives distinct images one
  UUID, which TN3178 forbids. It also needs a tool of our own to edit the load
  command, and crash reports could not tell two releases apart.
- **A launcher that never changes, which spawns a helper that changes.** It kept
  the grant in the measurements. TN3179 documents that macOS counts a helper's
  connections as the app's. Rejected: it costs a second process, and the owner
  accepts a new grant after an update. The launcher would receive the quit event
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

- After an upgrade, a person may have to allow TestAnyware again. Until then, a
  guest connection fails with `local_network_denied`, and the remedy names the
  app (ADR-0019). No command waits for the person.
- A change of the bundle id or of the signing team loses the grant and the
  approval of the first start, for each user. The release check exists to stop
  that by mistake.
- An existing user takes one new step: the cask install and the one-time setup.
- The probe was not notarised or quarantined. The live check
  `release:upgrade-check` ran the release build once, on 2026-10-22. An upgrade
  with a new executable UUID kept the signing identity and the approval of the
  first start. The grant needed no second approval after it. One run gives no
  rate for the alert (spec, "Open risks").
