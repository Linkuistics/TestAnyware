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
