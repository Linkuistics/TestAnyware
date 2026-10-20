# app-bundle-k10 — brief

## Goal

Make the app a real signed menu bar app on this Mac. Make the Host CLI route through it by default on macOS. An agent under herdr can then drive a guest that a person started in Terminal. This is the first increment that unblocks herdr.

## Done when

- A build script makes a signed bundle. The bundle sets `LSUIElement`, carries the hardened runtime and a Developer ID signature of team `TA43A4RUP3`, and has a bundle id by channel. The development channel has its own id, so it never shares a Local Network row with a release.
- The status item shows the version, whether the app serves its socket, the Local Network state, the Start at Login switch and Quit. The Quit item says that each VM keeps running.
- On a macOS host, the CLI uses the app socket in `~/Library/Application Support/TestAnyware/` by default. No command falls back to a direct connection. Linux and Windows hosts are unchanged.
- The errors `APP_NOT_INSTALLED` and `APP_UNREACHABLE` name the remedy of the setup.
- A live check shows that an agent under herdr drives a guest through the app, after a person allows the alert once.

## Decomposition

1. `app-menu-bar-shell`: the AppKit shell, the bundle and the signing script.
2. `app-default-routing`: the flip of the default, the two errors and the live check.

## Pointers

- Spec: `docs/specs/testanyware-app.md`, sections "The TestAnyware app", "How the app starts" and "Packaging and identity".
- ADR-0003 for the objc2 bindings, ADR-0005 for the thread split, and ADR-0020.
- Glossary terms: TestAnyware app, Local Network grant.
- Koine, the sibling project in `../Koine`, shows the same pattern. Read `scripts/build-app.sh`, `scripts/signing-env.sh` and `Sources/KoineApp/LoginLaunchView.swift`.

## Notes

- Lifecycle commands stay direct in this increment. A person starts the guest in Terminal, which has Local Network access.
- A person must allow the Local Network alert once. A live check records this as a human step and never waits for it unattended.
- Notarisation belongs to `app-release-k21`. A development bundle needs the signature only.
