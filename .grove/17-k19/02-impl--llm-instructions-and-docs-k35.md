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
- `cli-app-agent-routing-k24` added `APP_UNREACHABLE` and `APP_INCOMPATIBLE` to `ERROR_CODES` in `surface.rs`, and it updated the table "New error codes" of the spec. The causes of `APP_UNREACHABLE` are `not_running`, `unsafe_socket_directory`, `quitting`, `socket_path_too_long` and `socket_error`. The catalogue of the CLI contract and the `llm-instructions` still lack them.
- `TESTANYWARE_APP_SOCKET` is a variable for tests in this increment, and it turns the routing on. `app-bundle-k10` makes macOS route by default. Describe the variable only as far as a person needs it.
- `app-default-routing-k26` made macOS route by default. `TESTANYWARE_APP_SOCKET` is now an override of the default socket, for tests, and a macOS host needs no variable. A Linux or Windows host routes only when the variable names a socket. The spec says so in "The socket and access". The catalogue of the CLI contract and the `llm-instructions` also lack `APP_NOT_INSTALLED`. Its remedy is `SETUP_REMEDY` in `testanyware-app-protocol/src/error.rs`, and `ERROR_CODES` lists the code.
- `config-evaluator-cli-k18` added `CONFIG_INVALID` (exit 2) to `ERROR_CODES` in `surface.rs` and to `exit_code_for`, and the variable `TESTANYWARE_CONFIG`. Both are still missing from the catalogue of the CLI contract, `docs/reference/error-codes.md`, `docs/reference/env-vars.md` and the `llm-instructions`. The file `~/.config/testanyware/config.rhai` has no user document yet. Its keys are `tools` (`tart`, `qemu-system-aarch64`, `swtpm`), `state_dir` and `data_dir`, each an absolute path.
- `doctor-app-check-k20` added the `app` check to `doctor`, and the schema `docs/reference/cli-schemas/doctor.json` and the spec describe it. `docs/reference/cli-commands.md` and `docs/components/cli.md` also describe `doctor`, and they lack the `app` check and the `config` check. The `llm-instructions` mention `doctor` as the first command to run, and they should say that it reports the app with a remedy for each failing item.
