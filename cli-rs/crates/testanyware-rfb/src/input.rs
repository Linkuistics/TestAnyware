//! High-level VNC input helpers: `press_key`, `type_text`, `click`,
//! `drag`, `scroll`, and the down/up primitives.
//!
//! Ports `cli/Sources/TestAnywareDriver/Input/VNCInput.swift`. The Swift
//! `useRawKeysyms` env-var branch collapses here because we don't go
//! through RoyalVNCKit's ARD-remapping layer — every keysym is sent as
//! supplied.
//!
//! Inter-event sleeps mirror Swift's 50 ms pauses around modifier
//! transitions; some guests drop the modifier-down before the keystroke
//! arrives if these are removed.

use std::time::Duration;

use tokio::io::{AsyncRead, AsyncWrite};

use crate::connection::RfbConnection;
use crate::error::RfbError;
use crate::keymap::{
    decompose_scroll, key_for_name, mouse_button_bit_for_name, resolve_modifiers,
    shifted_char_to_base, xk, KeymapError, Platform,
};

const MODIFIER_SETTLE: Duration = Duration::from_millis(50);

/// Errors specific to the high-level input helpers. Wraps both the
/// keymap layer (name lookups) and the connection layer (wire I/O) so
/// CLI handlers can map each variant to a stable §4 error code.
#[derive(Debug, thiserror::Error)]
pub enum InputError {
    #[error(transparent)]
    Keymap(#[from] KeymapError),
    #[error(transparent)]
    Rfb(#[from] RfbError),
}

impl<T: AsyncRead + AsyncWrite + Unpin> RfbConnection<T> {
    /// Press a named key (with optional modifiers) and release it.
    /// Equivalent to a tap — the modifiers are released afterwards too.
    pub async fn press_key(
        &mut self,
        key: &str,
        modifier_names: &[&str],
        platform: Platform,
    ) -> Result<(), InputError> {
        let keysym = key_for_name(key)?;
        let mods = resolve_modifiers(modifier_names, platform);
        self.send_modifier_chord(&mods, &[keysym]).await?;
        Ok(())
    }

    /// Send a key-down for a named key, leaving it held.
    pub async fn key_down_named(&mut self, key: &str) -> Result<(), InputError> {
        let keysym = key_for_name(key)?;
        self.key_event(keysym, true).await?;
        Ok(())
    }

    /// Send a key-up for a named key.
    pub async fn key_up_named(&mut self, key: &str) -> Result<(), InputError> {
        let keysym = key_for_name(key)?;
        self.key_event(keysym, false).await?;
        Ok(())
    }

    /// Type a string by sending per-character key events.
    ///
    /// - Uppercase letters and US-shifted symbols are sent with Shift
    ///   bracketing the keystroke.
    /// - Other printable characters are sent as their Unicode scalar
    ///   value (RFB keysyms are X11 keysyms, which include direct
    ///   Unicode mapping for the Latin-1 range).
    /// - Newline characters are skipped (matching Swift parity); the
    ///   caller can issue an explicit `press_key("return", ..)` if
    ///   needed.
    pub async fn type_text(&mut self, text: &str) -> Result<(), InputError> {
        for ch in text.chars() {
            if ch == '\n' || ch == '\r' {
                continue;
            }
            if ch.is_uppercase() {
                let lower: Vec<char> = ch.to_lowercase().collect();
                let keysyms: Vec<u32> = lower.iter().map(|&c| c as u32).collect();
                self.send_modifier_chord(&[xk::SHIFT_L], &keysyms).await?;
            } else if let Some(base) = shifted_char_to_base(ch) {
                self.send_modifier_chord(&[xk::SHIFT_L], &[base as u32]).await?;
            } else {
                let keysym = ch as u32;
                self.key_event(keysym, true).await?;
                self.key_event(keysym, false).await?;
            }
        }
        Ok(())
    }

    /// Click at framebuffer coordinates. `count` is the number of full
    /// down/up cycles to issue (default 1).
    pub async fn click(
        &mut self,
        x: u16,
        y: u16,
        button: &str,
        count: u32,
    ) -> Result<(), InputError> {
        let bit = mouse_button_bit_for_name(button)?;
        let mask = 1u8 << bit;
        for _ in 0..count {
            self.pointer_event(mask, x, y).await?;
            self.pointer_event(0, x, y).await?;
        }
        Ok(())
    }

    /// Press a mouse button (no release) at coordinates.
    pub async fn mouse_down(
        &mut self,
        x: u16,
        y: u16,
        button: &str,
    ) -> Result<(), InputError> {
        let bit = mouse_button_bit_for_name(button)?;
        self.pointer_event(1u8 << bit, x, y).await?;
        Ok(())
    }

    /// Release a mouse button at coordinates.
    pub async fn mouse_up(
        &mut self,
        x: u16,
        y: u16,
        _button: &str,
    ) -> Result<(), InputError> {
        // RFB pointer events carry a mask of currently-held buttons,
        // not the released one — mouse_up sends mask=0 to indicate
        // "nothing held". The Swift API takes `_button` purely for
        // symmetry with `mouse_down`; we accept it but do not validate
        // it because there is no other useful behaviour to invoke.
        self.pointer_event(0, x, y).await?;
        Ok(())
    }

    /// Move the pointer without changing button state.
    pub async fn mouse_move(&mut self, x: u16, y: u16) -> Result<(), InputError> {
        self.pointer_event(0, x, y).await?;
        Ok(())
    }

    /// Scroll wheel pulses at coordinates. Y first, then X — matching
    /// Swift parity. Convention: `dy < 0` scrolls up, `dy > 0` scrolls
    /// down.
    pub async fn scroll(
        &mut self,
        x: u16,
        y: u16,
        dx: i32,
        dy: i32,
    ) -> Result<(), InputError> {
        for component in decompose_scroll(dx, dy) {
            let mask = 1u8 << component.direction.button_bit();
            for _ in 0..component.steps {
                // Each "step" is a transient down+up edge of the wheel
                // direction bit. Some guests need both edges to register
                // a single click of the wheel.
                self.pointer_event(mask, x, y).await?;
                self.pointer_event(0, x, y).await?;
            }
        }
        Ok(())
    }

    /// Drag from `(from_x, from_y)` to `(to_x, to_y)` with `steps`
    /// interpolation points. `steps` is clamped to at least 1.
    pub async fn drag(
        &mut self,
        from_x: u16,
        from_y: u16,
        to_x: u16,
        to_y: u16,
        button: &str,
        steps: u32,
    ) -> Result<(), InputError> {
        let bit = mouse_button_bit_for_name(button)?;
        let mask = 1u8 << bit;
        self.pointer_event(mask, from_x, from_y).await?;

        let n = steps.max(1);
        for i in 1..=n {
            let t = i as f64 / n as f64;
            let x = lerp(from_x, to_x, t);
            let y = lerp(from_y, to_y, t);
            self.pointer_event(mask, x, y).await?;
        }
        self.pointer_event(0, to_x, to_y).await?;
        Ok(())
    }

    // ---- internal helpers -------------------------------------------------

    /// Press each modifier, then send each `keysym` as a quick
    /// down+up, then release the modifiers in reverse order. The 50 ms
    /// pauses around the modifier transitions match Swift parity.
    async fn send_modifier_chord(
        &mut self,
        modifiers: &[u32],
        keysyms: &[u32],
    ) -> Result<(), InputError> {
        for &m in modifiers {
            self.key_event(m, true).await?;
        }
        if !modifiers.is_empty() {
            tokio::time::sleep(MODIFIER_SETTLE).await;
        }
        for &k in keysyms {
            self.key_event(k, true).await?;
            self.key_event(k, false).await?;
        }
        if !modifiers.is_empty() {
            tokio::time::sleep(MODIFIER_SETTLE).await;
        }
        for &m in modifiers.iter().rev() {
            self.key_event(m, false).await?;
        }
        Ok(())
    }
}

fn lerp(a: u16, b: u16, t: f64) -> u16 {
    let af = a as f64;
    let bf = b as f64;
    (af + (bf - af) * t) as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lerp_endpoints_exact() {
        assert_eq!(lerp(100, 400, 0.0), 100);
        assert_eq!(lerp(100, 400, 1.0), 400);
    }

    #[test]
    fn lerp_midpoint_truncates_toward_zero() {
        assert_eq!(lerp(100, 401, 0.5), 250); // 250.5 → 250 (f64-as-cast truncates)
    }
}
