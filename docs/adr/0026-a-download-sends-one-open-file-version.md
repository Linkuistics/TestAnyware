# 26. A download sends one open file version, and in-place overwrite is excluded

Date: 2026-11-18

## Status

Accepted. Recorded in grove `TestAnyware.gh-issue-14`, leaf
`lifecycle-evidence-k8`. The area's spec is
`docs/specs/guest-command-lifecycle.md`. It refines ADR-0001 and keeps its
bounded streaming.

## Context

Ravel-Strategies downloads status records that a guest process can change during
the read. The investigations of issue #14 found these facts:

- The macOS download looks up the path three times. The length comes from one
  lookup and the bytes from another. A replacement by rename can therefore give
  wrong bytes with a success receipt. On the host, this defect reproduced the
  bytes of the retained failure D (k7). Its cause in the guest is not proved.
- The Linux download opens the file once. It reads to end-of-file, not to its
  declared length, so growth sends bytes past the length (k14).
- An in-place overwrite during the read gave torn bytes on both agents. No
  framing check detected them (k7, k14).
- Publication to a fresh name gave a whole version in 200 of 200 reads on each
  agent (k7, k14).
- An interrupted Host CLI leaves its temporary file (k7, k14).

## Decision

The agent opens the file once. It takes the length L from that handle and sends
exactly L bytes from it. A short read closes the connection, so the body is
incomplete. The agent never sends a byte past L.

The supported contract has four rows. An unchanged file gives the exact bytes.
A rename replacement gives the version that was open at the start. An append
gives the first L bytes. A truncation gives a detected failure.

In-place overwrite is excluded. The stable alternative is fresh-name
publication: the writer writes each version once under a new name and never
changes it.

The Host CLI rejects a short body and bytes past the body. It writes to a
temporary file, flushes it and renames it. It removes the temporary file at
every failure and at SIGINT or SIGTERM.

## Considered options

- **One open handle and an exclusion with a stable alternative (chosen).** It
  removes the established macOS and Linux defects. Fresh-name publication has a
  demonstrated control on both agents.
- **Detect change with a modification-time check after the read.** Rejected. An
  append would then fail, and the time resolution can miss a fast overwrite.
- **Send a content digest.** Rejected. The agent computes the digest from the
  bytes it reads, so a torn read gets a matching digest.
- **Copy to a snapshot before the send.** Rejected. The copy reads the file in
  the same way, so it can also be torn.

## Consequences

- A rename replacement is in the contract only after the corrected macOS control
  demonstrates it.
- A writer that changes evidence in place must change to fresh-name
  publication. A reader then downloads each version by its name.
- A new Host CLI detects the extra bytes that an old Linux agent can send.
- SIGKILL of the Host CLI still leaves a temporary file. Its name never becomes
  the destination name.
