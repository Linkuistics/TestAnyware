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

## Decisions (running log)

- **The platform tells a close from a half-close, and only through readiness.** On a
  Unix socket, a peer's `shutdown(SHUT_WR)` and its `close()` both end a read with
  end of file. The kernel sets another flag on a close: the socket can send no more.
  mio reads it as `EPOLLHUP` on Linux and as `EV_EOF` of the write filter on macOS
  (`mio-1.2.0/src/sys/unix/selector/{epoll,kqueue}.rs`). tokio 1.52.1 shows it as
  `Ready::is_write_closed`. The test `a_watch_tells_a_close_from_a_half_close_on_a_unix_socket`
  runs it on this Mac: a quiet peer and a half-closed peer are not gone, and a closed
  peer is. A zero-byte write does not work as a probe, because `sock_write_iter`
  returns 0 on Linux before the protocol sees it. **Linux is not run.** The Linux
  rule rests on the mio source and the kernel's `unix_release_sock`, and the cask
  ships only for macOS.
- **The watch holds a duplicate descriptor.** `ready(WRITABLE)` returns at once on a
  healthy socket, so the watch must clear the writable readiness to wait for the
  next event. On the socket that the relay writes to, that clear would stall the
  relay's writes. So `ClientWatch` registers `try_clone_to_owned` of the descriptor
  in its own `AsyncFd`. The cost is one descriptor for each open stream.
- **The relay is two pumps and a third input.** `copy_bidirectional_with_sizes`
  returns only when both directions end and has no input for "the client closed". The
  relay now runs one pump for each direction and a `client_gone` future. The future
  counts only after the client's own end has been copied, so a client that sends a
  last message and closes still delivers it. Two unit tests state this.
- **The client comes back from hyper as a `UnixStream`.** `Upgraded::downcast` gives
  `TokioIo<UnixStream>` and the bytes that hyper read past the request. The server
  writes those bytes to the guest first. An upgraded connection of any other type
  gets an error line and ends. The type cannot occur, because `serve` makes it.
- **The close of the guest connection is a reset when the stream did not end by both
  sides.** A first live run, with an ordinary close, kept the app's descriptors flat
  (43, then 32 after 151 commands) but tart's connections in `CLOSE_WAIT` went from 2
  to 152. They drained about two minutes after the last command. On a throwaway probe against the same guest, five
  half-closed connections stayed in `CLOSE_WAIT` for at least 30 s after an ordinary
  close, and left within 1 s after a close with `SO_LINGER` of 0. A burst of about 250
  commands in two minutes would reach tart's own limit of descriptors. So
  `close_guest` calls `TcpStream::set_zero_linger` when the client has gone or the
  relay failed. It does not when both sides ended, because then the guest has sent its
  own end. I use `set_zero_linger` and not the deprecated `set_linger`, which blocks
  a thread on drop.
- **The app does not raise its soft limit of descriptors.** The limit is 256 by
  launchd default. A higher limit would hide a regression for longer: an app that
  leaks would fail after 1000 commands and not after 120. The fix removes the leak at
  its cause, and the test and the live check catch a regression by a count that is
  flat. The relay uses one more descriptor for each stream now, so the limit allows
  about 80 streams open at once. A command opens one or two.
- **The protocol seam is in `app-vnc-routing`, with the real Host CLI.**
  `many_vnc_commands_leave_the_descriptors_of_the_app_flat` runs 41 `screen size`
  commands against `FakeRfb::start_holding`, which reads the client's end and keeps
  the connection, as tart does. Before the fix the app held 95 descriptors after 41
  commands and 15 after one. With the watch disabled it again held 136 and 16. After
  the fix it passes, three runs in a row. The test measures the descriptors before it
  probes the guest, because a probe byte reaches a stuck relay and frees it. The
  probe is a second write on each held connection, and a reset or a broken pipe on
  it shows that the app closed. One write is not enough on macOS: the first read
  returns end of file, not the reset.
- **The holding guest counts a reset as the client's end.** With the reset in place, the
  app can reset a connection before the fake's thread reads the client's end. The fake
  then counts the reset. This was a race in the fake, not in the app.
- **The live check is `task app:stream-end-live VM=<id>`.** It reads only: 151
  `screen size` commands to a tart guest. It needs the development bundle to hold the
  socket, so a person quits a running app first. I did this for the owner's installed
  app 2.4.0 (pid 72123, and then 35234) twice, when no testanyware command ran, and I
  opened `/Applications/TestAnyware.app` again each time. It adopted both Koine guests
  at once. The guest was `koine-verify-44327`, which was running and which both macOS
  slots left no room to replace.
  - Ordinary close (first run): the app 43 → 32 descriptors, tart `CLOSE_WAIT` 2 → 152.
    FAIL, as a finding, not as a defect of the app.
  - Reset (final): the app 43 → 32 descriptors, tart `CLOSE_WAIT` 1 → 1. PASS. The
    last command printed `1920x1080`.
  - Baseline of the leak, from `plan-k1`: the app 2.4.0 held 254 descriptors after
    about 120 commands.
- **The spec.** The stream rules now list when a stream ends, how the app tells a close
  from a half-close, and the reset. The test seams name the new test, the unit tests of
  the relay and the live check. The open risk of a stall says what the fix covers, and
  names two stalls that it does not: a guest that stops reading while the app writes to
  it, and a guest that goes silent while the client stays. ADR-0004 already says
  "closes that connection when either side closes. A half-close passes through." So
  the ADR stands, and the code now matches it.
- **No review leaf.** The claim that the compiler cannot check is the readiness flag on
  Linux. I could not run Linux, so I name it above instead of cutting a review that
  would have the same limit. The rest is covered by tests that fail without the fix.
- **Results.** `task cli:test`: 1359 tests passed, 0 failed. `task app:test` and
  `task app:routing-test` pass. `task scripts:check` passes. clippy has no finding in
  `testanyware-app`. This change voids the candidate of `f0108944`. The next leaf,
  `release-2-5-0-k3`, rebuilds it.
