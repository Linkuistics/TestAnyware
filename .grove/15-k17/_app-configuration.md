# app-configuration-k17 — brief

## Goal

Add the one optional file `~/.config/testanyware/config.rhai`. The Host CLI and the app evaluate it with one evaluator. It names the tool locations and the two directories.

## Done when

- One shared evaluator runs the script with no access to files, processes or the network, and under an operation limit. The value is a map of settings. An unknown key is an error.
- The settings are `tools` (the locations of `tart`, `qemu-system-aarch64` and `swtpm`), `state_dir` and `data_dir`.
- In the Host CLI, a flag beats an environment variable, which beats the file, which beats the built-in default. In the app, the file beats the built-in default.
- A file that fails gives `CONFIG_INVALID` (exit 2) with the file, the line and the message, in each Host CLI command except `doctor`.
- `doctor` runs with a file that fails. Its `config` check fails with the error, and `doctor` exits with 1.
- A macOS host reads the file only at the last step. By then the app reads it too, so the two never disagree about the directories.
- The app runs with its defaults when the file fails, and it shows the error in its menu.
- A directory mismatch between a request and the app names `config.rhai` in its remedy.
- On a macOS host, golden creation compares its directories with the app's before it starts.
- `TESTANYWARE_CONFIG` names another file, or `none`.

## Decomposition

1. `config-evaluator-cli`: the evaluator, the Host CLI side on Linux and Windows hosts, and the `doctor` exception with its `config` check. A macOS host ignores the file.
2. `config-in-app`: the app side, the menu and the remedy.
3. `golden-directory-check`: the activation on a macOS host and the check of golden creation. A test with the real CLI and the app proves both.

## Pointers

- Spec: `docs/specs/testanyware-app.md`, section "Configuration".
- ADR-0021.
- Glossary term: TestAnyware configuration.

## Notes

- The file is a delivery of its own for the Host CLI. Linux and Windows users gain it first.
- A macOS host stays inactive until the app can read the file. Without this order, a valid file with a new `state_dir` makes the CLI incompatible with an app that cannot evaluate it. Golden creation could also write where the app does not read.
- Policy functions for issues #5 and #7 are out of scope. The evaluator must not block them later.
