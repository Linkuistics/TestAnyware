# queue-record-retry-k30

## Goal

A record that tart did not decide is decided later. The app keeps the VM and the
record while tart does not answer. It decides again after each observation whose
`tart list` answered. It never stops a VM that no meta proves.

## Context

- Spec `docs/specs/admission-queue.md`, the section "A quit, a crash and a
  restart". Read from "A record that tart did not decide" to the end of the
  section. Read the scenarios of the requirement "A crash or a restart keeps the
  count true", and the runs of that case in "Test seams".
- The brief of `queue-start-record-k23` holds the full "Done when".
- `queue-record-decision-k29` wrote the record, the function of the four rows and
  the decision before the socket is served. Read its decision log first.
- `queue-external-k19` built the observer. This child reuses it.

## Done when

- The observer decides again for each record that the fourth row kept. It decides
  after each observation whose `tart list` answered. That includes an observation
  whose later call failed. It decides after the app keeps that answer, and before
  the next observation starts. The tick of 3 s makes such an observation while no
  client asks for one.
- The observer tries the gate of the VM id, as a stop takes it, and never waits for
  it. When a start or a stop of the id holds the gate, the record waits for the next
  list. A start of the id replaces the record in any case. The observer holds no
  lock of the queue while it holds a gate.
- When a list reports the VM running, the app notes that for the record, in memory.
  The third row then decides at the first decision that gets the gate. Later lists
  change nothing.
- The second row never comes from the list of the observation. Suppose no list of
  this run reported the VM running. Then the observer reads `tart list` again under
  the gate, with the bound of 5 s, and decides with that read. When the read does
  not answer, the record stays.
- The decisions delay the next observation but never the decision at a deadline.
- `GET /v1/status` reports each new result.
- The tests of a decision again cover the lists of observations:
  - a list that reports the VM running gives the third row
  - a later list that does not report it gives the third row too
  - a list that does not report it, with no such list before it, never gives the
    second row.
- A protocol test replaces the clone of a record with another VM while the app is
  down. It runs with a `tart list` that does not answer at the next start. The tick
  of the app makes the observation, and the app decides again.
- Two more runs of that case pass:
  - Each later call to tart fails for a while. Then the VM stops, and the VM stays.
  - A `vm start --wait` of the same id joins the queue while the record stays. The
    queue serves its waiters, and `vm slots` answers.

## Notes

- The note in memory lives as long as the run of the app. After a crash, the next
  start decides again (spec, "Open risks").
