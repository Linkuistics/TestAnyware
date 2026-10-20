# config-in-app-k33

## Goal

Make the app evaluate `config.rhai` when it starts, and use every setting.

## Context

- `docs/specs/testanyware-app.md`, section "Configuration".
- ADR-0021.
- `config-evaluator-cli-k18`, `app-protocol-core-k9` and `app-menu-bar-shell-k11`.

## Done when

- The app evaluates the file at start. A restart reloads it.
- The app uses `tools`, `state_dir` and `data_dir`. Without them it uses its built-in tool search and default directories.
- When the file fails, the app runs with its defaults. The menu shows the file, the time of the last evaluation and the error, with an item that opens the file.
- A request with directories that differ from the app's fails with `APP_INCOMPATIBLE`, cause `directories`. The remedy names `config.rhai`, the variable that differs, and a restart of the app when it evaluated an older file.
- `GET /v1/status` reports the configuration state, so that `doctor` can show it.
- Tests in the harness cover a failing file and a directory mismatch.

## Notes

- Replace the interim remedy of `app-protocol-core-k9` that names only the variable.
