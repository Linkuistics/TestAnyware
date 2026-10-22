# default-config-file-writer-k60

## Goal

Make the TestAnyware app write `~/.config/testanyware/config.rhai` when it starts and finds no entry there. The spec states this in "Configuration", and ADR-0021 records it. The written file changes no setting.

## Context

- The design is in `docs/specs/testanyware-app.md`, section "Configuration", and in ADR-0021. The decision log of `default-config-file-k58` holds the evidence and the rejected writers. Find it with `grove-llm resolve default-config-file-k58`. The planning leaf `default-config-file-increments-k59` cut this leaf as the only increment. The text, the write step and the call cannot work apart, so one slice holds all three.
- The text of the file is a pure function of the home directory. It lives in the crate `testanyware-config`, beside `DEFAULT_RELATIVE_PATH` in `location.rs` and the evaluator in `evaluate.rs`. The crate's tests run the text through that evaluator.
- The write step lives in the app. `AppConfig::evaluate` in `testanyware-app/src/config.rs` stays a pure reader. `main.rs` calls `AppConfig::evaluate_now(&paths.config)` once, after `app_paths()` and after the socket binds. The write goes just before that call, and only for a normal run (`!args.test_mode`) whose location is the default place. A file that `TESTANYWARE_CONFIG` names is never written.
- One mechanism gives the two properties of the write. Write the text to a temporary file in the same directory. Then `hard_link` it to the final name, and delete the temporary file. `hard_link` fails when the name exists, a dangling symlink included. On a file system with no hard links the write fails, and the app runs as with no file. Create the missing directory first. Another mechanism that gives both properties is acceptable.
- Put the write step behind an injected seam. A unit test then drives it in a temporary directory, with no real home directory. The existing tests in `testanyware-app/tests/configuration.rs` show the style. The test `a_test_mode_run_without_config_reads_no_file_of_the_user` guards a developer's own file and must stay green.
- The documents that describe the file each gain a sentence on the written file:
  - `docs/reference/env-vars.md`, section "config.rhai".
  - `docs/reference/cli-commands.md`, the `doctor` `config` check. On a macOS host the verdict is `loaded` after the first start of the app. It is `absent` before that start and on Linux and Windows hosts.
  - `LLM_INSTRUCTIONS.md`, where it says that `config.rhai` is optional.
- `scripts/app-bundle-live.sh` accepts `absent` or `loaded` at its first open, so it passes with the written file. Add a check, after that first open, that the file exists when `TESTANYWARE_CONFIG` was unset in the environment of the open. Keep the header comment of the script in step. The `config:test` task in `Taskfile.yml` lists the config tests, so add the new test targets to it.

## Done when

- The unit tests of the test seams in the spec pass:
  - The text evaluates, and it sets no setting: the evaluator returns the empty configuration.
  - The text with the `//` deleted from each example line still evaluates. Show that the check can fail. A mutated copy of the text, such as one with `stat_dir`, must fail.
  - Each key of the evaluator (`tools` with each of `TOOL_NAMES`, `state_dir`, `data_dir`) appears in the text. The test takes the keys from the evaluator's own constants, not from a copy.
  - The examples of `state_dir` and `data_dir` name the built-in directories under the home directory that the function was given.
  - The write step, in a temporary directory, writes when no entry is there, creates a missing directory, and leaves no temporary file. It never replaces a file, an empty file, a directory or a dangling symlink at the path. A failed write returns the failure and leaves the path as it was.
  - The app evaluates the file that it wrote, with `changed_since_evaluation` false.
  - A test-mode run writes nothing, and a run with `TESTANYWARE_CONFIG` set writes nothing at the default place.
- `cargo test --workspace` passes, and `task config:test` runs the new tests. Clippy and the format check that the project runs stay green.
- The three documents above carry the sentence on the written file. No other document states that only a person writes the file. Check the claim by enumeration, with a positive control and a failing control, as `references/execute.md` of the grove skill requires.
- The live proof: this Mac has no `~/.config/testanyware/config.rhai` today. Run the new app for the first time through `scripts/app-bundle-live.sh`, with the environment of a normal start. Afterwards the file exists, `GET /v1/status` reports `configuration.state` `loaded`, and `testanyware doctor` reports the `config` check as passing with `loaded`. Record the commands and their output in the decision log. If a file is already there when the session starts, move it aside, run the proof, and put it back. Never overwrite it.
- Every command, flag, environment variable, exit code and `--json` field keeps its meaning. The Host CLI does not write the file.

## Notes

- This work is outside the issue #8 delivery. The owner asked for focused deliveries, so it is a separate one.
- Many projects drive TestAnyware through its CLI. The change adds no field to the status and no flag to a command.
- **When it ships.** The change ships with the next release of TestAnyware, together with `linux-golden-disk-size-k57`. The v2.4.0 release is published without either. The owner decides when a later release happens. A release for a file of comments alone is not worth the cost of the release pipeline. So planning cut no release leaf. A release candidate built after this leaf contains the change. This session does not build, tag or publish a release.
- The release scripts treat a change outside `.grove/` and the documents as a change of the candidate. A later release builds a new candidate, so this change needs no extra step there.
- Sequencing: this leaf can run before or after any other work. It touches no file that a live leaf touches.
