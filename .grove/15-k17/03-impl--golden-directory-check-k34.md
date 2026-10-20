# golden-directory-check-k34

## Goal

Turn `config.rhai` on for the Host CLI on a macOS host. Add the directory check of golden creation in the same step. The CLI and the app then agree on every setting, and a test with the real CLI and the app proves it.

## Context

- `docs/specs/testanyware-app.md`, sections "Configuration" and "Golden creation keeps its direct path", and the scenario "Golden creation with directories that differ".
- `vm-broker-design-k6`, finding F4.
- `config-evaluator-cli-k18`, which holds the activation switch and the `doctor` exception. `config-in-app-k33`, which makes the app read the file.
- The harness of `app-protocol-core-k9` that drives the app in its test mode with the real CLI.

## Done when

- The activation switch is on for a macOS host. The Host CLI there resolves `state_dir`, `data_dir` and the tool locations with the order flag, environment variable, file, default.
- On a macOS host, a file that fails gives `CONFIG_INVALID` with exit 2 in each command except `doctor`. `doctor` exits with 1 and its `config` check fails.
- Golden creation resolves the directories that the app resolves: those of `config.rhai`, else the built-in defaults. It uses no flag and no variable for this.
- When its own directories differ, golden creation fails with `APP_INCOMPATIBLE`, cause `directories`, before it starts. The error names both pairs and the remedy.
- The golden check needs no running app.
- On Linux and Windows hosts, golden creation does not run the check.
- Pure-function tests cover the match and the mismatch.
- Harness tests run the real CLI binary and the app with a configuration file that names a non-default `state_dir` and `data_dir`. A command succeeds when both read the same file. It fails with `APP_INCOMPATIBLE`, cause `directories`, when they differ. The tests also run a file that fails with an ordinary command and with `doctor`.

## Notes

- The spec lists `TART_HOME` as an open risk. Add no check for it.
- This leaf is the one place where the switch changes for a macOS host. Do not turn it on in an earlier leaf.
