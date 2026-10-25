# queue-record-decision-k29

## Goal

A crash of the app during a start leaves no VM that the app cannot account for.
Before the app serves its socket, it decides for each start record that it finds.
It removes a VM only when the record proves its `tart run`. It also removes a VM
when tart shows that nothing runs under the name. It never signals a process that
the record does not prove.

## Context

- Spec `docs/specs/admission-queue.md`, the section "A quit, a crash and a
  restart". Read from "A start record" to the end of "A record that tart did not
  decide". Then read the scenarios of the requirement "A crash or a restart".
- Spec `docs/specs/testanyware-app.md`, "Lifecycle and adoption", for
  `unfinished_starts` in `GET /v1/status`.
- ADR-0018: the keeper ends by itself when its port refuses.
- The brief of `queue-start-record-k23` holds the full "Done when". The other two
  children take what this child leaves.
- `StartTrack` and `observe_tart_boot` in `testanyware-vm/src/lifecycle.rs` already
  read the identity of the `tart run` and of the keeper. `adoption::check_process`
  in the vm crate is the identity check.

## Done when

- The app writes a start record beside the sidecars of a VM. It writes the record
  before the clone of each start that it runs, on tart and on QEMU. The record
  names the VM id and the backend. It names the pid and the identity of the
  `tart run` and of the keeper once they exist. The Host CLI on the direct path
  writes none.
- The app removes the record when the start ends, as the spec states. On a success
  it removes the record after the meta is written and before it sends the result.
  On a failure or a cancel it removes the record after the cleanup.
- One pure function decides for a record that names a start on tart. Its inputs are
  the record and the identity check of each recorded process. They also include the
  state that tart reports for the VM. When a meta names the VM, the check of the
  meta's own `tart run` is an input too. The function returns the action of the
  four rows, and which sidecars stay. A record of a start on QEMU needs no
  function. The app removes it as the stop of the app does. The tests cover:
  - each row
  - a record beside the meta of its own start
  - a record beside the meta of an earlier start of the same id. The meta comes
    in each outcome of its check, an older meta included, in the third and the
    fourth row
  - a keeper that matches while its `tart run` does not.
- Before the app serves its socket, it applies the first, the second and the third
  row to each record. It uses one bounded `tart list`. The fourth row keeps the VM
  and the record, and it removes the sidecars as the table says. The decision
  again belongs to `queue-record-retry-k30`.
- The app signals only a process that the record proves. It keeps a running VM that
  no meta proves, and the VM counts as a seen holder.
- `GET /v1/status` holds `unfinished_starts` in its adoption results. Each entry has
  the VM id, `removed` and a `detail` with the reason and the remedy. A removal that
  fails keeps the record, and the next start of the app decides again.
- A protocol test kills the app during a boot and starts it again. The app removes
  the VM, the clone and the sidecars before it serves. A second test replaces the
  clone of a record with another VM while the app is down. The app keeps that VM
  and reports `removed` false.

## Notes

- The fourth row and the retry that follows it are the next child. This child may
  leave the fourth row as a function result that the startup applies without a
  retry.
- The spec and `docs/specs/testanyware-app.md` state the files of the state
  directory. Name the record file in the documents that list them.

## Decisions (running log)

**The record file is `<id>.start-record`, with no `.json` ending.** A VM id may hold dots, so the
spec of the id `a.start` is `a.start.json`, and a record named `a.start.json` would read as that
spec. The new suffix cannot collide with a spec (`.json`) or a meta (`.meta.json`). The vm crate
holds the type and the file (`start_record.rs`), and the app holds the decision (`unfinished.rs`).

**Only a start with `start_records` set writes a record.** `StartContext::for_app` sets it and
`direct` does not, so the Host CLI on the direct path writes none. `start_in` opens the record
before the backend runs and closes it when the backend returns. So the three ends of a start share
one removal: a success after the meta, and a failure or a cancel after the cleanup. The panic arm of
`start.rs` removes it after its own cleanup.

A record that cannot be written fails the start, because a crash would then leave a VM that nothing
accounts for. An update that adds a process is a best effort. The first write still names the id and
the backend.

**The record names one main process and replaces it on each try.** A refused `tart run` is gone, and
the next try writes its own pid and identity. The record names the keeper once it starts. A QEMU start
writes QEMU and swtpm after the runner returns, because the runner exposes them only then.

**A start on QEMU is removed as the stop of the app removes it.** `settle_qemu` signals the QEMU and
the swtpm that the record proves, through `qemu::teardown_verified`, removes the clone, the session
directory and the sidecars, and asks tart nothing. A crash before the runner returns leaves no
identity, so the app signals nothing and the clone goes. This is the open risk of "A crash before the
start record names its `tart run`", applied to QEMU.

**A meta that proves its `tart run` stops the second row from deleting.** The table gives the
second row to a record with no matching `tart run` and a VM that tart reports stopped, with no
exception for the meta. A meta whose own `tart run` matches proves a live process under the name,
and a `tart list` that says otherwise is older than the check or wrong. `decide` then keeps the VM
and the meta, as the third row does, and reports `removed` true. The test
`a_meta_that_proves_its_tart_run_stops_the_second_row_from_deleting_that_vm` holds it. The spec
table does not state this case, so the spec stays as it is.

**One bounded `tart list` serves every record, and it runs only when a record names tart.** `run`
reads the list once through `TartReader`, the reader of the observer, so the bound of 5 s applies.
A list that fails gives `Unanswered`, which is the fourth row. The row keeps the VM and the record,
and removes the sidecars that prove nothing. The decision again is `queue-record-retry-k30`.

**A VM that stays after its removal keeps the record and the sidecars.** `Effects` returns whether
the VM is gone. When it is not, `run` touches no file, so the next start of the app decides again
and `vm stop` can still find the VM. `GET /v1/status` reports `removed` false with the remedy.

**The decision runs in `main`, after the context exists and before `serve`.** The socket is bound
earlier, so a client that connects meanwhile waits in the backlog. `AdoptionLog::set_unfinished_starts`
stores the results, and the adoption that follows keeps them when it replaces the rest of the
status. Each result is also a line on stderr.

**The menu and `doctor` do not show `unfinished_starts`.** The spec asks for the field in
`GET /v1/status` only, and a client reads it there.

**The tests discriminate.** The two crash tests failed when an empty list replaced the call to
`run_for_app`. They passed with the call restored. The new tests are 19 unit tests in
`unfinished.rs`, 5 in `start_record.rs` and 2 in the protocol crate. Four protocol tests in
`app-vm-queue.rs` cover the life of the record, a cancel, a kill during a boot, and a VM that
replaced the clone. `TestApp::crash` and
`restart` are new in `tests/common`.
