# llm-instructions-and-docs-k35

## Goal

Describe the app in the `llm-instructions` command, and correct the documents that the design changes.

## Context

- `docs/specs/testanyware-app.md`, sections "What the commands report about the app" and "Documents that the implementation corrects".
- The discoverability code of the CLI and its tests.

## Done when

- The `llm-instructions` have a section on the TestAnyware app. It covers the one-time setup by a person and the rule that a client never starts the app. It covers each new error with its remedy, and golden creation from Terminal.
- `docs/reference/connection-spec.md` no longer calls `<id>.json` a contract for direct readers, and it says that QEMU listens on `localhost`.
- `docs/user/multi-vm-networking.md` puts the QEMU agent on `localhost`.
- The doc comment of the target resolution and `docs/reference/env-vars.md` put `TESTANYWARE_VM_ID` before `--agent` and `--vnc`, as the code does.
- The error codes list the new codes and `AGENT_INVALID_JSON`.
- `docs/architecture/cli-design-contract.md` has the amended Local Network remedy for a macOS host, and the catalogue gains the new codes.
- The ADR set, the spec and `CONTEXT.md` agree with the shipped behaviour. The leaf lists each change it makes to them.
- The dated research documents stay unchanged.

## Notes

- The documents are prose. Follow the writing rules of this repository for each file that this leaf edits.
