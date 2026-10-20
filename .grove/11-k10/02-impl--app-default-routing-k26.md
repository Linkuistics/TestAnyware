# app-default-routing-k26

## Goal

Make the Host CLI route agent and VNC connections through the app by default on a macOS host. Prove it from herdr with a guest that a person started.

## Context

- `docs/specs/testanyware-app.md`, sections "How the app starts" and "New error codes", and the requirements "Every guest connection on a macOS host goes through the app" and "No client starts the app".
- The harness of `cli-app-agent-routing-k24` and the live checks in `Taskfile.yml`.
- The tests that run the CLI on macOS with explicit loopback endpoints. They assumed a direct connection.

## Done when

- On a macOS host without `TESTANYWARE_APP_SOCKET`, the CLI uses the default socket path of the app.
- A missing app bundle gives `APP_NOT_INSTALLED`, and the remedy is the setup. Nothing answering on the socket gives `APP_UNREACHABLE` with a cause. No command starts the app.
- No command falls back to a direct connection on macOS. `vm create-golden` and the lifecycle commands stay direct for now.
- Linux and Windows hosts behave as before. A test shows it.
- The existing tests that run on macOS use the harness, so they pass.
- A Taskfile live check drives a running guest from a herdr session through the development bundle. It runs `agent snapshot`, a screen capture, an input command and a file transfer.
- The leaf records the live result. A person starts the guest in Terminal and allows the alert once. The agent does the rest.

## Notes

- This leaf is the first time the default of the product changes. Run the whole suite before and after.
- The remedy text for a missing app is the five setup steps in the spec. The cask does not exist yet, so say that the release provides it.
