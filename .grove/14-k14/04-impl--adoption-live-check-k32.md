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
