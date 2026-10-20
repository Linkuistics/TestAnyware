# adoption-live-check-k32

## Goal

Prove on this Mac that the VMs survive a quit of the app and that the app adopts them.

## Context

- The requirement "VMs outlive the app" and the live check list in the section "Test seams" of the spec.
- The live checks in `Taskfile.yml`.

## Done when

- A Taskfile live check starts a guest through the app. It quits the app, and it shows that the guest still runs. It opens the app again.
- After that, `vm list` shows the guest, and an agent command reaches it.
- The check also covers a guest that an older Host CLI started, when such a CLI is at hand. If none is, the leaf says so and covers the legacy path with a hand-made older meta.
- The check stops the guest and cleans up.
- The result is in the decision log.

## Notes

- Quit the app with a script, for example through `osascript`, so the check needs no human.

## Decisions (running log)

- **One script, one Taskfile task.** `scripts/app-adoption-live.sh <bundle>` and `task app:adoption-live` (`APP=<bundle>` defaults to the development bundle). It follows `herdr-acceptance-live.sh`: it refuses to run when another app answers, it quits only an app that it started, it never waits for the Local Network alert, and it stops its guests on every exit (the app first, then tart). The script is in the `scripts:check` list, and shellcheck and shfmt pass.
- **The check quits and opens the app twice, with one guest.** (1) It starts a macOS guest through the app, and the meta must record the identity of the guest and its keeper. (2) It quits the app through the Apple event of the Dock's Quit. The guest must run on: tart says `running`, and the tart process and the keeper keep their pids and start times. With the app gone, an agent command fails with `APP_UNREACHABLE`, so the CLI has no direct path. (3) It opens the app. `GET /v1/status` must show the guest in `adoption.adopted` with `legacy` false and the agent and VNC probes answering, and not in `not_adopted`. `vm list` shows the guest, `agent snapshot` and `screen capture` reach it, and no process restarted. (4) It quits the app, removes `identity` and `keeper_identity` from the meta (the hand-made older meta), and opens the app. The guest is adopted with `legacy` true, and both streams still work. `vm stop` then removes the guest, both sidecars and the `vm list` row, so the app stopped it by name.
- **The legacy case with a real older CLI is tried, and it is skipped here.** The `testanyware` 2.3.0 in `/opt/homebrew/bin` is an older Host CLI at hand, and the script uses it (or `OLD_TESTANYWARE_BIN`) when its path differs from the CLI under test. Its meta must record no identity, or the phase says so and skips. On this Mac, from a herdr shell, its direct start failed in 8 s with `VM_BOOT_TIMEOUT` and `cause: local_network_denied`, which is the denial of ADR-0019 for a CLI that has no app. The phase reports a SKIP with that output, and the hand-made meta of phase 4 stands for the legacy path. A person who runs `task app:adoption-live` from Terminal, which has Local Network access, runs the real older CLI case as well. That case is not yet seen to pass.
- **The app is the process with no arguments.** The first run failed at the quit: the pattern `<bundle>/Contents/MacOS/` also matched the VNC keeper, which runs the app's executable as `_vnc-keeper` and must outlive the app. That was the script's fault. The pattern is now `^<bundle>/Contents/MacOS/testanyware-app$`, for the quit check and for the cleanup. `app-bundle-live.sh` and `herdr-acceptance-live.sh` still use the directory pattern. Both only quit an app after a guest is gone or never started, so they are left as they are.
- **Result, on this Mac, the development bundle 0.0.1, `task app:adoption-live`: PASS.** The guest `adoption-live-76485` reached a healthy agent in 15 s. After the quit, its tart process (pid 76556) and keeper (pid 76575) ran on with the same start times. The app adopted it with identity (agent and VNC answering), then as a legacy guest (the same answers), and `vm stop` removed it. Afterwards `tart list` showed no `adoption-live` guest, and no app ran. The first run failed on the pattern above, and its cleanup removed its guest.
- **The checks were seen to discriminate.** `adopts_as` read `legacy` false and then true in one run, so it tells the two cases apart. The quit check failed in the first run for a stranger process. The process comparison was run on an altered start time and a dead pid in a scratch shell, and it reported a mismatch and "not alive" for them.
- **No in-session reviewer.** The claims are about real processes, and the run shows them.
