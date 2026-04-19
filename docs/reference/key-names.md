# Key Names, Modifiers, and Mouse Buttons

Accepted values for `testanyware input key`, `input key-down`,
`input key-up`, and the `--modifiers` flag.

## Key names

| Category | Accepted values |
|----------|-----------------|
| Letters | `a`, `b`, `c`, ... `z` (case-insensitive; use `--modifiers shift` for uppercase) |
| Digits | `0`, `1`, `2`, ... `9` |
| Return / enter | `return`, `enter` |
| Whitespace / editing | `tab`, `space`, `delete`, `backspace`, `forwarddelete` |
| Escape | `escape`, `esc` |
| Arrows | `up`, `down`, `left`, `right` |
| Navigation | `home`, `end`, `pageup`, `pagedown` |
| Function keys | `f1`, `f2`, `f3`, ... `f19` |

Key names are case-insensitive. Unknown names raise `PlatformKeymapError.unknownKey`.

## Modifiers

Passed to `testanyware input key` as `--modifiers cmd,shift,alt` (comma
separated, no spaces). Values:

| Value | Alias | Mapped to (per `--platform`) |
|-------|-------|------------------------------|
| `cmd` | `command` | Command (macOS) / Win (Windows) / Super (Linux) |
| `alt` | `option` | Option (macOS) / Alt (Windows/Linux) |
| `shift` | — | Shift on all platforms |
| `ctrl` | `control` | Control on all platforms |

The CLI maps these to the correct keycodes for the target platform via
`--platform <macos|windows|linux>`.

## Mouse buttons

Accepted values for `--button`:

| Value | Alias | Meaning |
|-------|-------|---------|
| `left` | — | Primary button (VNC button 1) |
| `right` | — | Secondary button (VNC button 3) |
| `middle` | `center` | Middle / wheel button (VNC button 2) |

Unknown button names raise `PlatformKeymapError.unknownButton`.
