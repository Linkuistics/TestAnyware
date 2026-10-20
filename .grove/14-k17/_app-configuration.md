# app-configuration-k17 — brief

## Goal

Add the one optional file `~/.config/testanyware/config.rhai`. The Host CLI and the app evaluate it with one evaluator. It names the tool locations and the two directories.

## Done when

- One shared evaluator runs the script with no access to files, processes or the network, and under an operation limit. The value is a map of settings. An unknown key is an error.
- The settings are `tools` (the locations of `tart`, `qemu-system-aarch64` and `swtpm`), `state_dir` and `data_dir`.
- In the Host CLI, a flag beats an environment variable, which beats the file, which beats the built-in default. In the app, the file beats the built-in default.
- A file that fails gives `CONFIG_INVALID` (exit 2) with the file, the line and the message, in each Host CLI command except `doctor`.
- The app runs with its defaults when the file fails, and it shows the error in its menu.
- A directory mismatch between a request and the app names `config.rhai` in its remedy.
- On a macOS host, golden creation compares its directories with the app's before it starts.
- `TESTANYWARE_CONFIG` names another file, or `none`.

## Decomposition

1. `config-evaluator-cli`: the evaluator and the Host CLI side, on every host.
2. `config-in-app`: the app side, the menu and the remedy.
3. `golden-directory-check`: the check of golden creation.

## Pointers

- Spec: `docs/specs/testanyware-app.md`, section "Configuration".
- ADR-0021.
- Glossary term: TestAnyware configuration.

## Notes

- The file is a delivery of its own for the Host CLI. Linux and Windows users gain it too.
- Policy functions for issues #5 and #7 are out of scope. The evaluator must not block them later.
