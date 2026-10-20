# doctor-app-check-k20

## Goal

Add the `app` and `config` checks to `doctor`.

## Context

- `docs/specs/testanyware-app.md`, section "What the commands report about the app".
- The doctor code of the CLI and its report schema in `docs/reference/cli-schemas`.
- `GET /v1/status` from `app-protocol-core-k9`, and the state that later leaves added to it.

## Done when

- On a macOS host, the `app` check reports whether the app is installed, with its path and version. It reports whether the app runs, with its build and protocol range.
- It reports the status of the login item and the observation of the Local Network grant. It reports the configuration and each VM that the app did not adopt. It reports whether the directories match.
- It reports whether the running app is older than its bundle on disk.
- On a Linux or Windows host, the check reports that it does not apply.
- The `config` check runs on every host and reports the evaluation of the CLI.
- A configuration error, a directory mismatch and a protocol mismatch are each a failing check, not an error envelope. `doctor` exits with 1 as today.
- `doctor` is the one command that runs with a failing `config.rhai`.
- Each failing item carries a remedy. The `--json` report keeps its existing fields and the schema documents the new ones.
- Tests in the harness cover a failing file, a directory mismatch and a protocol mismatch.

## Notes

- The status request of `doctor` changes nothing in the app.
