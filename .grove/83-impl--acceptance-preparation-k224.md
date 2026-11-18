# acceptance-preparation-k224

## Goal

Isolated production test builds and new goldens exist. The checker rejects
the four corruption controls on those builds. Every subject, ceiling and bound
is frozen before any measurement.

## Context

- Spec: section "Validation route", steps 2 to 5.
- Root brief: sections "Constraints" and "Acceptance inventory and bounds".
- The trial plan and checker of leaf k223, with the review repairs of leaf
  k242. The report `docs/experiments/exec-reliability-acceptance-plan.md`
  states the manifest fields and the remaining limits.
- Golden tasks: `vm:create-golden-macos` and `vm:create-golden-linux`.

## Done when

- Build a private Host CLI and app in an isolated jj workspace.
- Create a new macOS golden and a new Linux golden. Each uses
  `TESTANYWARE_AGENT_BIN_OVERRIDE` with the new agent.
- Record that each retained golden is unchanged and was not booted.
- Run the controller self-check against the private builds.
- Run the four corruption controls against the private builds. The checker
  rejects each one.
- Declare the boot, slot and export bounds. The boot bound limits the whole
  `vm start`, because no clone waits for a slot.
- Write a freeze manifest. It names each build hash, golden, image identity,
  export identity and agent version. It lists every ceiling and bound.
- Give the manifest the `agents` and `app` fields that `acceptance_k223.py`
  defines. Its `sha256` field pins the Host CLI, Tart and the app executable.
- Commit the manifest before any trial clone starts.

## Notes

Do not patch the global CLI or app. Do not stop the installed app without the
human's explicit authorisation. This leaf measures no trial clone.
