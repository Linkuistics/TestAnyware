# herdr-acceptance-run-k29

## Goal

Write the herdr acceptance run as a Taskfile live check, and run it on the development bundle. The same check runs on the release bundle in `release-publish-k37`.

## Context

- The done-when of the root brief, the last line about the herdr acceptance run.
- The live checks in `Taskfile.yml` and the scripts that they call.
- `app-default-routing-k26`, which has the first live drive.

## Done when

- A script takes the path of an app bundle. From a herdr session it starts a macOS guest with `vm start --platform macos`. It runs one agent command, one screen command, one input command and one file command. It stops the guest.
- Each step goes through the app. The script shows it, for example by the denial that the same CLI gets when the app is not in use.
- A Taskfile task runs the script and prints a clear pass or fail.
- The script cleans up its guest on a failure.
- The leaf runs the check on this Mac, from the herdr session of the leaf, on the development bundle. The result is in the decision log.

## Notes

- The run needs the grant. A person allows the alert once before the first run. The script names that step and fails fast when the grant is missing.
- `app-default-routing-k26` wrote `scripts/app-routing-live.sh` (`task app:routing-live VM=<id>`). It takes a guest that a person started. It starts the development bundle only when no app answers, and quits only that one. It runs the four commands with `TESTANYWARE_APP_SOCKET` unset. Its control sets that variable to a socket with no app, and the command fails with `APP_UNREACHABLE`. This leaf can take the structure of that script, and add the guest start and stop.
- A shell under herdr can dial a guest on loopback, so only a guest on a Local Network address shows that the app carried the traffic. The script prints whether the shell can dial the guest, and the control covers the case where it can.
