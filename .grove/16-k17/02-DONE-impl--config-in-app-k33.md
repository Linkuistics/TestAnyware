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

- The Host CLI of a macOS host does not read the file until `golden-directory-check-k34`. The tests set the directories of the CLI with its environment variables and flags.
- Replace the interim remedy of `app-protocol-core-k9` that names only the variable.

## Decisions (running log)

- **The app holds an `AppConfig` in its `AppContext`, fixed at start.** It replaces the unused `config_path`. `AppConfig::evaluate` is a pure function of a location, a reader, a `stat` and a clock, so each outcome (disabled, absent, loaded, failed) has a test with no file system. It uses the evaluator of `testanyware-config`, the crate that the Host CLI uses.
- **In the app the file beats the built-in default, and nothing else is in the order.** The built-in default of the directories stays what `app_paths` computed before (the process environment, then `~/.local`). The file replaces a directory that it names and leaves the other. A file that fails gives both defaults and no tool location.
- **Test mode reads no file unless `--config` names one.** Without that rule a developer's own `~/.config/testanyware/config.rhai` would reach every test. With `--config`, the file beats `--state-dir` and `--data-dir`, which stand in for the built-in defaults. Outside test mode the location comes from `testanyware_config::locate` on the process environment, so `TESTANYWARE_CONFIG` works for the app as the spec says.
- **The status carries a `configuration` object.** Fields: `state` (`disabled`, `absent`, `loaded`, `failed`), `file`, `evaluated_at` (seconds since the epoch), `changed_since_evaluation` and `error` (the `CONFIG_INVALID` object that the Host CLI reports for the same file). The field has `#[serde(default)]`, as the other late fields do, so an older body still reads. `AppError` gained `Eq` for this.
- **"The app evaluated an older file" is a fingerprint, not a guess.** The app records the length and modification time of the file just before it reads the text. A status or a mismatch compares them with the file as it is now. The fingerprint comes before the read, so an edit between the two counts as a change. A rewrite that keeps both the length and the modification time to the clock tick is not seen. I accept that, because the remedy is only a restart.
- **The directory remedy names `config.rhai` and the setting that differs.** `incompatible_directories` takes the `ConfigurationStatus`. It names `XDG_STATE_HOME` and `state_dir` when the state directory differs, and the data pair when the data directory differs. A file that fails adds that the app runs with its built-in directories and why. A file that changed adds "quit and open TestAnyware". The gate reads the file only for a request that fails on its directories.
- **The menu section sits after Start at Login and before Quit, as the spec lists it.** It shows `Configuration: <file>`, with `(failed)` when the file fails, `Last evaluated: <time>`, the error with its line when Rhai placed one, and a note that the app runs on its built-in defaults. It adds "The file changed since then. Quit and open TestAnyware to read it" when the file moved on. An item `Open config.rhai` appears when a file is there to open, which includes a file that fails, because that is the file a person must fix. An absent file gets no item, because `open` would have nothing to open. `menu::build_menu` stays a pure function, and the new entries have unit tests.
- **The time is UTC.** The menu is pure and runs in tests on any host, so it asks no host for a time zone. `format_utc` converts seconds since the epoch with Hinnant's civil-from-days algorithm, and a test checks four known instants. A person in another zone reads UTC. I accepted that rather than add a date crate or an AppKit formatter to a text line.
- **The item opens the file with `/usr/bin/open -t`.** A `.rhai` file has no application of its own, so `NSWorkspace.openURL` would ask the person which one to use. I clicked the item through Accessibility on this Mac, and TextEdit opened `config.rhai`.
- **The live script had a bug that this leaf's check uncovered.** `scripts/app-bundle-live.sh` closed the menu with `key code 53` (Escape). On macOS 27.0.1 the menu stays open, so the app stays in its menu tracking loop (`sample` shows `NSMenuTrackingSession`), which handles no Apple event, and the quit of step 6 times out after two minutes. Earlier runs had no Accessibility grant and skipped the menu step. `perform action "AXCancel"` closes the menu and the quit then takes 37 ms. The script now uses it. It also gained a second pass: the same bundle opened with `open --env TESTANYWARE_CONFIG=<file>` and a file that fails, with a check of the status and of the menu.
- **A mutation check of the harness.** With `state_dir` of the file ignored, three harness tests fail. With the tool locations ignored, the test that names a `tart` fails. Both mutants were restored.
- **Not done, and why.** `app-default-routing-k26` left a note for this leaf: the `socket_path_too_long` remedy points at `TESTANYWARE_APP_SOCKET`, which the app does not read outside test mode, so a person with a long `HOME` has no way out. The "Done when" of this leaf and ADR-0021 list `tools`, `state_dir` and `data_dir` only, and a `socket` setting would widen the spec. I did not add it. It needs a decision of its own (a setting, or a remedy that names another way out).
