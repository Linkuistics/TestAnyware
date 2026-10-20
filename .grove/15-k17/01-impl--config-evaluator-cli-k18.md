# config-evaluator-cli-k18

## Goal

Build the shared Rhai evaluator, and make the Host CLI honour `config.rhai` on every host.

## Context

- `docs/specs/testanyware-app.md`, section "Configuration".
- ADR-0021.
- The code in the CLI and the vm crate that reads the directory and tool variables today.

## Done when

- A crate holds the evaluator. It limits operations, gives the script no access to files, processes or the network, and rejects an unknown key.
- The Host CLI resolves `state_dir`, `data_dir` and the tool locations with the order flag, environment variable, file, default.
- The Host CLI takes the tool locations where it runs the tools itself: on Linux and Windows hosts, and in golden creation.
- A file that fails gives `CONFIG_INVALID` with exit 2, the file, the line and the message. Each command except `doctor` refuses to run.
- `TESTANYWARE_CONFIG` names another file or `none`.
- The crate builds in the Linux and Windows cross-builds with no C toolchain.
- Pure-function tests cover the order of sources and every error.

## Notes

- Without the file, every command behaves as before.
- The `doctor` exception comes in `doctor-app-check-k20`.
