# 020-viewer-input-forwarding

**Kind:** work

## Goal

Make the viewer **interactive**: forward mouse and keyboard from the egui window
to the guest over RFB. Builds on the render loop and thread/channel skeleton
from leaf 010 — this leaf adds the UI→RFB input producer and the coordinate/
keysym mapping, not a new architecture.

## Done when

- The eframe `update()` reads egui input (pointer position/buttons/scroll, key
  presses/releases, modifiers) and pushes `ViewerInput` events onto the
  `mpsc` channel the RFB thread drains; the `select!` loop calls
  `RfbConnection::{pointer_event, key_event}` accordingly.
- **Coordinate mapping** is correct: egui pointer position (in the displayed
  image's widget rect, accounting for scaling/letterboxing and HiDPI) maps to
  framebuffer pixel `(x, y)` passed to `pointer_event`.
- **Keyboard** maps egui keys/text to RFB keysyms via the existing
  `keymap::{key_for_name, resolve_modifiers, shifted_char_to_base}`; modifiers
  press/release tracked. Mouse buttons via `mouse_button_bit_for_name`; wheel
  via `ScrollComponent`/`ScrollDirection` (transient down+up edges per the
  `pointer_event` button-mask doc).
- Focus handling: input is only forwarded when the viewer window/widget has
  focus; the window does not steal host shortcuts it shouldn't.
- Verified on the macOS primary host against a golden VM: clicking, typing, and
  scrolling in the window land in the guest (cross-check with `agent snapshot`
  or visible UI response). Mind the macOS Cmd/Option keysym remap already in
  `keymap` (Command→XK_Alt_L etc.).

## Notes

- The input layer is **not new** — `key_event`/`pointer_event` and the whole
  `keymap` module already power every `input *` command. This leaf wires egui
  events into them; reuse, don't reinvent.
- Keysym mapping is keyed on the **guest** platform (`keymap::Platform`), not the
  host — resolve the guest platform the same way the `input` command does.
