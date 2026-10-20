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

## Decisions (running log)

- **The documents were converted first, in their own change.** The writing rules need an existing document to pass the checker before an edit. Eight files had hard findings. The conversion landed as `docs: convert the documents that k35 edits to controlled English`. `docs/architecture/cli-design-contract.md` is large (76 hard findings, 4339 words), so it keeps `pending` regions for the sections that this leaf does not edit. The edited sections are in `on` regions and pass.
- **A test guards the documents.** `documents_list_every_error_code_and_public_variable` in `cli-contract.rs` compares each code of `ERROR_CODES` with `docs/reference/error-codes.md` and the contract, and each public variable of `capabilities` with `docs/reference/env-vars.md`. It failed first, and it found one gap that the leaf did not name: `VM_HOST_UNSUPPORTED` was in no document. I documented it in both. `llm_instructions_describe_the_app` pins the app section of the guide.
- **`AGENT_INVALID_JSON` joins `ERROR_CODES`.** The contract table of section 4.5 already listed it, but `surface.rs` did not, so `capabilities` hid it.
- **`capabilities` lists two more variables.** Contract section 9.5 needs each behaviour variable in `capabilities`. `TESTANYWARE_CONFIG` is public, because the remedy of `CONFIG_INVALID` tells a person to set it. `TESTANYWARE_APP_SOCKET` is internal, as `TESTANYWARE_RFB_ENCODING` is.
- **The resolution chain was wrong in more places than the leaf named.** clap reads `TESTANYWARE_VM_ID` as the value of `--vm`, so it comes before `--agent` and `--vnc`. I corrected the doc comment of `resolve.rs`, `docs/reference/env-vars.md`, `docs/reference/cli-commands.md` and `LLM_INSTRUCTIONS.md`. `LLM_INSTRUCTIONS.md` and `env-vars.md` also said that `TESTANYWARE_AGENT` overrides the spec file. The code never did that, because the spec file comes first. I removed the claim.
- **`<id>.json` is no longer a contract for direct readers.** `connection-spec.md` says so. `multi-vm-networking.md` now tells a reader to run `vm list` for an id, and no longer to list the state directory. The same file says that QEMU listens on `localhost`, for the agent and for VNC.
- **The guide grew to 13.9 KB.** The test ceiling is 18 000 bytes. The contract asks for well under 3000 tokens, which is about 12 KB at four bytes a token. So the guide now sits above that estimate. I kept the app section to one table. I added no `--topic` flags, which section 8.3 allows as the way to grow. A later leaf can add them if the size matters.
- **`doctor` is documented in `cli-commands.md` only.** `docs/components/cli.md` lists the crates of the app and the route, and says where `doctor` holds its checks. The schema was already complete from `doctor-app-check-k20`.
- **The `config.rhai` user document.** The keys are in a section of `docs/reference/env-vars.md` and in the guide. I made no separate user page, because the file has three keys and one place to find them is enough.

## Changes to the ADR set, the spec and the glossary

- **ADR-0021:** one added sentence. The remedy of `CONFIG_INVALID` also names `TESTANYWARE_CONFIG=none`, and only the Host CLI reads the variable.
- **`docs/specs/testanyware-app.md`:** "How the app starts" names `APP_NOT_INSTALLED` for a host with no bundle. "Configuration" says that `TESTANYWARE_CONFIG` has two users, a test and the `CONFIG_INVALID` remedy. "Documents that the implementation corrects" is now in the past tense, and it lists the three extra corrections above.
- **ADR-0004, ADR-0019, ADR-0020 and `CONTEXT.md`:** no change. Each agrees with the shipped behaviour.
