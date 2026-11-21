# setup-assistant-zrle-fault-k10

## Goal

Find the root cause of the ZRLE fault at Setup Assistant in the macOS 27.2.1
base build. Fix it. Then a base build from the IPSW of
27.2.1 can pass Setup Assistant.

## Context

- On 2026-11-22, `task vm:refresh-golden-macos` of `macos-golden-refresh-k7`
  planned 27.2.1 (26B131), downloaded its IPSW and installed it into
  `testanyware-base-macos-27.2.1-26B131-building`. The boot to Setup
  Assistant then failed:
  `recovery: RFB error: protocol violation: ZRLE: tile stream ended mid-pixel`.
  The build deleted the `-building` VM. The log is in the scratch directory
  of that session, not in the repository.
- The error comes from `cli-rs/crates/testanyware-rfb/src/zrle.rs`: the
  inflated bytes of a rectangle ended before its tiles did.
- The failing client is the `RecoverySession` of `configure_fresh_vm` in
  `ipsw_base.rs`. It is the first VNC client of a fresh install.
  `capture_frame` in `testanyware-vm/src/capture.rs` drives it.
- Checked in k7, and not the cause on their own:
  - `read_framebuffer_update` in `connection.rs` reads each ZRLE blob whole
    by its length prefix, and it inflates every ZRLE rectangle. No path
    skips a rectangle of the persistent zlib stream.
  - `refresh_full_frame` in `capture.rs` races only `readable()`, which is
    cancel-safe, and then it reads each message to its end.
- Hypotheses, none tested:
  - The server starts a new zlib stream, for example after a
    `DesktopSize` change, and the persistent inflater does not expect it.
  - The server encodes with a pixel format other than the one the client
    set, so the CPIXEL width is wrong.
  - `inflate_rect` stops early, so a rectangle leaves bytes in the stream.
- `first-frame-size-k9` reports a stale frame size at the first VNC
  connection after a tart boot. The keeper was the first client of each
  guest until `keeper-removal-code-k5`. The last good base build ran on
  2026-10-18, with 27.0.1, before `rfb-cancel-safe-reads-k41`.
- The IPSW `macos-27.2.1-26B131.ipsw` is in the cache, so a fresh install
  needs no download. `tart create <vm> --from-ipsw <cached ipsw>` makes a
  never-booted VM in about 4 minutes. `task vm:setup-assistant-live
  FRESH=<vm>` then drives Setup Assistant on a clone of it.
- `TESTANYWARE_RFB_ENCODING=raw` or `tight` forces one encoding. A run with
  each one tells whether the fault is ZRLE alone.

## Done when

- The root cause is stated with the evidence that shows it, in the running
  log.
- A test fails before the fix and passes after it. If the cause is in the
  RFB client, the test is a unit test or an RFB fixture test. It feeds the
  client the server behaviour that caused the fault.
- `task vm:setup-assistant-live FRESH=<a never-booted 27.2.1 VM>` passes.
- The existing suites pass, and no suite gains a compatibility case.

## Notes

- Use the `systematic-debugging` skill. No fix before the root cause.
- Keep instrumentation and logs in scratch, not in the repository.
- Delete the never-booted VM and its clones when they are no longer needed.
- The other live macOS work of this grove waits for this fix:
  `macos-golden-refresh-run-k11` builds the base again.
