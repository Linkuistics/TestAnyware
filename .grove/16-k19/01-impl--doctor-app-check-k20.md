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
