# 020-rfb-encoding-override

**Kind:** work

## Goal

Add a mechanism to **force the RFB client to advertise a single encoding** so
the live-VM gate can make a real VNC server send each of ZRLE, Tight, and Raw,
then assert the decoded framebuffers match. Today the `SetEncodings` preference
list is hard-coded (`testanyware-rfb/src/connection.rs:164`: ZRLE > Tight >
CopyRect > Raw), so the server always picks ZRLE and the Tight/Raw decoders are
never exercised against a live server — only against synthetic fixtures
(`tests/handshake_fixture.rs`, decoder unit tests).

## Context

- `testanyware-rfb/src/connection.rs:164–219` — where the fixed preference list
  is built and `set_encodings()` is sent at handshake. The override hooks here.
- `testanyware-rfb/src/proto.rs:35–45` — encoding constants (`RAW=0`,
  `COPY_RECT=1`, `TIGHT=7`, `ZRLE=16`, pseudo-encodings).
- `testanyware-cli/src/commands/screen.rs` — `screen capture`, the consumer; the
  override must reach it (the gate drives `screen capture`, not the crate
  directly).
- Decoder entry points to exercise: `zrle.rs:50`, `tight.rs:79`.

### Design choices to settle at bootstrap

- **Surface.** Prefer an **env var** (e.g. `TESTANYWARE_RFB_ENCODING=zrle|tight|raw`)
  over a user-facing `--encoding` flag — this is a test/diagnostic seam, not part
  of the CLI design contract surface, so it should not appear in `capabilities`,
  `schema`, or help. Confirm this matches the contract's stance on hidden/internal
  knobs before adding a flag. (CopyRect and the desktop-size/last-rect
  pseudo-encodings should still be advertised alongside the forced primary, so a
  resize or a copyrect-bearing update doesn't break the connection — force the
  *primary* encoding, keep the pseudos.)
- **Validation.** An unknown value should be a clear error, not silently ignored.

## Done when

- Setting the override makes the RFB client advertise only the chosen primary
  encoding (+ required pseudo-encodings) via `SetEncodings`, so a compliant
  server responds with that encoding.
- A unit/fixture test confirms the advertised list changes with the override.
- The override is invisible to the contract surface (no new `capabilities`/help
  entry); `cli-contract.rs` stays green.
- `screen capture` honours the override end-to-end (the gate in `030` relies on
  this to diff ZRLE vs Tight vs Raw captures of a static screen).

## Notes

This is the only genuinely *new* protocol-facing code in the node; the rest of
the gate is orchestration. Keep it minimal — one seam, well-tested.
