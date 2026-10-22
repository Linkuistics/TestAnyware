# default-config-file-increments-k59

## Goal

Cut the increments that make the TestAnyware app write `~/.config/testanyware/config.rhai` when it starts and finds none, by the design of `default-config-file-k58`.

## Context

- The design is in the spec `docs/specs/testanyware-app.md`, section "Configuration" and requirement "The app writes the file when it finds none". ADR-0021 records the writer and the rejected writers.
- The decision log of `default-config-file-k58` holds the evidence: a candidate file tested with the installed CLI 2.4.0, and the negative controls. Find it with `grove-llm resolve default-config-file-k58`.
- The design in short: a normal run of the app writes a file of comments and an empty map, `#{}`, before it evaluates the file. The file changes no setting. The app never replaces an entry at the path. The Host CLI never writes the file.
- The shared evaluator is the crate `testanyware-config`. Its `locate` gives the default place, `DEFAULT_RELATIVE_PATH`. The text of the file can live in this crate as a pure function of the home directory, beside the evaluator that its tests run.
- The app evaluates the file in `AppConfig::evaluate` in `testanyware-app/src/config.rs`. Its `main.rs` calls `AppConfig::evaluate_now` once, after `app_paths()` for a normal run. The write step goes before that call, and only for a normal run.
- This is one way to give the two properties of the write. Write the text to a temporary file in the same directory. Then `hard_link` it to the final name, and delete the temporary file. `hard_link` fails when the name exists, a dangling symlink included. On a file system with no hard links, the write then fails, and the app runs as with no file.
- The documents that describe the file: `docs/reference/env-vars.md` (section "config.rhai"), `docs/reference/cli-commands.md` (the `doctor` `config` check) and `LLM_INSTRUCTIONS.md`. Each one gets a sentence on the written file.
- `scripts/app-bundle-live.sh` is the live check of the bundle. Its first open accepts `absent` or `loaded`, so it passes with the written file. It can gain a check that the file exists after that open.

## Done when

- The tree holds the increments of the design, each a vertical slice that a session can prove alone. One `impl` leaf may be enough.
- Each increment states its proof. The unit tests are those of the test seams in the spec. The text evaluates to no setting, and each example evaluates without its `//`. Each key of the evaluator is in the text, and the write step never replaces an entry. The live proof is the first start of the new app on this Mac, which has no file today.
- The tree records when the change ships. The v2.4.0 release is published without it.

## Notes

- This work is outside the issue #8 delivery. The owner asked for focused deliveries, so it is a separate one.
- Every command, flag, environment variable, exit code and `--json` field keeps its meaning. Many projects drive TestAnyware through its CLI.
- `linux-golden-disk-size-k57` also ships with the next release, and no release leaf exists for it. Planning can decide whether one release carries both.
- A test-mode run of the app must write nothing. The test `a_test_mode_run_without_config_reads_no_file_of_the_user` guards a developer's own file.

## Decisions (running log)

- **One increment, one `impl` leaf: `default-config-file-writer-k60`.** The text of the file, the write step and the call in `main.rs` cannot work apart. The text with no writer is dead code, and a writer with no text writes nothing. The documents describe the written file, so they are wrong before the writer lands and belong in the same slice. The live check of the bundle proves this slice. It is not a further increment. The slice fits one session. It holds a pure function in `testanyware-config`, a short write step in the app, and three document sentences.
- **No node.** `linux-golden-disk-size-k57` set the precedent of one root leaf for a single slice. A second leaf would give a node no purpose.
- **No release leaf. The change ships with the next release, together with `linux-golden-disk-size-k57`.** The v2.4.0 release is published without either. The owner decides when a later release happens. A file of comments alone does not justify the cost of the release pipeline. A candidate built after the writer lands contains both changes. The leaf, the root brief and this log record this.
- **No review leaf cut.** The write step is small and bounded. Its unit tests cover the two properties that matter: it never replaces an entry, and it never shows a partial file. The implementing session can spend its one in-session reviewer on the hard-link mechanism. A second need then becomes a `review-impl` leaf.
- **The leaf names its proof.** The unit tests are those of the spec's test seams, with a failing control for the example check. The live proof is the first start of the new app on this Mac, which has no file today.
- **Source check.** The leaf names `AppConfig::evaluate`, `evaluate_now`, `app_paths()`, `DEFAULT_RELATIVE_PATH`, `TOOL_NAMES`, the test `a_test_mode_run_without_config_reads_no_file_of_the_user`, `scripts/app-bundle-live.sh` and `task config:test`. I read each of them in the tree. I spent no in-session reviewer.
