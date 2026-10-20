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

## Handed over by app-vm-stop-delete-k28

- With the app routing on (the default on macOS), `vm stop <id>` asks the app. The app answers after the VM and its sidecars are gone, so a `vm list` that follows shows no VM. A stop by signal is another case: the app cleans up a moment after the CLI has gone, and `assert_vm_gone` in `scripts/live-start-checks.sh` polls for it.
- A stop of an id that is starting waits for the start to end. A check that kills a start should kill the CLI of the start, and not call `vm stop` while the start runs, or the stop will wait.
- `vm stop --dry-run` and `vm delete --dry-run` never contact the app, so a dry run proves nothing about the routing.

## Decisions (running log)

- The check is a new script, `scripts/herdr-acceptance-live.sh <app-bundle>`, and a task, `task app:herdr-acceptance [APP=<bundle>]`. The task builds the development bundle when `APP` is empty. `release-publish-k37` passes the bundle of its candidate. The rejected option was to extend `app-routing-live.sh`. That script takes a guest that a person started, and this one owns the guest from start to stop. The two checks keep separate scripts and share only the structure.
- The script reads the bundle id from the bundle and quits only an app that it started. When an app already answers and no process runs from the given bundle, the script fails. Such an answer would not prove the bundle under test. `status` carries no bundle path, so the script finds the process by the path `<bundle>/Contents/MacOS/`.
- "Each step goes through the app" is shown by a control for all six commands. With `TESTANYWARE_APP_SOCKET` naming a socket with no app, each of `vm start`, `agent snapshot`, `screen capture`, `input move`, `file upload` and `vm stop` fails with `APP_UNREACHABLE`. The control for `vm start` also checks that the refused start created no guest. A named dead socket gives `APP_UNREACHABLE`. The default socket with no app gives `APP_NOT_INSTALLED` on this Mac, so the control cannot use the default.
- The file command is an upload and a download that are compared byte for byte, and then an `exec` removes the remote file. The control covers the upload.
- The script has no pre-check of the grant. A missing grant shows as a Local Network denial of `vm start` or of a later command, and the script names the person's step and fails. The alert can only appear at the app's first connection to a guest.
- The cleanup runs on every exit. It asks the app to stop the guest. When the guest survives, it stops and deletes the guest with tart and removes the sidecars. It quits the app only when the script started it. The script never calls `vm stop` while its start runs, because the start is a foreground command that has ended by then (a stop waits for a start of the same id).
- The script refuses to start when both macOS guest slots are taken. Another session's guest, such as `k26-live`, holds one slot, and the check needs one.
- `vm start` prints progress on stderr and the result on stdout. The `run` helper keeps the two apart, so `jq` reads the result.

## Result of the run on this Mac

Run from the herdr session of this leaf (`HERDR_PANE_ID` `w2E:p1`), on the development bundle `TestAnyware Dev.app`, built and signed by `scripts/build-app.sh dev` with the Developer ID of team TA43A4RUP3. The Local Network grant of the bundle already existed, so no alert appeared.

- `task app:herdr-acceptance` printed `PASS`. `vm start --platform macos` reached a healthy agent in 37 s. `agent snapshot` saw 3 windows. `screen capture` wrote a 1920x1080 PNG. `input move` and the 200000-byte file round trip succeeded. All six controls failed with `APP_UNREACHABLE`. After `vm stop`, tart, the sidecars and `vm list` showed no guest.
- This shell can dial the guest at `192.168.64.9:8648`, so the success of the steps alone does not prove the routing. The six controls do. A shell that Local Network privacy denies would have shown the denial in the steps.
- The failure path was run once with a CLI wrapper that failed `screen capture` with a Local Network message. The script failed with the grant step in its message, exit 1. The guest was gone from tart, and the app had quit. Only the log files of the guest remained, as after a passing run.
- `task scripts:check` (shellcheck and shfmt) passes with the new script in its list.
