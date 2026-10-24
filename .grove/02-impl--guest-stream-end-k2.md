# guest-stream-end-k2

## Goal

Stop the leak of descriptors in the app's guest streams. When the client of a
stream has gone, the app closes the guest connection, even when the guest never
ends its own direction. A half-close still passes through while the client
reads.

## Context

- The decision log of `plan-k1`: the failed harness run, the counts of
  descriptors, the socket states, and the restart. Find it with
  `grove-llm resolve plan-k1`.
- `relay` in the app's stream module. It calls `copy_bidirectional_with_sizes`,
  which returns only when both directions end. Its unit tests include a
  half-close in both directions.
- The spec, the section on guest streams. It says: "The app opens it when the
  stream opens, and closes it when either side closes." It also says: "A
  half-close passes through." The code keeps the connection until both
  directions end, so the code does not match the first rule.
- The open risk "A silent stall of every stream" in the spec.
- ADR-0004 (each command opens its own guest connections) and ADR-0018 (tart
  guests get a VNC keeper).

## Done when

- The app tells a client's half-close from a client that has gone. After the
  client has gone, the app closes the guest connection, and the stream ends.
- A client that half-closes and still reads gets the guest's answer, as today.
- The protocol seam has a test that fails before the fix and passes after it.
  The app runs in its test mode. The real Host CLI opens many VNC streams to a
  synthetic RFB guest that never closes after the client's end. The app's open
  descriptors stay flat, and the fake guest sees each connection end.
- The relay's unit tests pass, the half-close test included.
- A Taskfile live check on this Mac sends many `screen` commands to a tart guest
  through a development bundle of the fix. The app's descriptors and tart's
  connections in `CLOSE_WAIT` stay flat. The decision log records the counts
  before and after.
- The spec states the rule that ends a guest stream, in the owner's agreed
  terms. Its test seams name the new test and the live check. The open risk of a
  stall says what the fix covers and what it does not.
- `task cli:test` and the other tasks that this change touches pass.

## Notes

- The leak. Each VNC stream to a tart guest left two descriptors in the app: the
  client's connection on `testanyware.sock` and the guest connection in
  `FIN_WAIT_2`. tart held its side in `CLOSE_WAIT`. The app's soft limit is the
  launchd default of 256, so the app failed after about 120 VNC commands.
- After the app's sockets closed, tart released all 94 of its waiting
  connections within about two minutes. So a fix in the app alone is enough,
  and the live check shows it.
- On a Unix socket, a client's half-close and its full close both give an end
  of file to a read. So a rule that ends the stream at the client's first end of
  file breaks a client that half-closes and then waits. Find what the platform
  offers to tell the two apart before you choose. Check the claim against the
  source or the documents of the library, not memory.
- Whether the app also raises its soft limit of descriptors, as a second
  defence, is your decision. Record it with its reason.
- This fix changes code, so it voids the candidate of `f0108944`. The release
  leaf after this one rebuilds the candidate.
- If the work proves bigger than one session, decompose this leaf. Put any new
  leaf before `release-2-5-0-k3`.
