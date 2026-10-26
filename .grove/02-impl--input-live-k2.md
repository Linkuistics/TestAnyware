# input-live-k2

## Goal

Add the two live checks of issue 12 as one Taskfile task. Run them on this Mac
to reproduce the fault. Find out whether the path through the app causes it,
before any code of the input path changes.

## Context

- Issue 12: its table under "What happened", and its "Proposed checks for a fix".
- The running log of `plan-k1` holds the facts, the lead and the agreed seams.
- Two existing live checks show the shape: `scripts/queue-live.sh` (`task
  queue:live`) and `scripts/herdr-acceptance-live.sh` (`task
  app:herdr-acceptance`). Each takes `APP=<bundle>` and starts the development
  bundle when no app answers. Each quits only the app that it started, and it
  stops its guests on every exit.
- `scripts/tart-vnc-idle-repro.py` holds a bare RFB client with VNC
  authentication. tart serves VNC on loopback (`docs/reference/connection-spec.md`),
  so that client reaches a guest with no app.
- The a11y surface of the agent (`agent set-value`, `agent focus`, `agent press`)
  acts in the guest without VNC.

## Done when

- One Taskfile task, for example `input:live`, runs both checks on a macOS 27
  guest through the app. It prints PASS or FAIL for each check, and it exits
  non-zero at a FAIL. It takes `APP=<bundle>`, as `queue:live` does.
- The type check clicks a text field and reads its focus through `agent
  snapshot`. Then it types text and reads the value back.
- The scroll check uses a view taller than its window. It reads the position of
  one element, scrolls with the wheel and reads the position again. Then it
  drags the thumb of the scroll bar and reads the position again. Each step must
  move the element.
- The fixture window lives in this repository. It needs no change to the golden,
  and it needs no other project.
- The oracle passed once. An agent action that does not use VNC changes the
  field, and `agent snapshot` reads the change.
- A run with 2.6.0 on this Mac is on record, with each step that failed.
- A control run is on record. It sends the same input through a bare RFB client
  straight to the loopback endpoint of tart, and it closes in the ordinary way.
  Its result says whether the path through the app causes the fault.
- A research note under `docs/research/` holds the reproduction and the control.
  The Context of `input-delivery-k3` gives its path.
- The spec testanyware-app lists the new task among the live checks of a
  release.

## Notes

- The lead: since 2.5.0, the app resets the guest connection when its client
  has gone. An `input` command closes straight after its last event, and it
  waits for no answer. A loopback experiment in `plan-k1` shows that the macOS
  kernel keeps unread bytes after a reset. So if the reset is the cause, the VNC
  server of tart drops the input when it sees the reset. Test the lead, and do
  not presume it.
- If the control does not settle the cause, a development bundle built at
  v2.4.0 can locate it. Its first start can need a person to allow the Local
  Network alert.
- The logs of Koine may show when the passing run of 24 October happened, and
  with which release.
- Both macOS guest slots may be in use. Start the guest with `vm start --wait`
  (ADR-0022).
- Do not fix the fault here. `input-delivery-k3` fixes it.
