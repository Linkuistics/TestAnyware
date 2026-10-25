# queue-start-record-k23 — brief

## Goal

After a crash of the app, or a restart of the Mac, no slot is lost and no live
guest loses its slot. A start that the crash interrupted leaves no VM that the
app cannot account for. The app never stops a VM that it did not start.

## Context

- Spec `docs/specs/admission-queue.md`, the sections "A quit, a crash and a
  restart" (the start record and the quit) and "Open risks". Meet the
  requirement "A crash or a restart keeps the count true".
- `docs/specs/testanyware-app.md` holds the rules of adoption and the report of
  `GET /v1/status`.
- ADR-0018: the VNC keeper ends by itself when its port refuses.
- `queue-external-k19` built the observer. This leaf reuses it, because some
  decisions wait for a newer answer of tart.

## Done when

- The app writes a start record beside the sidecars of a VM before the clone of
  each start. The record names the VM id, the backend and the identity of each
  process that the start runs. The app removes the record when the start ends,
  as the spec states.
- Before it serves its socket, the app decides for each record that it finds. One
  pure function decides with the four rows of the table. Its tests cover these
  cases:
  - each row
  - a record beside the meta of its own start
  - a record beside the meta of an earlier start of the same id, in each
    outcome of the check of that meta
  - a keeper that matches while its `tart run` does not.
- The app never signals a process that the record does not prove. It keeps a
  running VM that no meta proves, and the VM counts as a seen holder.
- When tart does not answer, the app keeps the VM and the record. It decides
  again after each observation whose `tart list` answered. It tries the gate of
  the VM id but never waits for it. The second row asks for a newer read of
  `tart list` under the gate.
- `GET /v1/status` reports each result in its adoption results, with `removed`,
  the reason and the remedy.
- A quit of the app stops admitting, cancels each waiting and admitted start, and
  waits up to 10 s for their cleanup. A start that the 10 s leave unfinished keeps
  its record.
- A protocol test kills the app during a boot and then starts it again. A second
  test replaces the clone of a record with another VM while the app is down. It
  runs again with a `tart list` that does not answer at the next start. The tick
  of the app makes the observation then.
- The tests of the spec's other two runs pass. In one, each later call to tart
  fails for a while, then the VM stops and the VM stays. In the other, a
  `vm start --wait` of the same id joins the queue while the record stays.

## Notes

- This leaf proved larger than one session, so it is a node. Three children share
  its "Done when":
  - `queue-record-decision-k29`: the record, its removal, the pure function of
    the four rows, the decision before the socket is served, and the results in
    `GET /v1/status`. The fourth row only keeps the record here.
  - `queue-record-retry-k30`: the observer decides again for each record that
    tart did not decide, and the second row asks for a newer read. It holds the
    tests of a `tart list` that does not answer.
  - `queue-quit-k31`: the quit of the app, with its wait of 10 s.
- The node closes when the three children retire. The close compares the "Done
  when" above with the three.
