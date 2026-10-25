# queue-vm-slots-k20

## Goal

A person or an agent can see the queue. The new command `vm slots` shows the
slots, each holder, each waiter and who asked. This increment adds version 2 of
the app protocol, which golden creation needs later.

## Context

- Spec `docs/specs/admission-queue.md`. Read these sections:
  - "Who asked" and "The protocol".
  - "The command surface", for `vm slots`, the help and the instructions.
  - "Documents that change".
- Meet the requirement "`vm slots` shows each waiter and each holder".
- `docs/specs/testanyware-app.md` holds the table of version skew and the menu.
- `queue-order-k16`, `queue-refusal-k17` and `queue-external-k19` built the state
  that this command reads.
- Consumers use only the command surface. Keep every command, flag, variable, VM
  id, exit code and `--json` field as it is.

## Done when

- The protocol range of the app and of the Host CLI is 1 to 2. `GET /v1/slots`
  is a request of version 2. `POST /v1/vms` keeps its form for both versions, and
  its body gains the optional member `cwd`.
- A command that needs version 2 fails with `APP_INCOMPATIBLE` and cause
  `protocol` when the range of the app ends at 1. With an older app, `vm start`
  works as that app does. The test `app-version-skew` has the new rows.
- `vm slots` prints the members that the spec lists: `slots`, `limit`, `holders`,
  `queue`, `contention`, `observed_at` and `unanswered_since`. Its schema id is
  `vm-slots`, and the schema is under `docs/reference/cli-schemas/`.
- Each waiter and each holder that the app admitted carries `requested_by`. Its
  `pid` comes from the kernel for the client socket. Its `cwd` comes from the
  Host CLI request, and an older Host CLI gives `null`.
- On a Linux or Windows host, `vm slots` reports `slots` 0, `limit` 0 and empty
  lists, and it needs no app. On a macOS host without the app, it fails with
  `APP_UNREACHABLE` or `APP_NOT_INSTALLED`.
- The menu of the app shows the slots, each holder and each waiter, with its place
  and who asked.
- The help of `vm start` names `vm slots`. The remedy of `VM_LIMIT_REACHED`
  names `vm slots` and `--wait`.
- `llm-instructions` tells an agent to pass `--wait` for a macOS start, to run a
  long wait in the background, and to read `vm slots`. `capabilities` lists
  `vm slots`.
- `docs/architecture/cli-design-contract.md` lists `vm slots` in its two tables.
  `docs/reference/cli-commands.md` documents the command. The app spec changes
  its protocol table and its menu.
- A protocol test drives `vm slots` while agents in two worktrees wait. It
  checks the order, the pid and the `cwd` of each waiter.
- The Taskfile lists any new test target.

## Notes

- The holders of golden creation appear in `vm slots` when
  `queue-golden-hold-k22` adds them. This leaf shows the kinds `vm` and
  `external`.

## Decisions (running log)

**The app learns who asked in the gate, not in the ask.** `AppGate` now carries the
`RequestedBy` of the start and calls `AdmissionQueue::admit_for`. `SlotAsk` and the 40 calls
of `admit` in the tests stay as they were. The pid is `peer_cred().pid()` of the accepted
socket, captured in `serve` and passed down `handle` and `route`. The `cwd` is a new optional
member of `StartRequest`, which an older app ignores.

**A running guest keeps who asked.** An admitted start leaves `admitted` when its hold ends,
and its meta then holds the slot. So the queue keeps a `starts` record by VM id, with the
request id, the requester and the time of the admission. `note_holders` drops each record
whose VM is no longer an own holder. A seen holder and an adopted VM have no record, so they
show `request_id` and `requested_by` as `null`, as the spec says.

**`since` of a holder with no record is the first moment the queue noted it.** The spec does
not say what `since` means for an external or an adopted holder. The queue keeps a
`held_since` map for that. It is "since the app first saw it", which is an upper bound on
the age of the guest.

**`GET /v1/slots` observes first and waits at most 8 s.** The spec says the app observes
again for each `vm slots` request. `observe_now` asks for the next observation and waits for
it to end with an answer or a failure. The 8 s bound stops a slow tart from holding the
command. The view states `observed_at`, so a reader sees how old it is.

**A version-1 client has no `GET /v1/slots`.** `check_request` already returns the version of
the pair. A pair at version 1 gets the same 404 `USAGE_ERROR` as an unknown route, with the
version in the message.

**The Host CLI decides "app too old" from a 404 and the headers.** `get_json_needing` reads
the range from the headers of the 404 of an app below version 2, and builds
`APP_INCOMPATIBLE` with cause `protocol` through the existing `AppError::incompatible_protocol`
and the existing remedy table (`needs_version` in `skew.rs`). Any other refusal, such as one
on the directories, stays the app's own.

**The range is 1 to 2 on both sides, and the tests that forged 1 to 1 moved.** The Host CLI
states 1-2, so an app of 1-1 and a Host CLI of 1-2 agree on version 1, and `vm start` works
as that app does. Four tests used `2-3` as a range that "does not overlap". It overlaps 1-2
at 2, so they use `3-4`. The assertions on `1-1` in the messages and on the status
(`protocol_range.max`) are now 2.

**`capabilities` gains a `commands` member.** The old output named only the top-level groups,
so `vm slots` could not appear in it. `commands` lists each canonical command by its path,
from the same table as `schema`. It is a new member, so no existing member changed. The
contract's §8.1 example shows it. `cli-contract` checks that every canonical command is in it.

**The menu reads the queue as it stands.** The menu calls `view()` and not `observe_now`, so
opening the menu never waits for tart. The spec says the app observes for each `vm slots`
request, and the menu is not one.

**The remedy of `VM_LIMIT_REACHED` always names `vm slots` and `--wait`.** It is a constant
sentence after the stop advice, in every case. The golden creation does not take a slot from
the queue yet (`queue-golden-hold-k22`), and the sentence says `--wait` of `vm start`.

**One test target.** `task slots:test` runs the unit tests of this leaf and the three
integration filters. The tests also stay in `app:routing-test` and `cli:test`.

**Checks that a test can fail.** The two-worktree test failed when the app stated no pid
(line 1261) and when the Host CLI stated no `cwd` (line 1262). Both mutations were reverted.

**The spec says what the code does about `since` and the wait.** Two sentences went into
"The command surface": the meaning of `since` for a holder that the app did not admit, and
the 8 s bound of the observation of a request.

**A known edge, left as it is.** If the meta of a running guest is torn at the moment of a
`view()`, the guest is no own holder for that call and its record of who asked is dropped. The
guest then shows no `request_id` until the app restarts. The hold of a start outlives the
write of its meta (`lifecycle.rs`), so a normal start never meets this.

**No review leaf, and the in-session allowance stays unspent.** The new logic is a snapshot
under the one lock of the queue, one pure rule in `skew.rs`, and plumbing for the pid and the
`cwd`. The two-worktree test failed under two mutations (no pid, no `cwd`), and the version
rows run against the real app.
