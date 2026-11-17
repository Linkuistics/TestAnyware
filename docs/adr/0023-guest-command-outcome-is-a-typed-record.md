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

Every change is additive. The legacy fields keep their meaning, so an old Host
CLI works with a new agent. A new Host CLI reads an agent reply without
`outcome` as a legacy record, with the outcome `unknown`. The Host CLI rejects a
contradictory record with `EXEC_RECORD_INVALID`. No protocol version field is
added.

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
  base64 bytes. Output size stays unbounded, as today.
- Windows keeps the legacy shape. A new Host CLI reports its records as legacy.
- The macOS exec types move into the vendored protocol module, under the
  encoding drift test. The Linux agent gains a model.
- Text-mode output and exit codes follow the record. The spec gives the rules.
