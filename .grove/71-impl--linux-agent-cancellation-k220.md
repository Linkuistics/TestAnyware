# linux-agent-cancellation-k220

## Goal

The Linux agent stays responsive while a command runs. It accepts explicit
cancellation, retains finished records and sends exact-length downloads. It
shuts down within bounds, and its accessibility endpoints stay serialised.

This leaf is the second part of candidate B. Leaf k217 owns the first part.

## Context

- Spec: sections "Agent shutdown", "Cancellation and CLI interruption",
  "Responsiveness", "Exec record", "Evidence transfer", "Bounds", "Test seams
  and required controls" and candidate B.
- ADR-0025 (cancellation is explicit and disconnect is not cancellation) and
  ADR-0026 (a download sends one open file version).
- Old failing paths: `diagnostic:linux-disconnect-container` and
  `diagnostic:linux-download-consistency-container`.
- Retired leaves 47 to 49 and 53 to 55 hold the Linux causal evidence.

## Done when

- The agent server serves each request on its own thread. A health request
  during a command replies within 1 s.
- All accessibility endpoints share one lock, because AT-SPI is not safe
  across threads. The exec, cancel, health, upload and download handlers never
  take that lock.
- `/exec/cancel` ends the command through the lifecycle owner of leaf k217. Its
  reply waits at most 5 s (`waitMs` 5000).
- Cancellation is idempotent. The exit-and-cancel race resolves by the spec's
  arbitration rule. The tombstone order holds.
- The agent retains each finished record for the retention time in the spec.
- A client disconnect does not cancel the command or change its record.
- A download sends exactly the declared length of one open file version.
- At SIGTERM, the agent meets the spec's "Agent shutdown" section. It stops
  admission, cancels every running command and waits only for bounded record
  publication. A leader whose status stays unavailable does not delay the
  exit. A VM stop does not replace this check.
- Each control below follows its row in the spec's required-control table.
  An old-fail control fails on its old path and passes on the new code. A
  new-behaviour control has no old failing path, and it passes on the new
  code. A compatibility control equals its recorded old reference. Each
  control runs in `agent:linux:test`:
  - Old-fail: the agent stays responsive during a command. The old path is
    `diagnostic:linux-disconnect-container`.
  - Corrected-pass: a health request and a cancellation reply within 1 s
    while another request holds the accessibility lock. Two concurrent
    accessibility requests never run inside AT-SPI together.
  - Corrected-pass: a disconnect changes nothing, and the agent retains the
    record. The old-fail case of this row is macOS.
  - New behaviour: cancellation, idempotence, the exit-and-cancel race and
    the tombstone order hold.
  - New behaviour: at SIGTERM during a command, the group ends and the agent
    exits within the spec's bounds. A second case holds a leader whose status
    stays unavailable.
  - Old-fail: a download sends no extra bytes. The old path is
    `diagnostic:linux-download-consistency-container`.
- When the change exists, insert a `review-impl` leaf directly after this
  leaf.

## Notes

The cancel types in the Rust protocol crate belong to leaf k222. Test the
endpoint here through raw HTTP in the agent tests. Do not replace a golden.

Review integration macos-agent-lifecycle-k234 found a Linux form of the macOS
finding R2. The request accepts any number as `timeout`. In
`LeaderWaiter.wait_exit`, `select.poll` raises `OverflowError` for a wait of
about 2147484 s or more. The host Python 3.12 showed this error. The source
shows the rest: the error comes after the spawn, the server replies HTTP 500,
and no owner ends the group.

The macOS owner now saturates its deadline and
waits at most 60 s in each kernel call. Bound each Linux poll the same way,
and add a control with a timeout of 20000000000 s.
