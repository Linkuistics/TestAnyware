# upgrade-identity-check-k36

## Goal

Prove on this Mac that a cask upgrade keeps the identity of the app, with the release build.

## Context

- The requirement "An upgrade keeps the app's identity", and the open risks "The release build" and "An alert after an upgrade" in the spec.
- `docs/research/testanyware-app-local-network-grant.md`, for the measurement method.
- `app-release-bundle-k22`.

## Done when

- A Taskfile live check installs release N and then release N+1 of the cask from a local tap with local artifacts. Nothing is published.
- After the upgrade, no "downloaded from the Internet" dialog appears, and System Settings shows one TestAnyware row.
- The first command to a tart guest succeeds within the retry, or it fails with `local_network_denied` and a remedy that names TestAnyware. After a person allows the app again, the command succeeds.
- Each running VM keeps running through the upgrade, and the new app adopts it.
- The leaf records the measurements in the decision log. If the result differs from the spec, it names the change that the spec and ADR-0020 need.

## Notes

- A person allows the alert, and answers the first-start dialog once. The check names these steps.
- Use two builds with different executable UUIDs. A rebuild gives them.
