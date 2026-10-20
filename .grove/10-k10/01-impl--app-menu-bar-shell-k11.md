# app-menu-bar-shell-k11

## Goal

Put the app server of `app-streams-k8` inside a menu bar app, and build it as a signed development bundle.

## Context

- `docs/specs/testanyware-app.md`, sections "The TestAnyware app" and "Packaging and identity".
- ADR-0003, ADR-0005 and ADR-0020.
- Koine's `scripts/build-app.sh`, `scripts/signing-env.sh` and `Sources/KoineApp/LoginLaunchView.swift`.
- `docs/research/testanyware-app-local-network-grant.md`, for the probe app that the owner already measured.

## Done when

- The AppKit run loop runs on the main thread and shows one status item. The asynchronous runtime runs the server on its own threads.
- The menu shows the version, whether the app serves its socket, and the Local Network state. The Local Network state has an item that opens the Local Network pane. The menu also has the Start at Login switch. The switch uses `SMAppService.mainApp` and reports the status `requiresApproval`.
- The menu has a Quit item. Its text says that each VM keeps running.
- A pure model builds the menu content from the state of the app. Unit tests cover it.
- A script builds the bundle for a channel. It writes the Info.plist, sets `LSUIElement`, and signs with the hardened runtime and team `TA43A4RUP3`. It checks the bundle id and the team, and it refuses any other.
- The development channel uses the id `dev.antony.TestAnyware.dev`. The release channel uses `dev.antony.TestAnyware`.
- A Taskfile task builds the development bundle. A live check opens it, and `GET /v1/status` answers on the socket with the bundle's build.

## Notes

- Test mode from `app-streams-k8` stays available in the same executable.
- Keep the shell thin. The server code must not depend on AppKit.
- The leaf may decompose if the bundle script and the shell do not fit one session. Cut the script first.
