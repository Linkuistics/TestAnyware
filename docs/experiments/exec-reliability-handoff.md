# Guest command reliability handoff (issue #14)

This document hands the result of issue #14 to Ravel-Strategies. It states
the released contract, the evidence for each acceptance condition, the support
limits and what the tutorial trial of Ravel-Strategies must run again. It
also records the resource audit of the workstream.

Release **2.9.0** carries the guest command lifecycle, explicit cancellation,
CLI interruption and exact evidence transfer. The frozen acceptance trial
passed **10 of 10**: five fresh macOS clones, then five fresh Linux clones.
The historical tutorial trial of Ravel-Strategies stays on record as **0 of
10, unavailable**. This result does not replace it, and the tutorial trial
must run again on the new release.

The full contract is the spec
[`guest-command-lifecycle.md`](../specs/guest-command-lifecycle.md). The
decisions are ADR-0023 to ADR-0026. This document summarises them, and the
spec wins where the two differ.

## Release identity

The release commit merges the delivery branch with `main` after release 2.8.0.
The trees `agents/` and `cli-rs/` of the release commit equal those of the
trial source commit. The trial therefore measured the source of the release.

| Item | Identity |
|---|---|
| Tag | `v2.9.0` |
| Release commit | `55af0aa84eea9eebbedebd1c2362edecf5368209` |
| Trial source commit | `fee86f928e8e923a8449cc3e948fb015209a3ed9` |
| GitHub release | <https://github.com/Linkuistics/TestAnyware/releases/tag/v2.9.0> |
| Homebrew tap commit | `11b79223`, "testanyware v2.9.0", in `linkuistics/taps` |
| Notarisation | Apple submission `5a8eceaa-493a-4b25-98ac-2bb51ec030aa`, stapled |
| App signing | `dev.antony.TestAnyware`, team `TA43A4RUP3`, bundle version 2.9.0 |
| macOS Host CLI binary | SHA-256 `808eb51742ff48f310390c288fcdb370fcd6b2f599e88f80b14d3d30547820af` |
| macOS agent of the release | SHA-256 `aeec230a09722abf2795bb49bbafbf122d7eb69afd9109ca83da48a58880b6cc` |
| Linux agent of the release | The 11 files match the trial's frozen listing by SHA-256 |

The release artifacts have these SHA-256 digests:

| Artifact | SHA-256 |
|---|---|
| `testanyware-app-v2.9.0-aarch64-apple-darwin.zip` | `cb2332bc6e6633148bb05ec6192cfb092f56f00d3ca25ad7d1d58a82ba81785e` |
| `testanyware-app.rb` | `e9053ed32fd0ef80bd3da1e8df9d41e568f346971f784ea8032818d568cbee02` |
| `testanyware-v2.9.0-aarch64-apple-darwin.tar.xz` | `d9c03b56414e37e823d8581b8277c5a2f2c43b97994c6d451a7c09a571c83f27` |
| `testanyware-v2.9.0-aarch64-unknown-linux-gnu.tar.xz` | `aacd97940005658460792867b68229f11f35c3baea1fde4ce0c7eb784673384d` |
| `testanyware-v2.9.0-x86_64-unknown-linux-gnu.tar.xz` | `61b22e92afd90b5822c0e22f12792965d941660b75dc7c8d171d87c03aa1f988` |
| `testanyware.rb` | `2603be3d704f69a43b87313516e3b621249d5c2875773d2f3c534f9266f5d7db` |

The macOS agent of the release is a new build of the trial source. Its binary
differs from the trial agent, SHA-256 prefix `7998bee77cf6`, which another
workspace built. No agent reports a version, so the source commit and these
digests name each agent.

## What the tutorial trial must run again

A guest gets its agent only when its golden is built. No command replaces the
agent of an existing golden. A clone of an old golden therefore keeps the old
agent, and the new Host CLI then reports `outcome` as `unknown`.

Prepare the host and the goldens:

1. Upgrade the formula `linkuistics/taps/testanyware` to 2.9.0.
2. Upgrade the cask `linkuistics/taps/testanyware-app` to 2.9.0.
3. Run `testanyware doctor`, and check that the Host CLI and the app both
   report build 2.9.0.
4. If `vm start` fails with the cause `local_network_denied`, allow
   TestAnyware in the Local Network settings of macOS.
5. Build each golden again with `vm create-golden --platform macos` or
   `--platform linux`.
6. Use `--name` to keep an old golden beside its new golden.
7. Start one clone of each new golden, and check that `file exec --json`
   reports `"record": "agent"`.

Change the tutorial controller:

1. Read each `file exec` result in JSON mode.
2. Compare the decoded `evidence` bytes with the byte oracle. Do not compare
   the legacy `stdout` and `stderr`, because they stay trimmed.
3. Take the guest result from `outcome`. Do not take it from `exit_code` or
   from the process exit status.
4. Pass your own `--exec-id` to each command that a fallback may have to
   cancel after SIGKILL.
5. Publish changing evidence under a fresh name, and download that name.

The controller counts each of these as a failure:

- An envelope with `ok: false`.
- An outcome of kind `unknown`, which comes from an old agent.
- A stream with `capture: "incomplete"`.
- A termination block with `withinBounds: false`.
- A download that fails with `CONNECTION_DROPPED` or `DOWNLOAD_FRAMING_INVALID`.

Then run the complete ten-clone tutorial acceptance trial again, on fresh
clones of the new goldens. Freeze its inputs first, and record the release
identities above. Three earlier results carry nothing into this trial:

- The historical result of 0 of 10, unavailable.
- Its successful prefix and the disposal of its four diagnostic VMs.
- The TestAnyware trial of 10 of 10. It ran the root inventory of issue #14,
  not the tutorial.

## Causal findings

The investigations kept each symptom separate. A finding is established when
it has a retained failure or a deterministic reproducer. The spec section
"Evidence basis" gives every row with its source.

| Finding | State | Repair |
|---|---|---|
| A Linux deadline killed only the shell, and a forked child survived | Established | The owner signals the owned process group |
| The Linux agent served one request at a time, so `/exec` blocked every other request | Established | A threading HTTP server |
| Both agents trimmed output and dropped non-UTF-8 bytes | Established | The record carries exact bytes in base64 |
| A signal, a deadline and a spawn error arrived as exit codes | Established | A typed `outcome` |
| A macOS spawn error arrived as HTTP 500 after about 3 s | Established | `spawn_failed` with the errno in an HTTP 200 record |
| A disconnect did not end the command on either agent | Established | Explicit cancellation by exec id |
| A macOS download took three path lookups and could send wrong bytes | Established on the host | The agent reads one open handle |
| A Linux download sent bytes past its declared length | Established | The agent sends exactly the declared length |
| An in-place overwrite gave torn bytes on both agents | Established | Excluded from the contract |
| An interrupted download left a temporary file | Established | The Host CLI removes it at SIGINT and SIGTERM |
| The k7 download defect caused failure D in the guest | Unresolved | The guest cause is not claimed |

The two macOS symptoms of issue #14 have separate accounts:

- **Ordinary completion (failure A, operation 08).** The old runner reported
  finished commands as timed out in a host probe when the dispatch pool had
  no free worker. The new owner waits on kqueue on its own thread. On the
  same probe, the base runner gave `defect` and the owner gave `clean`. The
  cause of the guest operation 08 failure stays unresolved, because the probe
  does not separate two candidate mechanisms.
- **Post-timeout inspection.** The historical stall did not reproduce in
  three conditions (k4, k193 and k195). One later run gave a 30 s stall on a
  process listing, and its cause is not proved. No evidence shows that the
  two symptoms share a cause.

Each exec record now names the event that ended its command and the lateness
of the owner. If either macOS symptom returns, the record shows it, and a
controller fails the run.

## The contract

### Outcome kinds

The agent record holds one outcome. The Host CLI copies it to the JSON field
`outcome` in snake_case.

| Kind | Fields | Meaning |
|---|---|---|
| `exited` | `exit_code`, 0 to 255 | The command exited, for example with 23 |
| `signalled` | `signal`, `signal_name` | A signal ended the leader |
| `timed_out` | `timeout_seconds` | The guest deadline passed, and the owner ended the group |
| `cancelled` | `before_start` | A cancellation ended the command, or came before it started |
| `spawn_failed` | `errno`, `error_name`, `message` | The agent could not start the command |
| `unknown` | `reason`: `legacy_agent` | An old agent answered. A caller never counts it as success |

A transport failure is not an outcome. It sets the envelope `ok` to false,
with an error code such as `CONNECTION_DROPPED`.

### JSON fields of `file exec`

The envelope `ok` reports only host transport and record validity. A guest
outcome never sets `ok` to false. The fields `command`, `exit_code`,
`stdout`, `stderr` and `timed_out` keep their old values. The release adds
these fields:

| Field | Content |
|---|---|
| `exec_id` | The exec id |
| `record` | `agent` for a new agent, `legacy` for an old agent |
| `legacy_projection` | `macos` or `linux`, or null |
| `outcome` | The outcome above |
| `evidence` | `stdout` and `stderr`, each with `base64`, `bytes`, `file_bytes`, `sha256`, `capture` and `reason` |
| `lifecycle` | `leader_pid`, `group_id`, `leader_start_time`, `spawn_ms`, `duration_ms`, `owner_lateness_ms`, `finalised_after_ms` |
| `termination` | For a started `timed_out` or `cancelled` command: the step times, `leader_status`, `group_empty` and `within_bounds` |
| `remaining_group` | For `exited` and `signalled`: `none`, `present` or `unknown` |

The Host CLI checks each record against the validity rules of the spec. A
contradictory record gives `EXEC_RECORD_INVALID` and exit 1, never a success.
`schema_version` stays `1.0`, and text mode does not change.

### Bounds

The root brief of issue #14 set the first group, and the spec added the
second. A record field observes some bounds in the agent. The outer
controller measures each phase from the host, and an agent field never
replaces that measurement.

| Phase | Bound | Observation |
|---|---|---|
| Termination grace | 2 s at most | `termination.kill_sent_after_ms` |
| SIGKILL, drain and reap | 2 s at most | `finalised_after_ms` minus `kill_sent_after_ms` |
| Ordinary observation | 30 s | Controller |
| Intentional guest timeout | 2 s | `outcome.timeout_seconds` and `duration_ms` |
| Raw host exec | Guest budget + 20 s | Controller |
| Transfer | 150 s | Controller |
| Independent disposal | 45 s | Controller |
| Guest cleanup | 30 s | Controller |
| CLI SIGTERM, then SIGKILL | 2 s, then 2 s | Controller |
| Spawn | 1 s | `spawn_ms` |
| Owner lateness at a deadline | 0.5 s | `owner_lateness_ms` |
| Outcome to record, without termination | 2 s | `finalised_after_ms` |
| Outcome to record, with termination | 4 s | `finalised_after_ms` |
| Cancellation reply with a 5 s wait | 5 s | Controller |
| CLI SIGTERM to signal death | 2 s | Controller |
| Health during a running command | 1 s | Controller |

The trial froze two more bounds: 420 s for `vm start` and 30 s for a slot.

### Explicit cancellation

The command `file cancel --vm VM --exec-id ID [--wait SECONDS] [--json]`
sends one cancellation to the agent endpoint `POST /exec/cancel`. The reply
state is `terminating`, `terminal` with the record, or `unknown_id`. A
cancellation is idempotent and never changes a terminal outcome.

An `unknown_id` reply leaves a tombstone. A later `/exec` with that id
returns `cancelled` with `before_start: true` and starts nothing. The reply
`unknown_id` never proves that no command ran.

The agent keeps each terminal record and each tombstone for 300 s, and 64 of
them at most. An agent restart loses every record.

A disconnect is not a cancellation. A closed connection or a client deadline
changes nothing in the guest. The command reaches its outcome, and the agent
keeps the record.

### CLI interruption

At SIGINT or SIGTERM, `file exec` opens a new guest connection. It sends one
cancellation for its own exec id and waits 1.5 s at most for the reply. Then
it raises the same signal again, so the parent sees the same signal death as
before.

SIGKILL cannot be handled. Its fallback is `file cancel --exec-id ID`, or
the guest deadline. An old agent answers the cancellation with HTTP 404, and
the Host CLI then raises the signal without a cancellation.

### Escaped-descendant limit

The agent owns the leader and every process that stays in the leader's
process group. A descendant that leaves the group escapes. The agent does not
signal, wait for or report an escaped descendant. An escaped descendant can
keep a capture file open and write to it, and the record does not cover
those later bytes.

After a normal exit, the agent does not signal the remaining group members,
because tutorials can depend on them. The field `remaining_group` reports
them.

### Exclusions of the spec

The spec excludes these subjects:

- Durable jobs, leases, pooling, placement and replacement backends. Issue
  #10 owns them.
- App routing. Issue #8 and ADR-0004 own it.
- Windows reliability. A new Host CLI reads a Windows reply as a legacy
  record and makes no claim.
- A cap on exec output size. The read has a deadline, and a capture that
  misses it arrives incomplete.
- Containment of escaped descendants, for example with cgroups.
- A consistent read of a file that a writer changes in place.

The download contract covers no change, an append and a replacement by
rename. A truncation gives a detected failure. The stable alternative for
changing evidence is fresh-name publication.

### Compatibility

Every change is additive. Each existing field, text output and exit code
keeps its value for each supported agent, apart from three departures. A
finished command is now `exited`, where the old macOS agent sometimes
reported a timeout. A macOS spawn error is an HTTP 200 record. A cancellation
is new.

| Host CLI | Agent | Result |
|---|---|---|
| 2.9.0 | 2.9.0 macOS or Linux | The full contract |
| 2.9.0 | Old macOS or Linux | Legacy record, `outcome` unknown, no cancellation |
| 2.9.0 | Windows | Legacy record, no claim |
| Old | 2.9.0 macOS or Linux | The old values of the platform, no cancellation |

## Regression and integration results

Each implementation increment had its own adversarial review and an
integration that reconciled each finding. The running log of each leaf is in
`.grove/` at its integration commit. The table gives the final verification
of each integration.

| Increment | Producer, review, integration | Final verification |
|---|---|---|
| Linux lifecycle owner and typed record | `c6084c28`, `be6d55e6`, `30bdf545` | `agent:linux:test`: 31 passed and 39 skipped on the host, 70 passed in the pinned Linux container |
| Host CLI record reader | `58b52586`, `f8d67958`, `27053381` | `cli:test` passed every suite. The protocol crate passed 61, 23 and 14 tests on Rust 1.81 |
| macOS lifecycle owner | `d7e121df`, `6e6b12a7`, `3823eaa1` | Completion probe: base runner `defect`, owner `clean`. `agent:macos:test` passed 20 of 20 runs |
| Linux cancellation, retention, download and shutdown | `b41c7492`, `23dc2465`, `f97f8e8f` | `agent:linux:test`: 50 passed and 74 skipped on the host, 124 passed in the container. The responsiveness file passed 20 of 20 runs |
| macOS cancellation, retention, download and shutdown | `45fb4da1`, `17239805`, `251bf072` | `agent:macos:test` passed 83 tests with no skip. On the k221 agent, 57,995 of 57,995 stress reads gave whole versions. The k238 repair then changed only the flags of the download open |
| Host CLI cancel, interruption and download framing | `282ee507`, `20837279`, `5d558288` | `cli:clippy`, `cli:test`, `app:routing-test` and `cli:control-old-path` exited 0. The old path failed each of its four signal cases |
| Acceptance plan and checker | `b62275b4`, `2b1ac16c`, `fee86f92` | The checker rejected each of the four damaged copies |
| Acceptance preparation and freeze | `a2c877cb`, `8eb40a3f`, `b67459d2` | Six freeze findings repaired, and the manifest refrozen |
| Acceptance trial | `4a11b05e` | 10 of 10 passed. An independent recheck passed each clone, and a copy with one flipped bit failed it |
| Release | `55af0aa8`, `8fbb7568` | `release:candidate-verify`, `release:app-verify`, the Linux host harness and `release:upgrade-check` passed |

Each old-path control stays outside the passing suite. It runs through its
own `diagnostic:*-old-path` task and must fail on the old code. The spec
section "Test seams and required controls" lists each control and its seam.

## Support limits

These limits bound every claim of this document:

- The trial ran on one macOS 27.0.1 host. It measures nothing about Windows.
- A pass of 10 clones does not promise zero future failures. Ten clones
  cannot show a rare timing defect.
- The result holds for the frozen builds. A changed build, agent, golden or
  checker is a new candidate, and it needs a new trial.
- The fifth clone of each OS had no later clone, so its reset is not
  demonstrated.
- The app identity names the app that answered `doctor` once in each clone.
  It does not prove that the same app answered every later call.
- A guest reset after a complete download body is not detectable. On Darwin,
  the app relay sees a plain end of stream, and ADR-0004 gives it no in-band
  error. Bytes that reached the host arrive first, and the framing check
  rejects surplus bytes.
- The Linux download opens a FIFO with a blocking `open`. That request then
  holds its own thread, and health and other requests stay free.
- The cause of macOS operation 08, of the post-timeout stall and of the run 42
  observation gap stays unknown.
- Two release checks failed once with no known cause. In the second Linux
  harness run, a macOS guest did not show Finder's menu bar within 120 s.
  The first snapshot of the first upgrade check also failed. Each check
  passed on the next run.
- The installed app reports that it does not start at login. Releases 2.7.0
  and 2.8.0 reported the same state.

## Map of the root acceptance conditions

Each row is one item of the section "Done when" of the root brief.

| Condition | Evidence | State |
|---|---|---|
| Old failures or controlled reproducers are kept, and controls on the new code show the repair. macOS completion and post-timeout inspection have separate accounts. | The causal reports in the evidence index. The spec "Evidence basis" and "Test seams" sections. The section "Causal findings" above | Met. The operation 08 cause and the stall cause stay unresolved, as the condition allows |
| Structured interfaces separate exit 23, timeout, cancellation, signal death, spawn error and transport failure | ADR-0023, the spec "Exec record" and "Host CLI semantics" sections, `file-exec.json`. The trial cases for exit 23, timeout and cancellation | Met for macOS and Linux |
| Timeout and cancellation stop new work and reap the owned group within bounds. Partial evidence stays, child identities are checked, and the escaped-descendant limit is stated | ADR-0024, the spec "Termination" and "Limits" sections, the agent suites, ten after-timeout and ten after-cancel checks in each trial clone | Met. Escaped descendants stay outside the owned group |
| Explicit cancellation and CLI interruption have separate tested contracts. The outer controller survives, and requests stay responsive | ADR-0025, the Host CLI control tests, the trial interruption case (2.0 s to 2.1 s) and the after-case observations (0.08 s or less) | Met |
| Exact stdout, stderr, status and binary bytes survive transfer. Bad evidence never counts as success | The byte oracle and the four corruption controls in each trial clone. `DOWNLOAD_FRAMING_INVALID` and `CONNECTION_DROPPED` | Met |
| Mutable-file reads have a demonstrated contract, or an exclusion and a tested alternative | ADR-0026. In-place overwrite is excluded. Fresh-name publication gave 200 of 200 whole versions on each agent in k7 and k14. The k221 macOS agent gave whole versions for each rename read | Met. The rename result comes from host loopback, not from a guest, and it predates the k238 change to the open flags |
| Material implementation is reviewed, and protocol changes stay compatible and documented together | The reviews and integrations in the table above. The spec "Compatibility and version skew" section | Met. No Windows result is implied |
| The frozen trial passes on five fresh macOS clones, then five fresh Linux clones | [`exec-reliability-acceptance-trial.md`](exec-reliability-acceptance-trial.md) | Met: 10 of 10, passed |
| A durable public handoff exists, and every owned resource is disposed of or named with a remedy | This document and the section "Resource audit" | Met, with the residuals that the audit names |

## Resource audit

The audit ran on 2026-11-20. It recorded the host before and after disposal
in `audit-before.txt` and `audit-after.txt`. Both files are in
`experiments/exec-reliability/evidence/lifecycle-handoff-k227/`.

### Retained goldens

The tool `diagnostic:acceptance-images` hashed each retained Tart image. A
compare with the k224 record `images-final.json` passed for each image. Each
one kept its digests and its disk and NVRAM times, so none booted. A scratch
copy with one changed disk digest failed the same compare, so the instrument
can fail.

| Retained image | Result |
|---|---|
| `testanyware-base-macos-27` | Unchanged since 2026-11-18, by digest |
| `testanyware-golden-linux-24.04` | Unchanged since 2026-11-18, by digest |
| `testanyware-golden-macos-27` | Unchanged since 2026-11-18, by digest |
| `testanyware-golden-macos-tahoe` | Unchanged since 2026-11-18, by digest |
| `testanyware-golden-windows-11` | Files last changed on 2026-10-18, before the workstream |
| `testanyware-golden-windows-11-pwfix` | Files last changed on 2026-10-18, before the workstream |

The Windows goldens are QEMU images, and no earlier digest exists for them.
Their check uses modification times only.

### Disposed resources

| Resource | Identity | Action | Record |
|---|---|---|---|
| k224 goldens | `testanyware-golden-macos-27-k224`, `testanyware-golden-linux-24.04-k224` | `vm delete` after a dry run | `golden-disposal.txt` |
| k224 export archives | Two `.tvm` files, 26,552,642,627 and 4,935,099,799 bytes | Removed with their directory `TestAnyware.k224-exports` | `export-disposal.txt` |
| Build workspace | jj workspace and git worktree `TestAnyware.k224-build`, 12 GB | Forgotten and removed | `workspace-disposal.txt` |
| Orphan test processes | `pytest` PIDs 64549, 64570, 64602 and 64617 of leaf k146. `xctest` PID 70224 | SIGTERM, then SIGKILL for the four that ignored it | `process-disposal.txt` |
| Session scratch space | 91 directories of earlier sessions, about 17 GB | Removed | `temporary-disposal.txt` |
| SwiftPM lock files | 33 empty files in `$TMPDIR` | Removed | `temporary-disposal.txt` |
| VM logs | 54 logs of 27 VM names that the repository records name | Removed | `temporary-disposal.txt` |

Each process passed an identity check before its signal: PID, start time,
command and a group with one member. After disposal, no clone ran, `vm slots`
showed no holder and no waiter, and the clone directory was empty.

### Residuals

These resources remain. Each one has an identity and a remedy.

| Residual | Identity | Remedy |
|---|---|---|
| This workspace | jj workspace `gh-issue-14` | The grove finish removes it |
| Release candidate directories | `target/candidates/v2.9.0` (206 MB) and `target/candidates/v2.9.0.checks` (482 MB), ignored by git | They go with this workspace. The GitHub release holds the published artifacts |
| Local Network entry of the private app | `TestAnyware Dev.app`, executable UUID `85863282-8EA4-3676-AD50-A94D299B2E66` | Remove it in System Settings after an upgrade to macOS 27.2. macOS 27.0.1 cannot remove an entry |
| Unclassified VM logs | 24 log names of the form `testanyware-<hex>` and `testanyware-setup-<hex>` in `~/.local/state/testanyware/vms`, listed in `temporary-disposal.txt` | No repository record names them, so another project may own them. Remove them after the owner identifies them |
| This session's scratch directory | `/private/tmp/claude-501/…-gh-issue-14/34eb4215-…` | The system removes `/private/tmp` content at restart |

The failed release runs of leaf k226 lived only in the ignored candidate
directory. Their logs are now copied to
`experiments/exec-reliability/evidence/lifecycle-release-k226/`, with a
`SHA256SUMS` file. The two `harness.log` digests match the prefixes that the
release log recorded.

## Open question for the owner: the volume of test evidence

The repository tracks 362,382 evidence files under
`experiments/exec-reliability/evidence/`. They take 7.3 GB on disk and a
340 MiB pack, and release 2.9.0 brought them to `main`. One file,
`termination-cutoff-k85/audit-1/verdict.json`, is 52.64 MB, above the 50 MB
size that GitHub recommends. Its two maps of subject hashes make most of that
size.

The owner said: "I think in general we keep too much/many testing artifacts."
This workstream removed no committed evidence. A pruning of the evidence, or
a rewrite of `main`, is the owner's decision. A pruning that keeps every
verdict and every failed sample, and removes bulk inputs, keeps each claim of
this document checkable.

## Evidence index

The reports below hold the causal evidence, in the order of the
investigation:

| Report | Subject |
|---|---|
| [`exec-reliability-diagnostic-foundation.md`](exec-reliability-diagnostic-foundation.md) | The bounded controller, the resource ledger and the routes |
| [`exec-reliability-macos-completion.md`](exec-reliability-macos-completion.md) | macOS ordinary completion (failure A) and the owner result |
| [`exec-reliability-macos-timeout-inspection.md`](exec-reliability-macos-timeout-inspection.md) | macOS post-timeout inspection |
| [`exec-reliability-macos-exec-outcomes.md`](exec-reliability-macos-exec-outcomes.md) | macOS exit, signal, spawn error, deadline and capture |
| [`exec-reliability-macos-disconnect-availability.md`](exec-reliability-macos-disconnect-availability.md) | macOS CLI interruption and responsiveness |
| [`exec-reliability-linux-timeout-lifecycle.md`](exec-reliability-linux-timeout-lifecycle.md) | Linux deadline, partial capture and child survival (failure B) |
| [`exec-reliability-linux-disconnect-availability.md`](exec-reliability-linux-disconnect-availability.md) | Linux interruption and serial requests (failure C) |
| [`exec-reliability-download-consistency.md`](exec-reliability-download-consistency.md) | The macOS truncated download (failure D) |
| [`exec-reliability-linux-download-consistency.md`](exec-reliability-linux-download-consistency.md) | The Linux download comparison |

The design, the acceptance work and the audit have these records:

| Record | Subject |
|---|---|
| [`guest-command-lifecycle.md`](../specs/guest-command-lifecycle.md) | The contract, the bounds, the controls and the release |
| [ADR-0023](../adr/0023-guest-command-outcome-is-a-typed-record.md) | The typed outcome record |
| [ADR-0024](../adr/0024-one-owner-ends-each-guest-command.md) | One owner ends each guest command |
| [ADR-0025](../adr/0025-cancellation-is-explicit-and-disconnect-is-not-cancellation.md) | Explicit cancellation |
| [ADR-0026](../adr/0026-a-download-sends-one-open-file-version.md) | One open file version per download |
| [`exec-reliability-acceptance-plan.md`](exec-reliability-acceptance-plan.md) | The trial plan and the checker |
| [`exec-reliability-acceptance-preparation.md`](exec-reliability-acceptance-preparation.md) | The private builds, the goldens and the freeze |
| [`exec-reliability-acceptance-trial.md`](exec-reliability-acceptance-trial.md) | The trial result of 10 of 10 |
| `experiments/exec-reliability/evidence/lifecycle-acceptance-k225/` | The raw evidence of the trial |
| `experiments/exec-reliability/evidence/lifecycle-release-k226/` | The failed release runs |
| `experiments/exec-reliability/evidence/lifecycle-handoff-k227/` | The resource audit |

The review report of each causal leaf is beside its report, with the leaf key
in its name.
