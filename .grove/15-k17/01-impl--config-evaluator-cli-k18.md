# config-evaluator-cli-k18

## Goal

Build the shared Rhai evaluator. Make the Host CLI honour `config.rhai` on Linux and Windows hosts, with the `doctor` exception and its `config` check. A macOS host ignores the file until `golden-directory-check-k34`.

## Context

- `docs/specs/testanyware-app.md`, section "Configuration", and the requirement "doctor reports the failures that it diagnoses".
- ADR-0021.
- The code in the CLI and the vm crate that reads the directory and tool variables today.
- The doctor code of the CLI and its report schema in `docs/reference/cli-schemas`.

## Done when

- A crate holds the evaluator. It limits operations, gives the script no access to files, processes or the network, and rejects an unknown key.
- One activation switch decides whether a host reads the file. It is on for Linux and Windows hosts. It is off for a macOS host. A parameter of the pure functions carries it, so that a test can turn it on for any host.
- With the switch on, the Host CLI resolves `state_dir`, `data_dir` and the tool locations with the order flag, environment variable, file, default.
- With the switch on, the Host CLI takes the tool locations where it runs the tools itself, golden creation included.
- With the switch on, a file that fails gives `CONFIG_INVALID` with exit 2, the file, the line and the message. Each command except `doctor` refuses to run.
- `doctor` runs with a file that fails. Its `config` check fails with the `CONFIG_INVALID` error object, and `doctor` exits with 1, not 2. Its other checks run with the values that the CLI resolves without the file.
- With the switch off, the `config` check reports that it does not apply to the host.
- The `--json` schema of `doctor` documents the `config` check.
- `TESTANYWARE_CONFIG` names another file or `none`.
- The crate builds in the Linux and Windows cross-builds with no C toolchain.
- Pure-function tests cover the order of sources, every error, and the report of `doctor` with a file that fails.

## Notes

- Without the file, every command behaves as before.
- A macOS host stays as it is until `golden-directory-check-k34` turns the switch on. The app cannot read the file before `config-in-app-k33`, and a CLI that read it earlier would disagree with its app about directories.
- `doctor-app-check-k20` adds the `app` check. It does not change the `config` check.
