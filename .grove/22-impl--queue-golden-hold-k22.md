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
