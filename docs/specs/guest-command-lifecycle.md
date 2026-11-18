# Guest command lifecycle and evidence transfer

This spec defines how an In-VM agent runs, ends and reports a *guest command*.
It also defines how the Host CLI transfers command and file evidence without
loss. It is the target contract for issue #14. The current behaviour is in
`docs/architecture/agent-protocol.md` and `docs/architecture/cli-design-contract.md`.
The implementation updates those documents together with the code.

The visual document is `docs/design/guest-command-lifecycle/index.html`. Its
sources are in the same directory. The decisions are in
[ADR-0023](../adr/0023-guest-command-outcome-is-a-typed-record.md),
[ADR-0024](../adr/0024-one-owner-ends-each-guest-command.md),
[ADR-0025](../adr/0025-cancellation-is-explicit-and-disconnect-is-not-cancellation.md) and
[ADR-0026](../adr/0026-a-download-sends-one-open-file-version.md).
The terms are in `CONTEXT.md`.

## Problem and scope

Ravel-Strategies verifies tutorials through `testanyware file exec` and
`testanyware file download`. Any unexpected timeout fails a verification.
The investigations of issue #14 found these failures in the current agents and
Host CLI:

- A deadline kills only the Linux shell, and a forked child survives it.
- The Linux agent serves one request at a time, so a running command blocks
  inspection, health and transfer until its deadline.
- Both agents remove trailing line breaks and lose non-UTF-8 bytes.
- Both agents report a signal, a deadline or a spawn error as an exit code.
- No agent ends a command when its client disconnects, and no client can end
  a command on purpose.
- A macOS download can mix two file versions and still report success.
- A Linux download can send bytes past its declared length.
- An interrupted download leaves a temporary file.
- The macOS agent sometimes reports a timeout for a command that finished.

This spec covers the `/exec` and `/download` endpoints, a new `/exec/cancel`
endpoint, the macOS and Linux agents, the shared protocol types, and the
`file exec`, `file cancel` and `file download` commands. It makes no reliability
claim for Windows. It keeps the Windows wire shape. It keeps the app routing of
ADR-0004.

Issue #10 owns durable jobs, leases, pooling and placement. This spec does not
add them. A retained exec record is evidence for a short time, not a job.

## Evidence basis

Each row gives one finding, its state and the consequence for this design. An
established finding has a retained failure or a deterministic reproducer. A
non-reproduced finding happened once and did not recur under a repeat. An
unavailable finding has no evidence route. An unresolved finding has evidence
but no proven cause.

| Finding | State | Source | Consequence |
|---|---|---|---|
| Linux deadline leaves a forked child alive | Established | k5 container, k204 clone | Signal the owned group, not the shell |
| Linux agent blocks every request during `/exec` | Established | k6 container and clone | Serve requests concurrently |
| A client deadline or disconnect does not free the Linux agent | Established | k6 | Do not depend on disconnect handling |
| macOS disconnect does not end the command, and the agent stays responsive | Established | k13 clone and loopback | Make cancellation explicit |
| macOS child survives CLI interruption | Established | k4, k193, three samples | Give the CLI a cancellation path |
| Both agents trim output and drop non-UTF-8 bytes | Established | k12, k5 | Send exact bytes |
| Signal, deadline and spawn error arrive as exit codes | Established | k12, k5 | Send a typed outcome |
| macOS spawn error arrives as HTTP 500 after about 3 s | Established | k12 | Report errno as an outcome |
| macOS download takes three path lookups and can send undetected wrong bytes | Established on the host | k7 loopback | Read one open handle |
| The k7 defect reproduces the failure-D bytes | Established on the host | k7 digest match | Cover the failure-D case in controls |
| The k7 defect caused failure D in the guest | Unresolved | k7 | Do not claim the guest cause |
| Linux download sends bytes past its length | Established | k14 container | Send exactly the declared length |
| In-place overwrite gives torn bytes on both agents | Established | k7, k14 | Exclude it, and offer fresh-name publication |
| Fresh-name publication gives whole versions | Established, 200 of 200 per agent | k7, k14 | Make it the stable alternative |
| An interrupted download leaves a temporary file | Established | k7, k14 | Remove it on SIGINT and SIGTERM |
| macOS host suite reports finished commands as timed out | Established symptom, cause unresolved | k3 host suite, 7 of 20 | Require the suite green with the new runner |
| The old runner gives that receipt when the dispatch pool has no free worker | Established in a host probe only | k3 `free-1` probe, 10 of 10 | Remove both candidate mechanisms |
| Which phase of the old runner stalls in that receipt | Unresolved, because the single-hop control changes two things | k3 | Claim neither mechanism |
| Pool starvation caused macOS operation 08 | Unresolved, not reproduced in the guest | k3, k186, k191 | Make recurrence visible |
| macOS post-timeout inspection stalls | Not reproduced | k4, k193, k195 | Make recurrence visible |
| The stall and operation 08 share a cause | Unresolved | k4 | Keep the two symptoms separate |
| CLI SIGTERM changes the Linux handler end on a clone | Unresolved | k6 | Do not depend on it |
| Run 42 observation gap | Unresolved | k17 | No design input |
| Windows reliability | Unavailable | none | Make no claim |
| Help and contract name a `details.exit_code` field that does not exist | Established | `file exec` help, contract §5 | Correct both documents |

No design choice depends on an unresolved cause. The macOS runner in this spec
removes both candidate mechanisms of the receipt and of operation 08. Those
mechanisms are a delayed second wait block in the shared pool and a missed
Foundation exit notification. The k3 probe does not separate them, and the
receipt does not show the child exit or either wait block.

The exec record shows which event ended each command and how late the owner
was. If either macOS symptom recurs, the record names it, and the trial fails.
A clean run does not correct either symptom.

## Guest command lifecycle

A guest command passes through the states below. The state diagram is
`state-lifecycle` in the visual document.

| State | Meaning |
|---|---|
| Accepted | The agent read a valid request and registered its exec id. |
| Running | Spawn returned a PID. The deadline clock runs. |
| Terminating | The owner chose a timeout or a cancellation and is ending the group. |
| Terminal | The record is final. The agent retains it for a bounded time. |
| Tombstone | A cancellation named an id that the agent did not hold. |

### Start

1. Check the request and the exec id.
2. Register the exec id. A duplicate id of a running or retained command returns
   HTTP 409 with the error token `exec_id_in_use`.
3. Answer a tombstoned id with a cancelled record, and do not spawn.
4. Create a capture directory that only the agent owns.
5. Spawn `/bin/bash -c COMMAND` as the leader of a new session.
6. Connect stdin to `/dev/null`.
7. Connect stdout and stderr to two capture files in that directory.

On macOS, the spawn uses `posix_spawn` with `POSIX_SPAWN_SETSID`. On Linux, it
uses `subprocess.Popen` with `start_new_session=True` and opens a pidfd for the
leader. The deadline starts when spawn returns a PID. A spawn error ends the
command with the outcome `spawn_failed` and the errno. The agent reports it in
a normal HTTP 200 record, not as an HTTP error.

### Capture

The agent captures output to files, not pipes. A pipe reaches end-of-file only
when every descendant closes it. A background descendant can therefore hold a
pipe open for ever. A file has no end-of-file condition to wait for.

Finalisation has a deadline. After a termination, the deadline is 1.9 s after
SIGKILL. After any other outcome, it is 1.9 s after the outcome.

1. Note the size of each capture file as `fileBytes`.
2. Start one worker that reads each file from offset 0 up to `fileBytes`.
3. Let the worker count the bytes and compute their SHA-256 digest as it reads.
4. At the deadline, stop waiting for the worker.
5. Mark each stream that the worker finished as `complete`.
6. Mark each other stream as `incomplete`.

An incomplete stream carries the prefix that the worker read by the deadline,
with its own length and digest. Its reason is `deadline`. A read error also
gives `incomplete`, with the reason `read_error`. Then the agent publishes the
record and removes the capture directory. Bytes past the prefix are lost.

A stalled read can block the abandoned worker. It then holds one thread until
the read returns, and it changes nothing in the published record. The record
covers the bytes up to `fileBytes`. An escaped descendant can append later
bytes, and the record does not cover them.

### Arbitration

One owner handles all events of one guest command in order. The events are the
leader exit, the deadline and a cancellation. The first outcome that the owner
chooses is final.

1. At a leader exit event, reap the leader and choose `exited` or `signalled`.
2. At the deadline or a cancellation, make one non-blocking kernel wait on the
   leader first.
3. If that wait returns an exit, choose `exited` or `signalled`.
4. If the leader still runs, choose `timed_out` or `cancelled` and start
   termination.
5. After a terminal outcome, record each later event but change nothing.

Step 2 is the rule that makes a finished command impossible to report as
timed out. The kernel state takes priority over any lost or late notification.
A late owner can still see a leader that exited after the deadline. The outcome
is then `exited`, and the record carries the owner lateness. A cancellation
after a timeout returns the timed-out record. A cancellation after an exit
returns the exit record.

### Normal exit

The leader can exit while other members of its group still run, for example
after `server &`. The agent does not signal those members, because existing
tutorials can depend on them. The record states `remainingGroup` as `none`,
`present` or `unknown`. The agent finds it with one signal-0 probe of the group
after the reap.

### Termination after a timeout or a cancellation

T0 is the moment when the owner chooses `timed_out` or `cancelled`. The
sequence diagram is `seq-terminate` in the visual document.

K is the moment when the owner sends SIGKILL. Each wait below is an event wait
or a non-blocking check, never a blocking reap.

1. Send SIGTERM to the owned group at T0.
2. Wait for the leader exit until T0 + 1.9 s.
3. Send SIGKILL to the owned group at the leader exit or at T0 + 1.9 s.
4. Wait for the leader exit status until K + 1.4 s.
5. When the status arrives, reap the leader.
6. After the reap, probe the group with signal 0 until the probe returns ESRCH.
7. Stop the probe at K + 1.4 s.
8. Finalise the capture files by K + 1.9 s.
9. Publish the record.

The schedule keeps 0.1 s of margin under each root bound of 2 s. The margin
absorbs timer lateness. The bounds, not the schedule, decide `withinBounds`.

ESRCH gives `groupEmpty: yes`. A probe that still finds members at K + 1.4 s
gives `no`. EPERM gives `unknown`, because a member that changed its user
cannot be probed. A failed probe never proves absence.

The leader exit status can stay unavailable at K + 1.4 s, for example when the
leader is in uninterruptible sleep. The record then reports `leaderStatus`
`unavailable` and `groupEmpty: no`, because the leader is still a member. The
owner publishes that record on the same schedule. It keeps holding the leader
and reaps it when the kernel delivers the status. It sends no further signal,
and the published record does not change. The agent log records the late reap.

The termination block holds the step times from T0, the leader status,
`groupEmpty` and `withinBounds`. The section [Termination fields](#termination-fields)
defines them.

### Process ownership and identity

The agent owns the leader and every process that stays in the leader's group.
It sends SIGTERM and SIGKILL to the group only while the leader is unreaped.
An unreaped zombie keeps its PID. XNU and Linux do not give a new process an
ID that an existing process group uses. The group ID therefore names only the
owned group while the agent holds the leader. After the reap, the agent sends
only signal-0 probes, which cannot harm an unrelated process.

The owner checks the leader without a reap, for example with `waitid` and
`WNOWAIT`. Only the owner reaps the leader. No other code in the agent reaps
with a wildcard wait, because that wait could take the owner's leader.

This identity check needs no PID and start-time comparison. The record still
reports the leader PID, the group ID and the leader start time for independent
checks.

### Limits

- An escaped descendant leaves the owned group. The agent does not signal,
  wait for or report it. It can keep a capture file open and write to it.
- The grace period runs on the leader. A member that handles SIGTERM slowly can
  receive SIGKILL before 1.9 s, when the leader exits first.
- A member in uninterruptible sleep ends only when the kernel lets it. The
  record then reports `groupEmpty: no` and `withinBounds: false`. When that
  member is the leader, the record also reports `leaderStatus` `unavailable`.
- A capture that the worker cannot read by the deadline arrives as an
  incomplete prefix. A stalled read holds one agent thread until it returns.
- The finalisation deadline bounds the capture read only. The agent encodes and
  publishes the prefix after that deadline, in a time that grows with the
  prefix size. This spec sets no output limit. A large prefix can therefore make
  `finalisedAfterMs` exceed its bound, and the record reports that time. The
  record cannot measure its own serialisation, so the controller measurement
  stays the authority for each phase.
- An agent crash leaves its owned groups running. The agent removes old capture
  directories when it starts.
- A detached command (`detach: true`) has no owner, no record and no
  cancellation. Its behaviour does not change.

### Agent shutdown

When the agent receives SIGTERM, it stops accepting requests. It cancels every
running guest command and waits until each record is published, within the
bounds above. A leader whose status stays unavailable does not delay the exit.
Then the agent exits, and `init` or `launchd` reaps any leader that it
still holds. A VM stop or a controller disposal does not replace this owner.

## Cancellation and CLI interruption

Cancellation and CLI interruption are separate contracts. The sequence diagram
is `seq-cancel-interrupt` in the visual document.

### Exec id

The client chooses the exec id. It has 1 to 64 characters from `A-Z`, `a-z`,
`0-9`, `.`, `_` and `-`. The Host CLI sends a random UUID unless the caller
gives `--exec-id`. A caller who wants a fallback after SIGKILL gives its own id.
A request without an id, for example from an old Host CLI, gets an id from the
agent. That command cannot be cancelled, because no client knows its id.

### Cancel endpoint

`POST /exec/cancel` takes `{"id": ID, "waitMs": N}`. `waitMs` is 0 to 10000 and
defaults to 0. The reply is HTTP 200 with a state.

| State | Meaning | Record |
|---|---|---|
| `terminating` | The command is ending, and the wait ended first. | absent |
| `terminal` | The command has a final record. | present |
| `unknown_id` | The agent held no such id. It now holds a tombstone. | absent |

A cancellation is idempotent. A repeat returns the current state and never
changes a terminal outcome. A waiting cancellation returns only the record of
the command that it cancelled. Retention can evict that record and a new
command can take the id during the wait. The reply then still carries the
cancelled command's record.

A tombstone makes a later `/exec` with that id
return `cancelled` with `beforeStart: true`, without a spawn. The tombstone
covers a cancellation that overtakes its own `/exec` on another connection.
`unknown_id` never proves that no command ran.

### Retention

The agent retains each terminal record and each tombstone for 300 s. It retains
64 of them at most and evicts the oldest first. It never evicts a running
command. It removes the capture directory together with the record. An agent
restart loses every record.

### Connection loss

The agent does not watch the client connection. A close, a reset or a
half-close does not change the guest command. The command reaches its outcome,
and the agent retains the record. A client deadline does not free anything in
the guest. The evidence for this policy is in k6 and k13. A relayed half-close
is also a normal HTTP event, so disconnect-as-cancel could end a healthy
command.

### CLI interruption

When `file exec` receives SIGINT or SIGTERM, the Host CLI does these steps:

1. Open a new guest connection on the normal route.
2. Send one cancellation for its own exec id with `waitMs: 0`.
3. Wait for the reply, for 1.5 s at most.
4. Restore the default action of the signal.
5. Raise the same signal again.

The parent process therefore sees the same signal death as today. SIGKILL
cannot be handled. Its fallback is `file cancel --exec-id ID`, or the guest
deadline. An old agent answers the cancellation with HTTP 404. The Host CLI
then raises the signal again without a cancellation.

## Responsiveness

An agent serves independent requests while a guest command runs. A health,
observation, cancellation or transfer request does not wait for that command.

On Linux, a threading HTTP server replaces `HTTPServer`. The accessibility
endpoints share one lock, because AT-SPI is not safe across threads. The exec,
cancel, health, upload and download handlers never take that lock.

On macOS, the owner waits on kqueue process and timer sources on its own serial
queue. It holds no worker of the shared dispatch pool while it waits. No wait
in either agent depends on a second block in a shared pool.

## Exec record

The exec record is the `/exec` reply and the `record` of a cancellation reply.
Guest status lives in the record. Host transport status lives in the Host CLI
envelope. The agent wire uses camelCase.

### Legacy fields

The legacy fields are `exitCode`, `stdout`, `stderr` and `timedOut`. A new
agent sets them to the values that today's agent on the same platform reports
for the same outcome and bytes. This is the *legacy projection*. An old Host
CLI therefore sees today's values from a new agent. The added field
`legacyProjection` names the projection, `macos` or `linux`. The added fields
carry the corrected facts.

The macOS projection:

| Outcome | `exitCode` | `stdout` and `stderr` | `timedOut` |
|---|---|---|---|
| `exited` | The exit code | Decoded | false |
| `signalled` | The signal number, as Foundation `terminationStatus` gives it | Decoded | false |
| `timed_out` | -1 | Decoded, and `stderr` gains the note | true |
| `cancelled` | -1 | Decoded, and `stderr` gains the note | false |
| `spawn_failed` | -1 | `stdout` empty, `stderr` the message | false |

The macOS agent decodes each stream as strict UTF-8. A stream that is not
valid UTF-8 becomes empty. Then it removes trailing whitespace. The note is
`Process timed out after Ns` or `Process cancelled`. When `stderr` is empty,
the note replaces it. Otherwise `stderr` gains a line break and the note in
square brackets.

The Linux projection:

| Outcome | `exitCode` | `stdout` and `stderr` | `timedOut` |
|---|---|---|---|
| `exited` | The exit code | Decoded | false |
| `signalled` | Minus the signal number, as `returncode` gives it | Decoded | false |
| `timed_out` | -1 | `stdout` empty, `stderr` `Process timed out` | absent |
| `cancelled` | -1 | `stdout` empty, `stderr` `Process cancelled` | absent |
| `spawn_failed` | -1 | `stdout` empty, `stderr` the message | absent |

The Linux agent decodes each stream as strict UTF-8 and removes trailing
whitespace. When either stream is not valid UTF-8, the projection is today's
error reply. `exitCode` is -1, `stdout` is empty and `stderr` is the decoder
message as Python formats it.

The projection departs from today's values in three cases:

- A command that exits before its deadline is `exited`. Today's macOS agent
  sometimes reports it as timed out (k3).
- A macOS spawn error gives an HTTP 200 record with `exitCode` -1. Today it
  gives an HTTP error reply.
- A cancellation is new. Its projection is the `cancelled` row above.

An incomplete capture projects the bytes of its prefix. The added fields never
carry a note or a decoder message.

### Added fields

```json
{
  "id": "6f1c0d6e-4b0e-4f7a-9d55-1d2a3b4c5d6e",
  "outcome": {"kind": "exited", "exitCode": 23},
  "evidence": {
    "stdout": {"base64": "RXhwbGFpbiB0aGlzIGNoYW5nZS4=", "bytes": 20, "fileBytes": 20, "sha256": "…", "capture": "complete"},
    "stderr": {"base64": "c2VwYXJhdGUgc3RkZXJyCg==", "bytes": 16, "fileBytes": 16, "sha256": "…", "capture": "complete"}
  },
  "lifecycle": {"leaderPid": 812, "groupId": 812, "leaderStartTime": "…", "spawnMs": 2, "durationMs": 41, "ownerLatenessMs": 0, "finalisedAfterMs": 3},
  "remainingGroup": "none",
  "termination": null,
  "legacyProjection": "linux"
}
```

The `lifecycle` fields:

| Field | Meaning |
|---|---|
| `leaderPid`, `groupId`, `leaderStartTime` | The identity of the leader and the owned group. Null when no leader started. |
| `spawnMs` | The time from the request to the spawn result. |
| `durationMs` | The time from the spawn to the outcome. Null when no leader started. |
| `ownerLatenessMs` | The time from the deadline or the cancellation event to the owner action. 0 for an outcome without such an event. |
| `finalisedAfterMs` | The time from the outcome to the published record. |

Each `evidence` stream holds `base64`, `bytes`, `fileBytes`, `sha256` and
`capture`. `fileBytes` is the file size at finalisation. `capture` is
`complete` or `incomplete`. An incomplete stream also holds `reason`,
`deadline` or `read_error`.

The outcome kinds and their fields:

| Kind | Fields |
|---|---|
| `exited` | `exitCode`, 0 to 255 |
| `signalled` | `signal`, `signalName` |
| `timed_out` | `timeoutSeconds` |
| `cancelled` | `beforeStart` |
| `spawn_failed` | `errno`, `errorName`, `message` |

`remainingGroup` is present exactly for `exited` and `signalled`.

### Termination fields

`termination` is present exactly for a `timed_out` or `cancelled` command that
started. Its times are milliseconds from T0, and T0 is the outcome.

| Field | Meaning |
|---|---|
| `termSentAfterMs` | When the owner sent SIGTERM to the group. |
| `killSentAfterMs` | When the owner sent SIGKILL to the group. |
| `leaderStatus` | `{"kind": "exited", "exitCode": n}`, `{"kind": "signalled", "signal": n}` or `{"kind": "unavailable"}`. |
| `leaderExitAfterMs` | When the leader exit status arrived. Null when `leaderStatus` is `unavailable`. |
| `groupEmpty` | `yes`, `no` or `unknown`. |
| `groupEmptyAfterMs` | When the probe returned ESRCH. Null unless `groupEmpty` is `yes`. |
| `withinBounds` | The derived value below. |

The leader can exit before SIGKILL or after it. A leader that ends on SIGTERM
exits before SIGKILL. A leader that ignores SIGTERM exits after SIGKILL. Both
orders are valid, so the rules set no order between `leaderExitAfterMs` and
`killSentAfterMs`.

`withinBounds` is true exactly when all three conditions hold:

- `groupEmpty` is `yes`.
- `killSentAfterMs` is 2000 at most, the root grace bound.
- `lifecycle.finalisedAfterMs` minus `killSentAfterMs` is 2000 at most, the
  root bound for kill, drain and reap.

### Validity rules

The Host CLI accepts a record only when every rule below holds:

- `id` equals the id that the Host CLI sent.
- `legacyProjection` is `macos` or `linux`.
- `exitCode`, `timedOut`, `stdout` and `stderr` equal the named projection of
  `outcome` and of the evidence bytes. The Host CLI does not compare the text
  of a Linux decoder message.
- Each `base64` decodes to `bytes` bytes, and the digest of those bytes equals
  `sha256`.
- `bytes` equals `fileBytes` for a complete stream and is less than
  `fileBytes` for an incomplete stream.
- `termination` and `remainingGroup` are present exactly as stated above.
- Every time is a non-negative integer.

A termination block is valid only when every rule below holds:

- `termSentAfterMs` ≤ `killSentAfterMs` ≤ `lifecycle.finalisedAfterMs`.
- `leaderExitAfterMs` is null exactly when `leaderStatus` is `unavailable`.
  Otherwise it is at least `termSentAfterMs` and at most
  `lifecycle.finalisedAfterMs`.
- `leaderStatus` `unavailable` requires `groupEmpty: no`.
- `groupEmptyAfterMs` is null exactly when `groupEmpty` is not `yes`.
  Otherwise it is at least `killSentAfterMs`, at least `leaderExitAfterMs` and
  at most `lifecycle.finalisedAfterMs`.
- `withinBounds` equals the derived value.

A record that breaks a rule is contradictory evidence. The Host CLI reports
the error `EXEC_RECORD_INVALID` and exits 1. It never reports a contradictory
record as a success. A record with `capture: incomplete` is valid but partial.
The Host CLI reports it unchanged.

## Host CLI semantics

### JSON mode

The envelope `ok` reports host transport and record validity only. A guest
outcome never sets `ok` to false. JSON mode exits 0 when `ok` is true. The
`file exec` envelope keeps `command`, `exit_code`, `stdout`, `stderr` and
`timed_out`. They copy the legacy fields of the agent record unchanged, as
today, and an absent `timedOut` gives false. The envelope adds these fields:

| Field | Content |
|---|---|
| `exec_id` | The exec id. |
| `record` | `agent` for a new agent, `legacy` for an agent without `outcome`. |
| `legacy_projection` | `macos` or `linux`, or null. |
| `outcome` | The outcome in snake_case, for example `{"kind": "exited", "exit_code": 23}`. |
| `evidence` | The two streams with `base64`, `bytes`, `file_bytes`, `sha256`, `capture` and `reason`. |
| `lifecycle` | The lifecycle block in snake_case, or null. |
| `termination` | The termination block in snake_case, or null. |
| `remaining_group` | `none`, `present` or `unknown`, or null. |

A legacy record gives `outcome: {"kind": "unknown", "reason": "legacy_agent"}`
and null for the other added fields except `exec_id` and `record`. A caller
never counts an `unknown` outcome as a success. `schema_version` stays `1.0`.
No existing field changes its value for a supported agent, apart from the three
departures that the legacy projection lists.

### Text mode

Text mode does not change. It prints the legacy `stdout` and `stderr` as today,
and it adds a line break to `stderr` when that field lacks one. When the legacy
`exitCode` is not 0, the Host CLI exits with it, as today. The process exit
status therefore takes it modulo 256, so -9 gives 247 and -1 gives 255.

Text mode cannot separate a timeout from a guest exit 255, or a signal from an
exit code, as today. It also cannot give the exact bytes. JSON mode is the
structured interface and gives both.

### Errors

| Code | Exit | Meaning |
|---|---|---|
| `EXEC_RECORD_INVALID` | 1 | The agent record breaks a validity rule. |
| `AGENT_ENDPOINT_UNSUPPORTED` | 1 | The agent does not have the endpoint, for example `/exec/cancel` on an old agent. |
| `DOWNLOAD_FRAMING_INVALID` | 1 | The download body has bytes past its declared length, or the reply declares contradictory lengths. |
| `CONNECTION_DROPPED` | as today | The body ended before its declared length. |
| `IO_ERROR` | as today | The Host CLI could not finish the local file. |

Each existing code keeps its meaning and its exit code.

### Commands and flags

- `file exec --exec-id ID` sets the exec id.
- `file cancel --vm VM --exec-id ID [--wait SECONDS] [--json]` sends one
  cancellation. `--wait` is 0 to 10 and defaults to 0. Its schema is
  `file-cancel.json`. The command is idempotent and safe to repeat.
- `file cancel` reports the state, and the record when the state is `terminal`.

### Help, schemas and documents

The implementation changes these together with the code:

- `file exec` help and the `run_exec` doc comment name the top-level
  `exit_code`, not `details.exit_code`.
- `cli-design-contract.md` §5 states the JSON and text rules of this spec.
- `file-exec.json` adds the new fields, `lifecycle` and `legacy_projection`
  among them. `file-cancel.json` is new.
- `file exec` help states that text mode and the existing JSON fields keep
  today's values. It also states that the added JSON fields give the outcome
  and the exact bytes.
- `agent-protocol.md` adds the record, `/exec/cancel` and the download rules.
- The command surface registers `file cancel` as mutating and idempotent.

## Evidence transfer

### Agent download contract

1. Open the file once.
2. Take the length L from `fstat` of that handle.
3. Send `Content-Length: L`.
4. Send exactly L bytes from that handle.
5. If the handle ends before L bytes, close the connection.

The agent never sends a byte past L. It does not compare modification times
after the read, because an append would then fail. It keeps a constant-size buffer, so
ADR-0001's bounded streaming stays true.

### Consistency contract

| Writer action during the read | Result |
|---|---|
| None | The exact bytes |
| Replacement by rename | The whole version that was open at the start |
| Append | The first L bytes as they were at the open |
| Truncation | A detected failure, `CONNECTION_DROPPED` |
| In-place overwrite | Excluded. The bytes can be torn, and the Host CLI can miss it. |

The stable alternative for changing evidence is fresh-name publication. The
writer writes each version once under a new name and never changes it. A reader
downloads that name.

The k7 and k14 controls gave 200 of 200 whole versions on
each agent. A rename replacement is in the contract only after the corrected
macOS control demonstrates it. The agent sends no digest. A digest computed
during the read cannot detect a torn read.

### Host CLI download

1. Write the body to a temporary file in the destination directory.
2. Fail with `CONNECTION_DROPPED` when the body is shorter than its length.
3. Fail with `DOWNLOAD_FRAMING_INVALID` when bytes follow the body on a
   connection that the agent closes.
4. Flush the temporary file to disk.
5. Rename it to the destination.
6. Report the verified byte count as `bytes`.

Each failure removes the temporary file and leaves the destination unchanged.
A local finalisation error reports `IO_ERROR`. At SIGINT or SIGTERM, the Host
CLI removes the temporary file and raises the signal again. SIGKILL leaves the
temporary file, and its name never becomes the destination name.

Step 3 also protects a new Host CLI against an old Linux agent.

## Compatibility and version skew

The policy is additive. Every change adds a field, an endpoint or a command.
Each existing field, text output and exit code keeps today's value for each
supported agent. The legacy projection lists the three departures. No version
negotiation is added, and `schema_version` stays `1.0`.

A new Host CLI detects an old agent from a reply without `outcome` and from
HTTP 404 on `/exec/cancel`. Existing goldens keep their old agents, and no
golden is replaced.

| Host CLI | Agent | Result |
|---|---|---|
| New | New macOS or Linux | The full contract |
| New | Old macOS or Linux | Legacy record, `outcome` unknown, no cancellation, CLI interruption without cancellation, download checks for short bodies and extra bytes only |
| New | Windows | Legacy record, no claim |
| Old | New macOS or Linux | The legacy projection of the platform, so today's values, no cancellation |
| Old | Old | Unchanged |

Shared types change together:

- The Rust protocol crate adds optional record fields and the cancel types.
- The macOS exec types move into the vendored `TestAnywareAgentProtocol` module,
  under the encoding drift test.
- The Linux `models.py` adds an exec record model.
- The Windows types do not change.

## Bounds

The root brief sets the first group. This spec proposes the second group.
Preparation freezes all of them before any measurement. Each bound has an
observation. A record field observes it in the agent, or the outer controller
measures it.

| Phase | Bound | Source | Observation |
|---|---|---|---|
| Termination grace | 2 s at most | Root | `termination.killSentAfterMs` |
| SIGKILL, drain and reap | 2 s at most | Root | `lifecycle.finalisedAfterMs` minus `termination.killSentAfterMs` |
| Ordinary observation | 30 s | Root | Controller |
| Intentional guest timeout | 2 s | Root | `outcome.timeoutSeconds` and `lifecycle.durationMs` |
| Raw host exec | Guest budget + 20 s | Root | Controller |
| Transfer | 150 s | Root | Controller |
| CLI SIGTERM, then SIGKILL | 2 s, then 2 s | Root | Controller |
| Spawn | 1 s | This spec | `lifecycle.spawnMs` |
| Owner lateness at a deadline | 0.5 s | This spec | `lifecycle.ownerLatenessMs` |
| Outcome to published record, without termination | 2 s | This spec | `lifecycle.finalisedAfterMs` |
| Outcome to published record, with termination | 4 s | This spec | `lifecycle.finalisedAfterMs` |
| Cancellation reply with `waitMs` 5000 | 5 s | This spec | Controller |
| CLI SIGTERM to signal death | 2 s | This spec | Controller |
| Health during a running command | 1 s | This spec | Controller |

The agent clocks cover only the agent. The controller measures each phase from
the host, and an agent field never replaces that measurement. Preparation also
declares the boot, slot and export bounds.

## Test seams and required controls

Each guarantee has a control at the highest seam that shows it. An old-fail
control runs the old code and must fail. A corrected-pass control runs the new
code and must pass. The seams are the three that the owner agreed on
2026-11-01. A Linux agent test task does not exist yet, so the planner adds one.

| Guarantee | Seam | Old-fail control | Corrected-pass control |
|---|---|---|---|
| Group ends after a timeout | Process and file | `diagnostic:linux-timeout-lifecycle-old-path` | Linux agent tests, fork-and-wait case |
| Grace and SIGKILL bounds | Process and file | `diagnostic:macos-exec-outcomes-old-path`, term-ignored case | Both agent suites, a cooperative leader and a SIGTERM-ignoring leader |
| Leader exit status unavailable at K + 1.4 s | Process and file | none, new behaviour | Both agent suites, with a leader whose status stays unavailable past the bound |
| Large or stalled capture | Process and file | none, new behaviour | Both agent suites, with a capture reader that stalls past the deadline |
| Exact bytes, exit 23, signal, spawn errno | Process and file, HTTP and Host CLI | `diagnostic:macos-exec-outcomes-old-path` | `diagnostic:macos-exec-outcomes`, Linux equivalent |
| No finished command reported as timed out | Process and file | `diagnostic:macos-completion-old-path` at `free-1` | Same task with `EXPECT=clean` on the new runner, 20 green `agent:macos:test` runs |
| Responsive agent during a command | Transport | `diagnostic:linux-disconnect-container` | Same cases on the threaded server |
| Disconnect changes nothing, record retained | Transport | `diagnostic:macos-disconnect-availability-host` | Same cases plus cancellation |
| Cancellation, idempotence, tombstone | Process and file, HTTP and Host CLI | none, new behaviour | Agent and Host CLI tests, exit-and-cancel race |
| CLI interruption sends a cancellation | HTTP and Host CLI | k13 CLI SIGTERM case | Host CLI test with a loopback agent |
| Download reads one version | Process and file | `diagnostic:download-consistency-local` with the shipped agent | Same task, corrected agent |
| Download never sends extra bytes | Process and file | `diagnostic:linux-download-consistency-container` | Same task, corrected agent |
| Host CLI rejects extra bytes and short bodies | HTTP and Host CLI | recorded old Linux reply | `testanyware-agent-client` transport tests that serve raw HTTP bytes to the real client, before any destination replacement |
| Temporary file removed at SIGTERM | HTTP and Host CLI | k7 interrupt case | Host CLI test |
| Record validity and skew | HTTP and Host CLI | none | Recorded new, legacy macOS, legacy Linux and Windows replies |
| Legacy projection keeps old values | Process and file, HTTP and Host CLI | none. Recorded replies of today's agents for each outcome are the reference | New agents give the same legacy fields, and an old Host CLI gives the same output and exit code |
| App route for `file cancel` | App | none | `app:routing-test` |

The unavailable-status control needs a leader whose exit status stays
unavailable past K + 1.4 s. A leader that ignores SIGTERM and dies on SIGKILL
does not qualify. The planner proposes the narrowest seam for it, at the
leader-wait source of the owner. The stalled-capture control uses a substituted
capture reader in the same way. Neither seam is a second execution backend.

The full public path is the third seam. The ten-clone acceptance trial tests
it. A lower pass never replaces that trial.

## Implementation candidates

The planner cuts the leaves. These candidates keep each schema with its reader.

- **A. Record reader.** The shared Rust types, record validity, the JSON
  fields, the unchanged text mode, errors, help, schemas and documents.
  Recorded new and legacy replies test it. It reads the record that B and C
  write.
- **B. Linux agent.** The lifecycle owner, the Linux legacy projection, the
  threaded server, cancellation, retention and the exact-length download.
- **C. macOS agent.** The `posix_spawn` and kqueue owner, the macOS legacy
  projection, cancellation, retention, the single-handle download and the
  vendored types.
- **D. Host CLI control.** `file cancel`, `--exec-id`, CLI interruption, the
  download framing check and the temporary-file cleanup.

B and C each work alone and remove established failures. A is useful only with
B or C, because it reads their record.

## Validation route

1. Land each implementation leaf with its own review and reconciliation.
2. Build isolated production test builds: a private Host CLI and app, and new
   goldens with `TESTANYWARE_AGENT_BIN_OVERRIDE`.
3. Run the controller self-check.
4. Check the four corruption controls against this record format.
5. Freeze the subjects, the ceilings and the bounds above.
6. Run the full inventory on five fresh macOS clones in sequence.
7. Run it on five fresh Linux clones in sequence.
8. Record the durable handoff and the resource audit.

The four corruption controls are an appended line break, a flipped binary byte,
a missing terminal outcome and a contradictory termination record. The
contradictory copy changes one claim of a valid termination block and leaves
every other field valid. Examples are `withinBounds: true` with `groupEmpty`
`no`, or a `groupEmptyAfterMs` before `killSentAfterMs`. The controller must
reject each damaged copy, and the originals must still match. It must accept
both leader orders and a record with `leaderStatus` `unavailable`.

The controller reads each exec result in JSON mode. The byte oracle compares
the decoded `evidence` bytes, because text mode keeps today's trimmed output.

Every root case, byte oracle and ceiling applies. A defect fails the trial. A
revised candidate starts a new trial. A baseline or deterministic pass never
replaces acceptance. The historical result of 0 of 10 stays unchanged.

## Decisions

- [ADR-0023](../adr/0023-guest-command-outcome-is-a-typed-record.md): the outcome is a typed record, separate from transport status. Its legacy fields keep each platform's values.
- [ADR-0024](../adr/0024-one-owner-ends-each-guest-command.md): one owner arbitrates by kernel state and ends the group while it holds the leader.
- [ADR-0025](../adr/0025-cancellation-is-explicit-and-disconnect-is-not-cancellation.md): cancellation names an exec id, and a disconnect is not a cancellation.
- [ADR-0026](../adr/0026-a-download-sends-one-open-file-version.md): a download sends one open file version, and in-place overwrite is excluded.
- Spec only: the Linux agent serves requests concurrently. The alternative, one
  request at a time, is the established failure C.
- Spec only: no investigation comes before this design, because no choice
  depends on an unresolved cause.

## Out of scope

- Durable jobs, leases, pooling, placement and replacement backends (issue #10).
- App routing (issue #8 and ADR-0004).
- Windows reliability.
- A cap on exec output size. Output size stays unbounded, as today. The time
  to read it has a deadline, and a capture that misses it arrives incomplete.
- Containment of escaped descendants, for example with cgroups.
- A consistent read of a file that a writer changes in place.
