# golden-directory-check-k34

## Goal

On a macOS host, make `vm create-golden` compare its directories with the directories of the app before it starts.

## Context

- `docs/specs/testanyware-app.md`, section "Golden creation keeps its direct path", and the scenario "Golden creation with directories that differ".
- `vm-broker-design-k6`, finding F4.

## Done when

- Golden creation resolves the directories that the app resolves: those of `config.rhai`, else the built-in defaults. It uses no flag and no variable for this.
- When its own directories differ, it fails with `APP_INCOMPATIBLE`, cause `directories`, before it starts. The error names both pairs and the remedy.
- The check needs no running app.
- On Linux and Windows hosts, golden creation does not run the check.
- Pure-function tests cover the match and the mismatch.

## Notes

- The spec lists `TART_HOME` as an open risk. Add no check for it.
