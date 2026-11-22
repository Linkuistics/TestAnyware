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

## Decisions (running log)

- 2026-11-22, reproduction. A scratch probe in Python reads tart's VNC
  server with the handshake of `testanyware-rfb`. It shares no code with
  the client: it inflates with `zlib` of Python and checks each tile
  stream with its own parser. On a fresh clone of a never-booted 27.2.1
  VM, the probe saw the same fault. So the fault is not in `zrle.rs` or
  in the inflate of `flate2`.
- 2026-11-22, root cause. tart's VNC server sends a rectangle whose header
  is larger than the framebuffer, and it encodes only the part inside the
  framebuffer. The recorded sequence of the first boot:
  - ServerInit gives 1280x720. Update 1 is a ZRLE rectangle of 1280x720.
  - Update 2 is `DesktopSize` 1024x768. Update 3 is a ZRLE rectangle of
    1024x768.
  - Update 4 has two ZRLE rectangles. Rectangle 0 has the header
    `0,0 1280x720`. Rectangle 1 has the header `0,720 1024x48`.
  - The 1086 inflated bytes of rectangle 0 parse exactly as a tile stream
    of 1024x720, and they underrun as 1280x720. The 64 bytes of
    rectangle 1 parse as 1024x48. Together the two rectangles cover
    1024x768.
  - Every later rectangle decoded cleanly. So the zlib stream stayed in
    step, and no new zlib stream started.
  The server clips the pixel data to the new framebuffer, but it keeps
  the unclipped damage rectangle in the header. The two other hypotheses
  of the context are false.
- 2026-11-22, timing. A second fresh boot gave the same messages up to
  update 3, and no stale rectangle. In the first boot the screen changed
  after the resize, and in the second boot it did not. So the fault needs
  a redraw soon after the `DesktopSize` change, and it is intermittent.
  The stale size of ServerInit is the stale size of `first-frame-size-k9`.
- 2026-11-22, fix. `read_framebuffer_update` clips a ZRLE rectangle to the
  framebuffer before it decodes the tiles, and it writes only the clipped
  part. The length prefix of ZRLE keeps the stream in step whatever the
  size. So a wrong clip fails loudly, as an underrun or as trailing bytes.
  The fixture test `stale_rect_fixture.rs` plays the recorded sequence. It
  failed with `ZRLE: tile stream ended mid-pixel` before the fix, and it
  passes after it.
- 2026-11-22, Raw and Tight stay unchanged. Three fresh boots with Raw
  forced saw no stale rectangle, so the server encoding of a stale Raw or
  Tight rectangle is not observed. Raw has no length prefix, and a clip by
  guess could misread the stream with no error. Today such a rectangle
  fails loudly in `raw_rect`. The default encoding is ZRLE, and the base
  build uses it.
- 2026-11-22, live run from the agent session. `task vm:setup-assistant-live
  FRESH=zrle-fresh-27.2.1` drove all 24 steps of Setup Assistant to the
  desktop, opened Terminal and enabled Remote Login. Then SSH to the guest
  failed with `No route to host` (errno 65). ADR-0019 records that error
  as the Local Network denial of a process without the grant. The agent
  session has no grant, so this gate needs a run from a shell that has
  one. `task cli:test` passed, with 1725 tests and none failed.
- 2026-11-22, live run from Terminal by the owner. Setup Assistant
  reached `skip Apple Account (confirm)`, then matched `Apple Account
  sign-in` again. The driver ran the actions of that page again, and it
  failed after 30 s: "never showed Other Sign-In Options". Its last OCR
  read was the Terms and Conditions page. The ZRLE fault did not occur.
- 2026-11-22, second root cause. After the driver acts on a screen, it
  waits until another screen matches. When a sheet closes, the page under
  it shows for a moment before the next page. That page is another match,
  so the wait ends, and the next pass runs the actions of the page again.
  The same race applies after the licence sheet, the Location Services
  sheet and account creation. Whether OCR catches the stale frame decides
  the run, so the first live run passed this step.
- 2026-11-22, second fix. The type `Walk` in `setup_assistant.rs` records
  the screens that the driver acted on. A screen acts again only when it
  is the last screen acted on, which keeps the retry after `LEAVE_WAIT`.
  An earlier screen that shows again is a stale view, and the driver polls
  again. A stale view is not progress, so a page that really shows again
  fails after `STALL_WAIT` with its OCR text. The unit test
  `a_page_that_shows_again_after_its_sheet_is_not_acted_on_again` failed
  with the old behaviour, and it passes with the fix. This work stays in
  this leaf, because the goal is that a 27.2.1 base build passes Setup
  Assistant.
- 2026-11-22, live gate passed. With both fixes, `task
  vm:setup-assistant-live FRESH=zrle-fresh-27.2.1` passed from the agent
  session in 423 s. It covered Setup Assistant to the desktop, Remote
  Login, SSH and the base checks. This run used the same session, so the
  earlier `No route to host` was not the missing Local Network grant
  alone. Its cause is unknown, and it did not occur again. I deleted the
  never-booted VM and its clone.
