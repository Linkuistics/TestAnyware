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

## Decisions (running log)

- **The switch is on for every host.** `Activation::for_this_host()` returns `Active`, and the `macos` arm is gone. The parameter of the pure functions stays, so a test can turn the file off. `ConfigStatus::NotApplicable` and the `not_applicable` verdict stay too. No real host reports them now. I reworded the `doctor.json` description to say so. Deleting the enum value would widen this leaf into the `doctor` renderer. The tests of earlier leaves are not affected, because the harness sets `HOME` to a temporary directory with no file in it.
- **The app side of the check is a second reading of the default file.** `host_config::resolve` also computes `HostConfig.app` (`AppView`). It holds the directories that the app resolves and the file the app reads. The view takes `state_dir` and `data_dir` from `~/.config/testanyware/config.rhai`. It falls back to the built-in defaults under `HOME`. It ignores `TESTANYWARE_CONFIG`, because the spec says golden creation "uses no flag and no variable for this". That variable names the file of a test, and the app of a person never gets it. A person who exports it to a file that moves a directory gets `APP_INCOMPATIBLE`. That is correct, because the app reads the default file. A file that fails gives the built-in directories, as it does in the app. Every command except `doctor` refuses such a file first, so golden creation never reaches that case.
- **The app's built-in directories now come from `HOME` alone.** `app_paths` called `VmPaths::from_process_env()`. That function honours `XDG_STATE_HOME` and `XDG_DATA_HOME` in the app's own environment. A login item has none, but `open --env` or `launchctl setenv` can set them. The app would then hold directories that golden creation cannot know. `VmPaths::builtin_for_home(home)` is the one function that both sides use for the default, so they agree by construction. This is the only change of behaviour in the app. The Host CLI still honours its variables, and a variable that differs from the app gives the remedy of `APP_INCOMPATIBLE`. ADR-0021 rejects the option "the app ignores XDG". That still holds. The variable keeps its effect in the Host CLI, and the app never saw the shell of a user. I added a consequence line to the ADR.
- **The check is `host_config::check_golden_directories(host, runs_the_app)`.** It is a pure function. The one call site passes `cfg!(target_os = "macos")`, so Linux and Windows hosts do not run it. A unit test shows that the same mismatch passes with `false`. The function builds the error with the existing `AppError::incompatible_directories`. The message names both pairs. The remedy is the one an ordinary command gets, with the file that the app reads. The check needs no running app.
- **Where the check runs.** It runs in the three `create-golden` functions, in the `cfg(target_os = "macos")` block. It comes after the `--dry-run` return and before the first side effect. A dry run mutates nothing and prints a plan, so it does not refuse. `CONFIG_INVALID` comes first, from the refusal before the dispatch. A file that fails therefore gives exit 2 in golden creation, as in every command except `doctor`.
- **The tests.** Unit tests in `host_config.rs` cover these cases: match, mismatch, the order of the sources, and no variable on the app side. They also cover a file that fails, a host without the app, and a switch that is off. Harness tests in `tests/golden-directory-check.rs` run the real CLI and the real app executable on one file. The file sits at the default place of the CLI, and the app gets it with `--config`. A command succeeds when the two agree. It fails with `APP_INCOMPATIBLE`, cause `directories`, when a variable moves a directory. Golden creation passes its check with no app running. It fails before it starts when its directories differ. The pair that golden creation names for the app equals the pair that the running app names in a refused command. A file that fails stops an ordinary command and golden creation with exit 2, and `doctor` exits with 1. No test lets golden creation into the real pipeline. `HOME` has no SSH key, so the pipeline stops at once. The file also names a `tart` that does not exist.
- **A mutation check of the tests.** I disabled the comparison, and three golden tests failed. I made the app view ignore the file, and three failed. I switched the file off for macOS, and six failed. I restored each mutant.
- **A live check on this Mac.** `scripts/app-bundle-live.sh` gained step 4b. The signed development bundle runs. A Host CLI at an `XDG_DATA_HOME` that the app cannot have gets a refusal from `vm list` and from `vm create-golden`. Both errors name the same app directories, `/Users/antony/.local/{state,share}/testanyware`. This shows that the production default of the app equals the golden assumption. The harness cannot show it, because the app in test mode takes its defaults from arguments. `task app:bundle-live` now depends on `cli:build`. The run passed.
- **Not done.** I added no check for `TART_HOME`, which the spec lists as an open risk. The `socket_path_too_long` remedy that `config-in-app-k33` left open is still open.
