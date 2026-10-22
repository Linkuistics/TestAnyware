# default-config-file-k58

## Goal

Decide whether TestAnyware installs a minimal configuration file when it finds none, and if so which program writes it, where, and with what content.

## Context

- The owner raised this on 2026-10-22, during `linux-golden-disk-size-k57`, in these words: "We should probably install a minimal configuration file if none is found."
- The words do not name the file. The likely file is `~/.config/testanyware/config.rhai`. It does not exist on this Mac today.
- ADR-0021 makes that file optional. A file that fails makes the app run on its defaults, and makes the Host CLI refuse to run. A written default file changes that story, so the ADR may need a rework.
- The brief says one optional Rhai file configures the app and the Host CLI. The brief's done-when list says "optional".
- Questions the design must answer:
  - Which program writes the file: the Host CLI, the app at its start, the brew formula, or `doctor`?
  - What does the file hold? ADR-0021 allows only tool locations, `state_dir` and `data_dir`. A file of commented examples changes no setting.
  - What happens when the file exists already? The design must never overwrite it.
  - Does a written file break a test that names no file or `TESTANYWARE_CONFIG=none`?
  - Is the benefit worth the cost? A written file shows a user what the file can hold, and `doctor` could show that as well.

## Done when

- A design records the answers to the questions above, the rejected options and the reasons.
- The ADR and the spec change if the decision changes a behaviour that they state.
- The design keeps every command, flag, environment variable, exit code and `--json` field.

## Notes

- This leaf is outside the issue #8 delivery and outside `clone-disk-size`. The owner asked for focused deliveries, so it is a separate one.
- `requirements` may be the better first kind if the owner's words do not settle which file is meant. A `design` session can hand the question back to the owner.

## Decisions (running log)

- **The file is `~/.config/testanyware/config.rhai`, so the question needs no `requirements` leaf.** TestAnyware has no other configuration file. ADR-0021 states that TestAnyware had none before it.
- **The written file changes no setting.** It holds comments and an empty map, `#{}`, as its value. Each key appears in a comment with an example path. I tested a candidate with the installed CLI 2.4.0 on this Mac, through `TESTANYWARE_CONFIG` and `doctor --json`. The commented file gave `loaded`. The same file with each `//` deleted also gave `loaded`, so the examples are valid Rhai: trailing commas, a nested map and the quoted key `"qemu-system-aarch64"`.
- **The evaluator requires the empty map.** A file of comments alone gave `invalid`, because its value is `()` and the evaluator requires a map. A file with the misspelled key `stat_dir` also gave `invalid`. These two negative controls show that the check can fail.
- **The TestAnyware app writes the file, when it starts and finds no file.** The write comes before the fingerprint and the evaluation. So the app evaluates the file that it wrote, and it reports no change that needs a restart. Only a normal run of the app writes, and only at the default location. A test-mode run never writes, so each test keeps its file or its absent file.
- **The app writes the file again at a later start when a person deletes it.** The words of the owner are "if none is found". A record of an earlier write would be new state, for a file that changes nothing. The comments say that TestAnyware wrote the file.
- **The app never replaces an entry at the path, and a person never sees a partial file.** The app writes the text to a temporary file in the same directory. It then links that file to the final name with a call that fails when the name exists, and deletes the temporary file. A dangling symlink at the path also counts as an entry.
- **A failed write changes nothing else.** The app then runs as with no file, and it reports the failure on its standard error. The status of the app gains no field, so its protocol does not change.
- **The examples name the home directory of the user.** A person can then delete the `//` and keep a valid absolute path. The comment of `tools` names the built-in search of the app, not one found location.
- **Rejected: the Host CLI writes the file at a command.** The CLI runs inside guests during a harness run, in tests with a temporary `HOME`, and in agent sandboxes that may refuse a write to `~/.config`. A write after the app started would also make the app report a changed file and ask for a restart. `gh` writes its own `config.yml` on first use, so the pattern has precedent. But here the app is the one long-lived process, and it starts before any command.
- **Rejected: the formula's `post_install`.** Homebrew 7.0.10 runs that step in a sandbox. It allows writes to the formula's logs, its cellar, its `etc` and the directories under `HOMEBREW_PREFIX`, not to `~/.config` (`formula_installer.rb`, `post_install`).
- **Rejected: a `postflight` in the cask.** It runs only at an install or an upgrade, so a person who installs from the archive or builds from the source gets no file. The cask template already has no `postflight`, by the choice of ADR-0020. A test of the step needs a live Homebrew install.
- **Rejected: `doctor` writes the file, or a new command writes it.** `doctor` diagnoses and changes nothing. A new command adds to the CLI surface, which many projects depend on. The owner asked for a file that is there when none is found, not for a step that a person must know about.
- **Rejected: an example file in the formula's `share` directory, which `doctor` names.** It needs a step by a person, which the request avoids.
- **Linux and Windows hosts get no written file.** No app runs there, and the Host CLI never writes. The Linux host is build-verified, and its harness runs the CLI in guests. A later delivery can add a writer for those hosts if a person needs one.
- **The cask's `zap` does not name the file.** The file belongs to the user, and the Host CLI of the formula reads it too.
- **Every command, flag, environment variable, exit code and `--json` field keeps its meaning.** `doctor`'s `config` verdict `absent` and the status field `configuration.state` `absent` keep their meaning. On a macOS host they now report `loaded` after the first start of the app. `absent` still appears before that start, after a failed write, and on Linux and Windows hosts.
- **The decision changes ADR-0021 and the spec, in place.** ADR-0021 states that the file is optional and says nothing about who writes it. The rework adds the writer, the rejected writers and the consequences. The spec's "Configuration" section, its requirement "One configuration file" and its test seams change with it.
- **The test seams are existing ones.** A unit test in the shared evaluator crate checks that the text evaluates to no setting. It also deletes the `//` of each example and checks that the text still evaluates. A third check finds each key of the evaluator in the text. A unit test of the app drives the write step in a temporary directory. The step writes when no entry is there, and it never replaces one. The existing test that a test-mode run reads no file of the user stays green. The live check is the first start of the new app on this Mac, which has no file today.
- **I published no browser presentation.** This session runs alone, and the ending command stops any server that it starts. The spec's Mermaid diagram of the configuration is the visual record, and it gains the write step. `app-all-guest-traffic-k42` made the same choice.
- **The spec states the two properties of the write, not its mechanism.** The properties are that the app never replaces an entry and that a person never sees a partial file. A temporary file and a hard link give both. But that mechanism fails on a file system with no hard links. So the mechanism goes to the planning leaf as a suggestion.
- **The app never writes a file that `TESTANYWARE_CONFIG` names.** `scripts/app-bundle-live.sh` opens the bundle with the variable in the app's environment, and the app's code reads it there. The spec and ADR-0021 said that only the Host CLI reads the variable. Both now say "in normal use", and the spec names the live check.
- **The live check of the bundle already accepts the written file.** Its first open reads the user's own file and accepts `absent` or `loaded`. On a Mac with no file, that open now writes the file and reports `loaded`.
- **I spent no in-session reviewer and cut no `review-design` leaf.** The change adds one bounded behaviour to the landed spec, and measurements on this Mac support its content. I checked the main risk myself. The app's tests run in test mode, which never writes. The CLI's tests name their file through `TESTANYWARE_CONFIG`, and the CLI never writes. The live check of the bundle accepts `loaded`. A review of the implementation can still read the write step.
- **The follow-on is a `planning` leaf, `default-config-file-increments-k59`.** A design session does not cut implementation leaves. Planning also decides when the change ships, and whether it shares a release with `linux-golden-disk-size-k57`. The root brief lists this delivery as item 6.
