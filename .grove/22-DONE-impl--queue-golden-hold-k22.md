# queue-golden-hold-k22

## Goal

A macOS golden creation takes its slot from the queue when the app runs. It
holds the slot through one open connection, and so between its boots too. No
start of the app takes that slot while the count is true.

## Context

- Spec `docs/specs/admission-queue.md`. Read these sections:
  - "Golden creation", with its sequence diagram.
  - "The count", for the VM names that a golden creation holds.
  - "The protocol", for `POST /v1/slots`.
- Meet the requirement "Golden creation takes its slot from the queue".
- ADR-0004: golden creation keeps its guest traffic on the direct path. Only the
  request for the slot goes over the Unix socket.
- `queue-vm-slots-k20` added version 2 of the protocol. `queue-golden-boot-k21`
  fixed the boots.

## Done when

- On a macOS host, `vm create-golden` of a macOS guest asks the app for a slot
  after its fast checks and before it changes anything. The fast checks are the
  directory check, the SSH key and the agent binary.
- `POST /v1/slots` takes `vms` and the optional `cwd`. A name that is not a plain
  VM id fails with `USAGE_ERROR` before the stream. The events are `progress`,
  `admitted` and `error`. After `admitted` the app sends nothing, and it holds
  the slot until the client closes the connection.
- The hold names the VMs that the golden creation boots. They are the setup VM
  and, for an IPSW build, the VM `<base>-building`. A golden creation is never
  unproven.
- Golden creation waits with no bound. Stderr shows its place and what holds the
  slots. Ctrl-C ends the wait.
- Golden creation closes the connection after its setup VM stopped. That is after
  the clone to the golden on success, and after its cleanup on a failure.
- A crash of golden creation closes the connection, and the hold ends. A setup VM
  that runs counts as a seen holder, and the app never stops it.
- When the app quits while the hold exists, golden creation goes on and writes one
  line to stderr. When the app quits or crashes while golden creation waits, it
  fails as a waiting start fails, with nothing changed.
- When the app is not running or not installed, golden creation runs as it does
  today and takes no slot. With an app of version 1, it fails with
  `APP_INCOMPATIBLE` and cause `protocol`. Linux and Windows golden creation take
  no slot.
- `vm slots` shows the hold with the kind `golden`. The menu shows it too.
- With `macos_slots` 0, golden creation fails with `VM_LIMIT_REACHED`.
- Protocol tests run the hold request through the library of the Host CLI. One
  test runs golden creation behind two VMs, and one test runs a start while the
  hold exists.

## Notes

- A guest that the app cannot see can still take the free slot between two boots.
  The spec accepts that risk. Golden creation then fails with the error that
  `queue-golden-boot-k21` gives.

## Decisions (running log)

**The hold is the open response, and it needs no new state machine.** `POST /v1/slots` answers
`200` with NDJSON. A thread of the app (`hold.rs`) joins the queue with `admit_golden`, sends
`progress` lines, sends `admitted`, and then waits for the cancel token of the body. Hyper drops
the body when the client closes the connection or crashes, and the drop cancels the token. The
thread then drops its `GoldenSlot`, which releases the slot. A golden creation is therefore never
a start record and never `unproven`.

**A golden creation is one own holder, whatever the number of its names.** `count::holders` takes
the golden holds as a separate list. `own_count()` is the own guests plus the number of golden
holds. A guest that a golden hold names is removed from the own guests and from the seen guests,
so the setup VM and a meta of the same name count once. `decide` gained `Waiter::golden`, so a
golden admission adds nothing to the unproven count of the same decision.

**The queue entry is a `Subject`.** `Entry` carries `Start { vm_id }` or `Golden { vms }`. A
refusal by the framework requeues starts only, so `refuse` always builds `Subject::Start`.

**`macos_slots` 0 takes the decision of a start without `--wait`.** `admit_golden` refuses at the
join with `refused_at_the_join`, the same function a start uses. The error is `VM_LIMIT_REACHED`
with `refused_by` `app` and `limit` 0, and the queue keeps nothing.

**The fast checks moved into `GoldenPreflight::check`.** The command order is now: directory
check, `GoldenPreflight::check` (SSH key, agent binary, setup VM name, and whether an IPSW build
will boot `<base>-building`), the request for a slot, and then `create_golden_after`. The old
`provision_boot1` and `create_golden` keep their signatures and call the new pair, so the live
tests need no change. The name `<base>-building` is in the hold only when the base VM does not
exist yet. `tart list` that fails reads as "does not exist", so the hold then names a VM that
may not boot, which only costs a name in the dedup.

**The lease is a task that owns the connection.** `AppClient::hold_slot` returns a `SlotLease`.
The lease owns a task with the request sender and the response body. A drop aborts the task,
which closes the connection, and `on_lost` runs only when the app closes it first. The Host CLI
writes one line for `on_lost` and goes on, as the spec says. The lease is held in `_slot` of
`run_vm_create_golden_macos` until `create_golden_after` returns, which is after the clone to the
golden on success and after `cleanup_setup_vm` on a failure.

**Only "no app" skips the slot.** `runs_without_the_app` is true for `APP_NOT_INSTALLED` and for
`APP_UNREACHABLE` with cause `not_running`. Every other failure, such as `APP_INCOMPATIBLE`
cause `protocol` for an app of version 1, `quitting` and `socket_error`, fails the command with
its own error.

**Ctrl-C needs no code.** The CLI installs no handler, so Ctrl-C ends the process. The kernel
closes the socket, and the app takes the waiter out of the queue. A test cancels the same way by
dropping the connection.

**Test seams.** The protocol tests run `app_route::hold_slot`, the library of the Host CLI,
against the real app and the fake tart. No test runs the real pipeline, which takes 10 to 40
minutes. The end-to-end tests run the real `vm create-golden` with the fake `tart` first on
`PATH`, a fake SSH key and a fake agent binary, and a vanilla version (`tahoe`) that the fake
cannot clone. So the command passes its checks, waits, is admitted, and stops at its first
`tart clone`. That shows the order of the checks, the slot and the first change. The directory
check of golden creation compares the CLI with the file of the app and not with XDG, so the
fixture for these tests names `state_dir` and `data_dir` in `config.rhai`.

**No review leaf, and the in-session allowance stays unspent.** The one non-obvious claim is the
release on a closed connection. A test shows it for a client that drops, for a client that
crashes (a raw socket closed), and for an app that quits. The mutation checks are below.

**Checks that a test can fail.** Three mutations each turned a test red, and I reverted them.
(1) `decide` counted a golden admission as unproven: `a_golden_creation_admitted_in_contention_leaves_no_unproven_start` failed.
(2) `count::holders` kept a name that a golden hold also names in the own guests:
`a_guest_that_a_golden_creation_names_is_neither_seen_nor_a_second_own_holder` failed.
(3) The thread of the hold dropped its slot without waiting for the close:
`a_start_beside_the_hold_takes_the_other_slot_and_the_next_start_waits` and
`the_crash_of_a_golden_creation_closes_its_connection_and_ends_the_hold` failed.
The whole workspace passed after the work, and clippy shows nothing in the files of this leaf.
`task golden:hold-test` runs the tests of this leaf.

**What the live checks must still show.** `queue-live-checks-k25` should run a real golden creation
behind two real VMs. The real pipeline never ran under the hold in this leaf, because it takes
10 to 40 minutes. Its keeping of the lease is checked by the compiler and by the end-to-end test
that stops at the first `tart clone`.

