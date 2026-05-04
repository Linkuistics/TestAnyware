//! Platform-aware key, modifier, and mouse-button name lookups.
//!
//! Ports `cli/Sources/TestAnywareDriver/Input/PlatformKeymap.swift` to a
//! pure-Rust table-driven form. The X11 keysym constants come from
//! `vendored/royalvnc/Sources/RoyalVNCKit/Input/X11KeySymbols.swift`.
//!
//! The macOS modifier mapping intentionally swaps Cmd and Option:
//! Cmd → `XK_Alt_L (0xffe9)`, Option → `XK_Meta_L (0xffe7)`. This is the
//! mapping required by macOS Tahoe's Virtualization.framework VNC server
//! and is documented in the project memory entry `cmd_key_tahoe`. Do not
//! "fix" the swap without re-checking that note.

use thiserror::Error;

/// Target guest platform. Selects which modifier table is used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Macos,
    Linux,
    Windows,
}

impl Platform {
    /// Parse the lowercase platform name accepted by `--platform` and
    /// `TESTANYWARE_PLATFORM`. Returns `None` for unknown values; the
    /// caller decides whether to fall back to a default or error out.
    pub fn from_name(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "macos" | "mac" | "darwin" => Some(Self::Macos),
            "linux" => Some(Self::Linux),
            "windows" | "win" => Some(Self::Windows),
            _ => None,
        }
    }
}

/// Errors produced by name → keysym / button lookups.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum KeymapError {
    #[error("unknown key: '{0}'")]
    UnknownKey(String),
    #[error("unknown mouse button: '{0}'")]
    UnknownButton(String),
}

// ---- X11 keysym constants we reference --------------------------------------

pub mod xk {
    pub const BACKSPACE: u32 = 0xff08;
    pub const TAB: u32 = 0xff09;
    pub const CLEAR: u32 = 0xff0b;
    pub const RETURN: u32 = 0xff0d;
    pub const ESCAPE: u32 = 0xff1b;
    pub const DELETE: u32 = 0xffff;

    pub const HOME: u32 = 0xff50;
    pub const LEFT: u32 = 0xff51;
    pub const UP: u32 = 0xff52;
    pub const RIGHT: u32 = 0xff53;
    pub const DOWN: u32 = 0xff54;
    pub const PAGE_UP: u32 = 0xff55;
    pub const PAGE_DOWN: u32 = 0xff56;
    pub const END: u32 = 0xff57;
    pub const INSERT: u32 = 0xff63;

    pub const KP_ENTER: u32 = 0xff8d;
    pub const KP_MULTIPLY: u32 = 0xffaa;
    pub const KP_ADD: u32 = 0xffab;
    pub const KP_SEPARATOR: u32 = 0xffac;
    pub const KP_SUBTRACT: u32 = 0xffad;
    pub const KP_DIVIDE: u32 = 0xffaf;
    pub const KP_EQUAL: u32 = 0xffbd;

    pub const F1: u32 = 0xffbe;
    pub const F19: u32 = 0xffd0;

    pub const SHIFT_L: u32 = 0xffe1;
    pub const CONTROL_L: u32 = 0xffe3;
    pub const META_L: u32 = 0xffe7;
    pub const ALT_L: u32 = 0xffe9;
    pub const SUPER_L: u32 = 0xffeb;

    pub const SPACE: u32 = 0x0020;
}

// ---- Special key name → keysym ---------------------------------------------

/// Resolve a key name (case-insensitive) to its X11 keysym. Letters
/// `a`-`z`, digits `0`-`9`, and the symbols listed in
/// [`shifted_char_to_base`] are handled directly; everything else
/// goes through this table.
pub fn key_for_name(name: &str) -> Result<u32, KeymapError> {
    let lower = name.to_ascii_lowercase();
    if let Some(byte) = single_ascii_byte(&lower) {
        if byte.is_ascii_alphanumeric() {
            return Ok(byte as u32);
        }
    }
    match lower.as_str() {
        "return" | "enter" => Ok(xk::RETURN),
        "tab" => Ok(xk::TAB),
        "escape" | "esc" => Ok(xk::ESCAPE),
        "space" => Ok(xk::SPACE),
        "delete" | "backspace" => Ok(xk::BACKSPACE),
        "forwarddelete" => Ok(xk::DELETE),
        "up" => Ok(xk::UP),
        "down" => Ok(xk::DOWN),
        "left" => Ok(xk::LEFT),
        "right" => Ok(xk::RIGHT),
        "home" => Ok(xk::HOME),
        "end" => Ok(xk::END),
        "pageup" => Ok(xk::PAGE_UP),
        "pagedown" => Ok(xk::PAGE_DOWN),
        s if s.starts_with('f') => function_key(s).ok_or(KeymapError::UnknownKey(name.into())),
        _ => Err(KeymapError::UnknownKey(name.into())),
    }
}

fn single_ascii_byte(s: &str) -> Option<u8> {
    let bytes = s.as_bytes();
    if bytes.len() == 1 && bytes[0].is_ascii() {
        Some(bytes[0])
    } else {
        None
    }
}

fn function_key(s: &str) -> Option<u32> {
    let n: u32 = s.strip_prefix('f')?.parse().ok()?;
    if (1..=19).contains(&n) {
        Some(xk::F1 + n - 1)
    } else {
        None
    }
}

// ---- Modifier name → keysym (per platform) ---------------------------------

/// Resolve a modifier name (case-insensitive) to a platform-appropriate
/// keysym. Returns `None` for unknown modifier names so the caller can
/// silently drop them, matching Swift's `compactMap` behaviour.
pub fn modifier_for_name(name: &str, platform: Platform) -> Option<u32> {
    let lower = name.to_ascii_lowercase();
    match platform {
        Platform::Macos => match lower.as_str() {
            // See module-level note on the Cmd/Option swap for macOS
            // Tahoe's Virtualization.framework VNC server.
            "cmd" | "command" => Some(xk::ALT_L),
            "alt" | "option" => Some(xk::META_L),
            "shift" => Some(xk::SHIFT_L),
            "ctrl" | "control" => Some(xk::CONTROL_L),
            _ => None,
        },
        Platform::Linux | Platform::Windows => match lower.as_str() {
            "cmd" | "command" => Some(xk::CONTROL_L),
            "alt" | "option" => Some(xk::ALT_L),
            "shift" => Some(xk::SHIFT_L),
            "ctrl" | "control" => Some(xk::CONTROL_L),
            "super" | "win" => Some(xk::SUPER_L),
            _ => None,
        },
    }
}

/// Resolve all modifier names in order, dropping any unknown entries.
pub fn resolve_modifiers(names: &[&str], platform: Platform) -> Vec<u32> {
    names
        .iter()
        .filter_map(|n| modifier_for_name(n, platform))
        .collect()
}

// ---- Shifted-character map (US layout) -------------------------------------

/// Characters that require Shift on a US keyboard, mapped to their
/// unshifted base ASCII byte. Drives `type_text`'s shifted-symbol path.
pub fn shifted_char_to_base(c: char) -> Option<u8> {
    Some(match c {
        '!' => 0x31,
        '@' => 0x32,
        '#' => 0x33,
        '$' => 0x34,
        '%' => 0x35,
        '^' => 0x36,
        '&' => 0x37,
        '*' => 0x38,
        '(' => 0x39,
        ')' => 0x30,
        '~' => 0x60,
        '_' => 0x2d,
        '+' => 0x3d,
        '{' => 0x5b,
        '}' => 0x5d,
        '|' => 0x5c,
        ':' => 0x3b,
        '"' => 0x27,
        '<' => 0x2c,
        '>' => 0x2e,
        '?' => 0x2f,
        _ => return None,
    })
}

// ---- Mouse buttons ---------------------------------------------------------

/// RFB pointer-event button-mask bit indices (§7.5.5). The mask is a
/// bitfield: bit 0 = left, bit 1 = middle, bit 2 = right; bits 3..6 are
/// wheel directions encoded as transient down+up edges by the host.
pub mod button_bit {
    pub const LEFT: u8 = 0;
    pub const MIDDLE: u8 = 1;
    pub const RIGHT: u8 = 2;
    pub const WHEEL_UP: u8 = 3;
    pub const WHEEL_DOWN: u8 = 4;
    pub const WHEEL_LEFT: u8 = 5;
    pub const WHEEL_RIGHT: u8 = 6;
}

/// Resolve a mouse button name (case-insensitive) to a button-mask bit
/// index. `center` is accepted as an alias for `middle` per Swift parity.
pub fn mouse_button_bit_for_name(name: &str) -> Result<u8, KeymapError> {
    match name.to_ascii_lowercase().as_str() {
        "left" => Ok(button_bit::LEFT),
        "right" => Ok(button_bit::RIGHT),
        "middle" | "center" => Ok(button_bit::MIDDLE),
        _ => Err(KeymapError::UnknownButton(name.into())),
    }
}

// ---- Scroll decomposition --------------------------------------------------

/// One axis-and-magnitude wheel pulse derived from a `(dx, dy)` request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScrollComponent {
    pub direction: ScrollDirection,
    pub steps: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollDirection {
    Up,
    Down,
    Left,
    Right,
}

impl ScrollDirection {
    pub fn button_bit(self) -> u8 {
        match self {
            ScrollDirection::Up => button_bit::WHEEL_UP,
            ScrollDirection::Down => button_bit::WHEEL_DOWN,
            ScrollDirection::Left => button_bit::WHEEL_LEFT,
            ScrollDirection::Right => button_bit::WHEEL_RIGHT,
        }
    }
}

/// Decompose a `(dx, dy)` request into wheel pulses. Y first, then X,
/// matching Swift's `PlatformKeymap.decomposeScroll`. Negative dy means
/// scroll up, positive means down (CLI convention from the Swift code).
pub fn decompose_scroll(dx: i32, dy: i32) -> Vec<ScrollComponent> {
    let mut out = Vec::new();
    if dy < 0 {
        out.push(ScrollComponent {
            direction: ScrollDirection::Up,
            steps: dy.unsigned_abs(),
        });
    } else if dy > 0 {
        out.push(ScrollComponent {
            direction: ScrollDirection::Down,
            steps: dy as u32,
        });
    }
    if dx < 0 {
        out.push(ScrollComponent {
            direction: ScrollDirection::Left,
            steps: dx.unsigned_abs(),
        });
    } else if dx > 0 {
        out.push(ScrollComponent {
            direction: ScrollDirection::Right,
            steps: dx as u32,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_letters_map_to_their_byte_value() {
        assert_eq!(key_for_name("a").unwrap(), b'a' as u32);
        assert_eq!(key_for_name("Z").unwrap(), b'z' as u32);
    }

    #[test]
    fn digits_map_to_their_byte_value() {
        assert_eq!(key_for_name("0").unwrap(), b'0' as u32);
        assert_eq!(key_for_name("9").unwrap(), b'9' as u32);
    }

    #[test]
    fn named_special_keys_resolve() {
        assert_eq!(key_for_name("return").unwrap(), 0xff0d);
        assert_eq!(key_for_name("Enter").unwrap(), 0xff0d);
        assert_eq!(key_for_name("escape").unwrap(), 0xff1b);
        assert_eq!(key_for_name("ESC").unwrap(), 0xff1b);
        assert_eq!(key_for_name("backspace").unwrap(), 0xff08);
        assert_eq!(key_for_name("ForwardDelete").unwrap(), 0xffff);
        assert_eq!(key_for_name("PageUp").unwrap(), 0xff55);
    }

    #[test]
    fn function_keys_in_range() {
        assert_eq!(key_for_name("f1").unwrap(), 0xffbe);
        assert_eq!(key_for_name("f19").unwrap(), 0xffd0);
        assert!(key_for_name("f0").is_err());
        assert!(key_for_name("f20").is_err());
    }

    #[test]
    fn unknown_key_reports_original_name() {
        let err = key_for_name("OnlyAtChristmas").unwrap_err();
        assert!(matches!(err, KeymapError::UnknownKey(s) if s == "OnlyAtChristmas"));
    }

    #[test]
    fn macos_cmd_maps_to_xk_alt_l() {
        // Project memory `cmd_key_tahoe`: Virtualization.framework VNC
        // server requires Cmd → XK_Alt_L on macOS.
        assert_eq!(modifier_for_name("cmd", Platform::Macos).unwrap(), 0xffe9);
        assert_eq!(modifier_for_name("Command", Platform::Macos).unwrap(), 0xffe9);
    }

    #[test]
    fn macos_option_maps_to_xk_meta_l() {
        assert_eq!(modifier_for_name("alt", Platform::Macos).unwrap(), 0xffe7);
        assert_eq!(modifier_for_name("Option", Platform::Macos).unwrap(), 0xffe7);
    }

    #[test]
    fn linux_cmd_maps_to_control() {
        assert_eq!(modifier_for_name("cmd", Platform::Linux).unwrap(), xk::CONTROL_L);
        assert_eq!(modifier_for_name("alt", Platform::Linux).unwrap(), xk::ALT_L);
        assert_eq!(modifier_for_name("super", Platform::Linux).unwrap(), xk::SUPER_L);
    }

    #[test]
    fn windows_super_aliases_win() {
        assert_eq!(modifier_for_name("win", Platform::Windows).unwrap(), xk::SUPER_L);
    }

    #[test]
    fn unknown_modifiers_drop_silently_in_resolve() {
        let mods = resolve_modifiers(&["cmd", "fritters", "shift"], Platform::Macos);
        assert_eq!(mods, vec![xk::ALT_L, xk::SHIFT_L]);
    }

    #[test]
    fn shifted_chars_map_to_base() {
        assert_eq!(shifted_char_to_base('!'), Some(b'1'));
        assert_eq!(shifted_char_to_base('@'), Some(b'2'));
        assert_eq!(shifted_char_to_base('?'), Some(b'/'));
        assert_eq!(shifted_char_to_base('a'), None);
        assert_eq!(shifted_char_to_base('1'), None);
    }

    #[test]
    fn mouse_buttons_resolve() {
        assert_eq!(mouse_button_bit_for_name("left").unwrap(), 0);
        assert_eq!(mouse_button_bit_for_name("MIDDLE").unwrap(), 1);
        assert_eq!(mouse_button_bit_for_name("center").unwrap(), 1);
        assert_eq!(mouse_button_bit_for_name("Right").unwrap(), 2);
        assert!(mouse_button_bit_for_name("forepaw").is_err());
    }

    #[test]
    fn scroll_decomposes_y_then_x() {
        let comps = decompose_scroll(2, -3);
        assert_eq!(
            comps,
            vec![
                ScrollComponent { direction: ScrollDirection::Up, steps: 3 },
                ScrollComponent { direction: ScrollDirection::Right, steps: 2 },
            ]
        );
    }

    #[test]
    fn scroll_zero_deltas_yield_no_components() {
        assert!(decompose_scroll(0, 0).is_empty());
    }

    #[test]
    fn scroll_negative_dx_is_left() {
        let comps = decompose_scroll(-5, 0);
        assert_eq!(
            comps,
            vec![ScrollComponent { direction: ScrollDirection::Left, steps: 5 }]
        );
    }

    #[test]
    fn platform_parses_common_aliases() {
        assert_eq!(Platform::from_name("macos"), Some(Platform::Macos));
        assert_eq!(Platform::from_name("Darwin"), Some(Platform::Macos));
        assert_eq!(Platform::from_name("Linux"), Some(Platform::Linux));
        assert_eq!(Platform::from_name("WINDOWS"), Some(Platform::Windows));
        assert_eq!(Platform::from_name("BeOS"), None);
    }
}
