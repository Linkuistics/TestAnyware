# app-version-skew-k16

## Goal

Never let a version mismatch between the Host CLI and the running app pass silently.

## Context

- `docs/specs/testanyware-app.md`, section "Version skew", and the requirement "A version mismatch is never silent".
- `vm-broker-design-k6`, finding F5, which fixes the notice on stderr.
- `app-protocol-core-k9`, which already carries the ranges and the builds in headers.

## Done when

- A pure function takes the two ranges, the two builds and the build of the bundle on disk. It returns a decision: serve, serve with a notice, or fail with one of three remedies.
- When the ranges do not overlap, the command fails with `APP_INCOMPATIBLE`, cause `protocol`. The error names both builds and both ranges, and one remedy from the table of the spec.
- When the builds differ and the ranges overlap, the command proceeds. The CLI writes one notice line to stderr in both output modes. The line names both builds and the remedy.
- The notice changes neither stdout nor the JSON envelope. `--quiet` does not suppress it.
- The status response says whether the running app is older than its bundle on disk.
- Tests in the harness use a test mode that reports forged builds and ranges. They cover each row of the table.

## Notes

- A client never restarts the app. The remedies tell the person what to do.
- The report of this state in `doctor` comes in `app-diagnostics-k19`.
