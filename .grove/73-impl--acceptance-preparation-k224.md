# acceptance-preparation-k224

## Goal

Isolated production test builds and new goldens exist. The checker rejects
the four corruption controls on those builds. Every subject, ceiling and bound
is frozen before any measurement.

## Context

- Spec: section "Validation route", steps 2 to 5.
- Root brief: sections "Constraints" and "Acceptance inventory and bounds".
- The trial plan and checker of leaf k223.
- Golden tasks: `vm:create-golden-macos` and `vm:create-golden-linux`.

## Done when

- Build a private Host CLI and app in an isolated jj workspace.
- Create a new macOS golden and a new Linux golden. Each uses
  `TESTANYWARE_AGENT_BIN_OVERRIDE` with the new agent.
- Record that each retained golden is unchanged and was not booted.
- Run the controller self-check against the private builds.
- Run the four corruption controls against the private builds. The checker
  rejects each one.
- Declare the boot, slot and export bounds.
- Write a freeze manifest. It names each build hash, golden, image identity,
  export identity and agent version. It lists every ceiling and bound.
- Commit the manifest before any trial clone starts.

## Notes

Do not patch the global CLI or app. Do not stop the installed app without the
human's explicit authorisation. This leaf measures no trial clone.
