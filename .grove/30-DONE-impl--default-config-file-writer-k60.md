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

## Decisions (running log)

- **The text is a pure function of the home directory, in `testanyware-config/src/default_file.rs`.** `default_text(home)` returns comments around an empty map. The examples sit inside the braces, so deleting the `//` of each example line leaves a map that evaluates. An example line is `//` and two spaces, and a prose comment is `// ` and a word. The test finds the example lines by that rule. The map is empty, so the text sets nothing, as the spec requires.
- **The home directory is escaped for a Rhai string.** A quote, a backslash or a line break in the path would end the string, or the comment line, early. A test uses the home `/Users/a"b\c<newline>d`. The text still evaluates, and the examples still hold that path.
- **The test of the keys takes them from the evaluator's own constants.** The test uses `TOOL_NAMES`. The top-level keys `tools`, `state_dir` and `data_dir` are listed in the test, because the evaluator keeps them in a private constant. A second test builds a map from that list and evaluates it, so a key that the evaluator drops fails that test. I did not export the private constant, because that adds a public name for a test.
- **The check of the examples can fail.** The test replaces `state_dir:` with `stat_dir:` in the uncommented text, and the evaluator rejects it. A copy of the text with comments alone is also rejected. The deletion test also asserts that it changed the text and that every example set something.
- **The write step lives in `testanyware-app/src/default_file.rs`, not in `config.rs`.** `AppConfig::evaluate` and the module keep their statement that everything in them is a pure function of its arguments. `write_if_absent(file, text)` is the seam: a unit test passes a path in a temporary directory.
- **The mechanism is the one the leaf suggests.** The step checks `symlink_metadata` first, then creates the directory, then writes the text to `.config.rhai.<pid>.tmp` with `create_new` and `sync_all`, then calls `hard_link` to the final name, then deletes the temporary file whatever the result. The pre-check only saves work. A test calls `hard_link` on its own after another file appears, and it gets `AlreadyExists` and leaves the other file whole. That test proves the link is what refuses.
- **The decision to write is a pure function too.** `plan(get, location, home)` returns the file and the text only when `TESTANYWARE_CONFIG` is unset or empty, the location is a file, and a home directory is known. `main.rs` passes `None` in test mode, so a test-mode run never writes. Only `app_paths()` on macOS calls `plan`. A set `TESTANYWARE_CONFIG`, whether it names a file or `none`, plans nothing.
- **The write comes after the socket binds and before the evaluation.** A second app that the login item starts exits at the bind, so it writes nothing. A failed write prints the failure on the app's standard error, and the app evaluates as with no file. The status gains no field, and no flag or command changes.
- **The tests of the failures use two real failures.** One makes the directory of the file a file, so no directory can exist there. One removes the write permission of the directory, and it skips its assertions when the process can write there anyway, as root can.
- **The two checks of a test-mode run can fail.** I made test mode plan a write for a moment. The assertion that the absent `--config` file stays absent then failed. I restored `main.rs` and ran the test again. A second integration test runs the real executable with an empty `HOME` and asserts that `HOME` is as empty as before.
- **The live script writes a check only for a Mac that had no entry.** It notes whether `~/.config/testanyware/config.rhai` exists before the first open. It opens with `env -u TESTANYWARE_CONFIG`, so the open states that no variable names another file. Without an entry before the open, it requires the file, the state `loaded` and `changed_since_evaluation` false. `shfmt` rewrote `((!had_config))` to `((! had_config))`, and `task scripts:check` passes.
- **`task config:test` runs the new tests.** The `--lib` filter gains `default_file::`, which covers the app. The tests of the text sit in `cargo test -p testanyware-config`, which the task already ran. I also corrected the description of the task. My first edit put `: ` into a plain YAML scalar and broke the Taskfile, and `task` said so at once.
- **The documents.** `docs/reference/env-vars.md` ("config.rhai"), `docs/reference/cli-commands.md` (the `config` check of `doctor`) and `LLM_INSTRUCTIONS.md` each gain the sentence on the written file. The sentence in `cli-commands.md` names `loaded` after the first start of the app on a macOS host, and `absent` before it and on Linux and Windows hosts. The spec and ADR-0021 needed no change, because the implementation matches them.
- **The claim that no other document says only a person writes the file.** I enumerated every file that names `config.rhai`, with `rg -l --hidden` outside `target`, `.git`, `.jj` and `.grove`. The result is 40 files. I classified each mention in the files that are documents, the schema and the strings of the CLI: `CONTEXT.md`, `docs/components/cli.md`, `docs/architecture/cli-design-contract.md`, `docs/reference/error-codes.md`, `docs/reference/cli-schemas/doctor.json`, `discoverability.rs`, `main.rs` and `doctor.rs` of the CLI. Each one states a fact about the file's format or its errors. None states who writes it, so no other document needed the sentence. The positive control was a search for a phrase of my own new sentence, and it found `env-vars.md`. The failing control was a search for a token that exists nowhere, and it printed nothing and exited 1. A second search over the Markdown, JSON, Ruby and YAML files for "config file", "configuration file", "only a person", "by hand" and "hand-written" found one true historical sentence, in ADR-0021 ("Until now TestAnyware had no configuration file"), and no claim to correct. The copy of the documents under `website/docs/` never mentions `config.rhai`, so it needs no change.
- **I quit the installed release app for the live proof, and I opened it again.** `app-bundle-live.sh` refuses to run while an app answers on the socket, and `/Applications/TestAnyware.app` (2.4.0) was running. The app held 15 adopted VMs, which outlive it, and `k26-live` was running. I quit it with the Apple event, ran the proof and opened it again with `open -a`. It came back on build 2.4.0, adopted the same 15 VMs and reported `configuration.state` `loaded`. So the released app reads the file that the new app wrote without error.
- **No in-session reviewer, and no `review-impl` leaf.** The write step has unit tests for every property in the spec, and the live proof exercised the real call on this Mac. The next release candidate will contain the change, and a reviewer of the candidate can read the step.

### The live proof

Before: `~/.config/testanyware` did not exist (`ls -la ~/.config/testanyware` gave `No such file or directory`). The environment of the open had `TESTANYWARE_CONFIG` unset. The release app (2.4.0) was quit first.

```
$ scripts/app-bundle-live.sh        # exit 0, 20 s
app-bundle-live: ok   built .../cli-rs/target/app/dev/TestAnyware Dev.app (0.0.1)
app-bundle-live: ok   GET /v1/status answers
app-bundle-live: ok   build 0.0.1 matches the bundle
app-bundle-live: ok   the status reports the configuration (loaded)
app-bundle-live: ok   the app wrote /Users/antony/.config/testanyware/config.rhai and evaluated it
app-bundle-live: ok   golden creation expects the directories that the app holds (...)
...
app-bundle-live: ok   a config.rhai that fails runs the app on its defaults and the status reports the error
app-bundle-live: PASS
```

The file after the first start (`ls -la ~/.config/testanyware/`): `-rw-r--r--@ 1.2k antony  config.rhai`. It holds the comments, the map `#{ ... }` and the examples with `/Users/antony/.local/state/testanyware` and `/Users/antony/.local/share/testanyware`.

After the first start, I opened the new bundle again with `env -u TESTANYWARE_CONFIG open -n -g` and read it:

```
$ curl --unix-socket "$HOME/Library/Application Support/TestAnyware/testanyware.sock" http://localhost/v1/status | jq -c .configuration
{"changed_since_evaluation":false,"error":null,"evaluated_at":1792675529,"file":"/Users/antony/.config/testanyware/config.rhai","state":"loaded"}

$ cli-rs/target/debug/testanyware doctor --json | jq -c .checks.config
{"file":"/Users/antony/.config/testanyware/config.rhai","status":"pass","verdict":"loaded"}

$ cli-rs/target/debug/testanyware doctor
Configuration file
  ✓ /Users/antony/.config/testanyware/config.rhai evaluated
...
  ✓ configuration: /Users/antony/.config/testanyware/config.rhai evaluated
  ✓ directories: the Host CLI and the app resolve the same directories
```

The file stays at `~/.config/testanyware/config.rhai`. It changes no setting, so it can stay.

### Checks

- `cargo test --workspace`: exit 0, 57 test binaries ok, no failure.
- `task config:test`: exit 0. `task scripts:check`: exit 0.
- `cargo clippy -p testanyware-config -p testanyware-app --all-targets`: no warning in a file of this leaf. The two warnings in `testanyware-vm` are the ones the Taskfile already records.
- `rustfmt --edition 2021 --check` on the files this leaf touches: clean. The repository has older drift elsewhere, and I left it.
