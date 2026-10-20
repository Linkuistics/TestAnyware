# rfb-cancel-safe-reads-k41

## Goal

Make the reads of the RFB client safe to cancel. A timer that wins a `select!` must not lose the bytes of a message that is half read.

## Context

- `RfbConnection::next_message` in `crates/testanyware-rfb/src/connection.rs` reads a message with several `read_exact` calls and parses as it goes. A future that is dropped after the first call returns has consumed bytes that it cannot give back. The next call then reads the middle of a message as a tag, and fails with `server sent unexpected message type N`.
- Two loops cancel it. `capture_loop` in `crates/testanyware-cli/src/commands/record.rs` races `next_message` against its poll and sample timers. The viewer loop in `connect_and_stream` in `commands/viewer.rs` races it against input and shutdown.
- `cli-app-vnc-routing-k25` found it. A `screen record` through the app failed about once in 25 runs. The same failure appears on the direct path when the server writes an update in 1500-byte pieces with a 3 ms pause. The app only makes a split likelier, because it adds a hop.

## Done when

- A test splits one framebuffer update into small pieces with pauses, and a `select!` that cancels `next_message` between the pieces does not lose a byte. The test fails on the code of today.
- `screen record` and the viewer no longer lose bytes when a timer fires during a message.
- `cli-app-vnc-routing-k25` keeps its recording test with a frame of 16 by 16 pixels. Raise it to the 64 by 48 frame of its other tests, and make the test pass 30 runs in a row.

## Notes

- One cheap shape: a readiness call such as `RfbConnection::readable()` that only waits for the buffered reader to hold a byte (`fill_buf` is safe to cancel). The loops race the timers against that call and then await `next_message` outside the `select!`, so a message is always read to its end. Another shape moves the parse onto a buffer that survives a cancel. Pick the smaller one.
- `testanyware-vm/src/capture.rs` wraps `next_message` in `tokio::time::timeout`. A timeout ends that capture with an error, so a lost byte does no harm there. Check that this stays true.
- This leaf is outside the grove's goal. It sits here because the recording through the app depends on it. The human can prune it or move it with `leaf-insert`.

## Decisions (running log)

- Shape: `RfbConnection::readable()` over `fill_buf`, the smaller one. It sits in an impl block bounded by `AsyncBufRead`, so the generic transports of the other tests keep working. Every real transport is a `BufReader`. Source: tokio 1.52.1 documents `fill_buf` as cancel-safe, and the call cites it.
- The loops race `readable()` and await `next_message()` outside the race. The viewer keeps the shutdown signal racing the read, because shutdown drops the connection anyway. `screen record` has no such race: a server that stalls inside a message holds the sampler until the read ends or fails.
- `capture.rs` did not stay harmless, against the note above. A timeout there ends the wait with `Ok` and the held frame, and `RecoverySession` reuses the connection. A timeout inside a message would have corrupted the next read. The timeout now wraps `readable()` only.
- The test is `testanyware-rfb/tests/cancel_safe_fixture.rs`. It failed on the old shape (a lost byte), and passes now. The routing test failed 1 in 30 runs on the old shape with `unexpected message type 240` at 64 by 48, and 0 in 30 on the new one.
- ADR-0005 states the viewer loop as design, so its bullet now names `readable()`.
