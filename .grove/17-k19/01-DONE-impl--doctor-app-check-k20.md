# doctor-app-check-k20

## Goal

Add the `app` check to `doctor`. The `config` check and the rule that `doctor` runs with a file that fails already exist from `config-evaluator-cli-k18`.

## Context

- `docs/specs/testanyware-app.md`, section "What the commands report about the app".
- The doctor code of the CLI and its report schema in `docs/reference/cli-schemas`.
- `GET /v1/status` from `app-protocol-core-k9`, and the state that later leaves added to it.

## Done when

- On a macOS host, the `app` check reports whether the app is installed, with its path and version. It reports whether the app runs, with its build and protocol range.
- It reports the status of the login item and the observation of the Local Network grant. It reports the configuration and each VM that the app did not adopt. It reports whether the directories match.
- It reports whether the running app is older than its bundle on disk.
- On a Linux or Windows host, the check reports that it does not apply.
- A directory mismatch and a protocol mismatch are each a failing check, not an error envelope. `doctor` exits with 1 as today.
- Each failing item carries a remedy. The `--json` report keeps its existing fields and the schema documents the new ones.
- Tests in the harness cover a directory mismatch and a protocol mismatch. They also cover a failing file, where the `config` check fails and the `app` check still reports the running app.

## Notes

- The status request of `doctor` changes nothing in the app.
- `app-guest-streams-k23` keeps the Local Network observation in `AppContext::grant`, an enum with `NotYetSeen`, `Allowed` and `Denied`. The last two carry the destination and the time. `GET /v1/status` does not report it yet. Add a field to `StatusResponse` (`testanyware-app-protocol/src/status.rs`) and fill it from `AppContext`. Older readers ignore unknown fields.
- `app-default-routing-k26` decides whether the app is installed. `app_route::bundle_search_paths(home)` in the CLI lists `/Applications/TestAnyware.app` and `~/Applications/TestAnyware.app`. The remedy of a missing app is the constant `SETUP_REMEDY` of `testanyware-app-protocol`. Reuse both, so that `doctor` and the commands agree on what installed means.
- `app-version-skew-k16` added `bundle_build` and `older_than_bundle` to `StatusResponse`, and the header `X-Testanyware-Bundle-Build` to every response. `doctor` reports both from the status. Use `testanyware_app_protocol::decide` with the status's two builds and ranges to word the remedy of a protocol mismatch, so that `doctor` and the commands give the same remedy. `Remedy::text` is the sentence, and `Remedy::word` is the stable word.

## Decisions (running log)

- **The status gains three fields.** `executable`, `local_network` and `login_item` join `StatusResponse`. Each has a serde default, so an older body still reads. `local_network` holds `state`, `destination` and `observed_at`. `login_item` is `enabled`, `not_registered`, `requires_approval`, `not_found` or `unobserved`. The app fills them in `AppContext::status()`, which `GET /v1/status` now calls. The route no longer builds the body itself.
- **The login item is a probe on `AppContext`.** The field `login_item` is a `fn() -> LoginItemState`. A `fn` pointer is `Debug`, so `AppContext` keeps its derive. The test mode keeps the default and states `unobserved`. The menu bar app sets `shell::login_item_state`. `SMAppService` is now read on a thread of the server and not on the main thread. Apple documents no main-thread requirement ( https://developer.apple.com/documentation/servicemanagement/smappservice ), and the comment in `shell.rs` cites it. I verified it live. `task app:bundle-live` reads the status twice from the signed development bundle. It got `not_registered` both times, with no crash and no hang. The live script also proves that `doctor` reports the same state.
- **One reader of the bundle build.** `plist_build` moved from the app to `testanyware-app-protocol::bundle`. `doctor` reads the installed bundle with it, and the app reads its own. "Installed" means one thing for `doctor` and for the commands: `bundle_search_paths` and `SETUP_REMEDY`, as the leaf directed. `app_route::find_installed` is the pure function. `app_route::status` reads `GET /v1/status` with a client that has no response observer, so `doctor` writes no skew notice to stderr. The report states the skew.
- **The Local Network remedy has one owner.** `LOCAL_NETWORK_REMEDY` is a constant of the protocol crate. The app's stream error and `doctor` both use it. The copy in `streams.rs` is gone. The copy in `testanyware-vm` stays: that crate does not depend on the protocol crate, and it words a `vm start` error, which this leaf does not touch.
- **The check is a module of its own.** `commands/doctor_app.rs` holds `AppCheck`. `classify(AppProbe)` is pure, and `gather(macos_host)` is the runtime layer. `doctor.rs` only holds the result, adds it to `is_ok`, the JSON and the text, and `run_doctor` is now `async`. A Linux or Windows host gets `NotApplicable` whether or not `TESTANYWARE_APP_SOCKET` names a socket. The spec says the check does not apply there.
- **Which item fails and which warns.** A `fail` flips `ok` and exit 1. A `warn` does not. These fail: no bundle with a silent app, a silent app, a login item that is off or waits for approval, a denied grant, a failed configuration, other directories, and ranges with no common version. The brief names the directory mismatch and the protocol mismatch. The login item and the denied grant follow from "reports whether the app is ... allowed and set to start at login, each with a remedy". These warn: a bundle outside the search paths while the app answers (a development bundle works), a grant not yet seen, a file that changed since the evaluation, VMs not adopted, adoption not finished, builds that differ with a shared range, and an app older than its bundle. A VM that was not adopted keeps running and may be a leftover record, so it must not make `doctor` exit 1. I note this as the one judgement in the leaf. A reader who wants it to fail changes one status in `adoption_item`.
- **A silent app skips the rest.** `installed` and `running` report. Every other item is `skip` with the verdict `app_not_running`. The remedy of `running` is the error's own, and it is left out when `installed` already carries `SETUP_REMEDY`. The text report prints one remedy once, even when two items share it.
- **The remedies come from the code that owns them.** The directory mismatch uses `AppError::incompatible_directories` with the app's configuration status. The protocol item uses `decide` and `Remedy::text`/`word`, as the leaf directed. The JSON item carries `remedy_word` for a reader that does not parse the sentence. Every item carries a `summary` sentence in the JSON too.
- **The schema.** `doctor.json` has a new required group `app` with one definition for each item. Each has its own status and verdict enums and `additionalProperties: false`. The unit test `every_report_fits_the_doctor_schema` generates reports for every state and checks every key, status and verdict against the schema file that the binary embeds.
- **The tests.** Unit tests in `doctor_app.rs` cover every item and the other host. `tests/doctor-app.rs` runs the real CLI against the real app executable in test mode. It covers a directory mismatch in JSON and in text, a protocol mismatch in three rows of the table, a bundle newer than the app, a file that fails (the `config` check fails and the `app` check still reports the running app), a silent app, and an unchanged app after `doctor`. A Linux or Windows host runs the not-applicable case. A test of the app pins the new status fields. A mutation check: I made the directories always match, and a unit test and two harness tests failed. I made the protocol failure a warning, and a unit test and two harness tests failed. I restored both.
- **Not done, and left to `llm-instructions-and-docs-k35`.** The `llm-instructions`, `docs/reference/cli-commands.md` and `docs/components/cli.md` do not describe the `app` check yet. The `--help` of `doctor` does. I added the spec paragraphs for the status fields and for the failing and warning items to `docs/specs/testanyware-app.md`. The ADRs and the glossary needed no change.
