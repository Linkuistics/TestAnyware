# 23. A guest command's outcome is a typed record, separate from transport status

Date: 2026-11-18

## Status

Accepted. Recorded in grove `TestAnyware.gh-issue-14`, leaf
`lifecycle-evidence-k8`. The area's spec is
`docs/specs/guest-command-lifecycle.md`.

## Context

The `/exec` reply has four fields: `exitCode`, `stdout`, `stderr` and an
optional `timedOut`. The investigations of issue #14 found that this shape
cannot carry the facts that a tutorial verifier needs:

- A deadline arrives as exit -1. The Linux agent also sets `timedOut` to false
  on a deadline. Host CLI text mode then exits 255, which a guest can also
  return.
- A signal death arrives as an exit code: 9 on macOS and -9 on Linux.
- A spawn error arrives as an HTTP 500 with an empty body on macOS, and as
  exit -1 with stderr text on Linux.
- Both agents remove trailing whitespace. The macOS agent replaces non-UTF-8
  output with an empty string. The Linux agent refuses it.
- The macOS agent appends a timeout note to the guest stderr.

The evidence is in the k12 and k5 reports. The Host CLI help and
`cli-design-contract.md` also name a `details.exit_code` field that does not
exist.

Many projects drive TestAnyware through the CLI only. A change to an existing
`--json` field or exit code breaks them. Old agents stay in existing goldens,
and no golden is replaced.

## Decision

The agent reports each guest command as an exec record with a typed terminal
outcome. The five kinds are `exited` with a code, `signalled` with a signal,
`timed_out`, `cancelled` and `spawn_failed` with an errno. The record carries
the exact stdout and stderr bytes in base64, with their lengths and SHA-256
digests. It also carries the lifecycle times and the termination steps.

Guest status lives in the record. Host transport status lives in the Host CLI
envelope. In JSON mode, `ok` reports transport and record validity only, and a
guest outcome never sets it to false.

Every change is additive. A new agent sets the legacy fields to the values that
today's agent on the same platform reports for the same outcome and bytes. The
record names this legacy projection, `macos` or `linux`. An old Host CLI
therefore sees today's values. The new Host CLI also keeps its existing JSON
fields, its text output and its exit codes. Only the added fields carry the
facts that the legacy values hide.

The spec lists three departures from today's values. A command that finished
is no longer reported as timed out. A macOS spawn error gives a record, not an
HTTP error. A cancellation is a new outcome.

A new Host CLI reads an agent reply without `outcome` as a legacy record, with
the outcome `unknown`. It rejects a contradictory record with `EXEC_RECORD_INVALID`. No protocol version field is
added, and `schema_version` stays `1.0`.

## Considered options

- **Typed record with legacy fields, additive (chosen).** Every supported pair
  of Host CLI and agent keeps working. The cost is a reply that carries the
  output twice.
- **Replace the reply shape in a coordinated cutover, as in ADR-0001.**
  Rejected. ADR-0001 assumed no external protocol consumers. The CLI consumers
  read `--json` fields, and old agents stay in existing goldens.
- **Version negotiation with a capability field.** Rejected for now. The
  presence of `outcome` already tells a new Host CLI which agent it has. A
  version field adds a second source of truth that can disagree.
- **Keep the shape and fix the values only.** Rejected. An integer cannot tell a
  timeout from exit 255, or a signal from an exit code. A reserved range does
  not help, because a guest can also produce it.

## Consequences

- A tutorial verifier can tell exit 23 from a timeout, a cancellation, a signal
  and a spawn error, and can compare exact bytes.
- A reply roughly doubles in size, because it carries the legacy text and the
  base64 bytes. Output size stays unbounded, as today. Reading the capture has
  a deadline, and a capture that misses it arrives as an incomplete prefix.
- Each agent keeps code that reproduces its old legacy values, including the
  signal number on macOS and the decoder error on Linux. The validity rules
  check the legacy fields against that projection.
- Windows keeps the legacy shape. A new Host CLI reports its records as legacy.
- The macOS exec types move into the vendored protocol module, under the
  encoding drift test. The Linux agent gains a model.
- Text mode cannot separate a timeout from exit 255, as today, and it gives no
  exact bytes. A caller that needs the outcome or the bytes uses JSON mode.
