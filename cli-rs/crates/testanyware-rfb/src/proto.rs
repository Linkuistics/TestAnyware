//! Wire-level constants and structures for RFB 3.8.
//!
//! Layout follows RFC 6143. Multi-byte integers are big-endian on the
//! wire unless stated otherwise.

/// Twelve-byte protocol-version greeting we negotiate with the server.
pub const PROTOCOL_VERSION_3_8: &[u8; 12] = b"RFB 003.008\n";

/// Server-to-client message types (§7.6).
pub mod server_msg {
    pub const FRAMEBUFFER_UPDATE: u8 = 0;
    pub const SET_COLOUR_MAP_ENTRIES: u8 = 1;
    pub const BELL: u8 = 2;
    pub const SERVER_CUT_TEXT: u8 = 3;
}

/// Client-to-server message types (§7.5).
pub mod client_msg {
    pub const SET_PIXEL_FORMAT: u8 = 0;
    pub const SET_ENCODINGS: u8 = 2;
    pub const FRAMEBUFFER_UPDATE_REQUEST: u8 = 3;
    pub const KEY_EVENT: u8 = 4;
    pub const POINTER_EVENT: u8 = 5;
}

/// Security types (§7.1.2).
pub mod sec_type {
    pub const INVALID: u8 = 0;
    pub const NONE: u8 = 1;
    pub const VNC_AUTH: u8 = 2;
}

/// Encoding type codes (§7.7). Values are signed 32-bit on the wire to
/// allow negative codes for pseudo-encodings.
pub mod encoding {
    pub const RAW: i32 = 0;
    pub const COPY_RECT: i32 = 1;
    // Pseudo-encodings (negotiated via SetEncodings, but represent
    // capabilities rather than rectangle data). Not decoded by this
    // foundation but included for completeness so SetEncodings is
    // future-proof.
    pub const PSEUDO_DESKTOP_SIZE: i32 = -223;
    pub const PSEUDO_LAST_RECT: i32 = -224;
}

/// Pixel format negotiated between client and server (§7.4). Sixteen
/// bytes on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelFormat {
    pub bits_per_pixel: u8,
    pub depth: u8,
    /// 0 = pixel is little-endian on the wire (what we always request).
    pub big_endian: u8,
    /// 1 = true colour (no colour map). Always 1 for our negotiated
    /// format.
    pub true_colour: u8,
    pub red_max: u16,
    pub green_max: u16,
    pub blue_max: u16,
    pub red_shift: u8,
    pub green_shift: u8,
    pub blue_shift: u8,
}

impl PixelFormat {
    /// 32 bpp, 24-depth, true-colour, little-endian-on-wire, with
    /// channels packed B(0..7) G(8..15) R(16..23) X(24..31). When read
    /// out as little-endian u32 words this gives BGRX byte order in
    /// memory, which the framebuffer converts to RGBA when writing
    /// into its own buffer.
    pub const fn rgba32_le() -> Self {
        Self {
            bits_per_pixel: 32,
            depth: 24,
            big_endian: 0,
            true_colour: 1,
            red_max: 255,
            green_max: 255,
            blue_max: 255,
            red_shift: 16,
            green_shift: 8,
            blue_shift: 0,
        }
    }

    pub fn encode(&self) -> [u8; 16] {
        let mut out = [0u8; 16];
        out[0] = self.bits_per_pixel;
        out[1] = self.depth;
        out[2] = self.big_endian;
        out[3] = self.true_colour;
        out[4..6].copy_from_slice(&self.red_max.to_be_bytes());
        out[6..8].copy_from_slice(&self.green_max.to_be_bytes());
        out[8..10].copy_from_slice(&self.blue_max.to_be_bytes());
        out[10] = self.red_shift;
        out[11] = self.green_shift;
        out[12] = self.blue_shift;
        // bytes 13..16 are padding
        out
    }

    pub fn decode(bytes: &[u8; 16]) -> Self {
        Self {
            bits_per_pixel: bytes[0],
            depth: bytes[1],
            big_endian: bytes[2],
            true_colour: bytes[3],
            red_max: u16::from_be_bytes([bytes[4], bytes[5]]),
            green_max: u16::from_be_bytes([bytes[6], bytes[7]]),
            blue_max: u16::from_be_bytes([bytes[8], bytes[9]]),
            red_shift: bytes[10],
            green_shift: bytes[11],
            blue_shift: bytes[12],
        }
    }

    /// Byte offsets of (B, G, R) channels within a 4-byte little-endian
    /// pixel for the format we negotiate. Used by the framebuffer to
    /// translate RFB-format pixels into RGBA storage.
    pub fn rgba32_le_channel_offsets() -> (usize, usize, usize) {
        // For LE u32: shift 0 → byte 0, shift 8 → byte 1, shift 16 → byte 2.
        let pf = Self::rgba32_le();
        (
            (pf.red_shift / 8) as usize,
            (pf.green_shift / 8) as usize,
            (pf.blue_shift / 8) as usize,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgba32_le_roundtrip() {
        let pf = PixelFormat::rgba32_le();
        let bytes = pf.encode();
        let decoded = PixelFormat::decode(&bytes);
        assert_eq!(pf, decoded);
    }

    #[test]
    fn rgba32_le_channel_offsets_match_layout() {
        // BGRX in memory means B at byte 0, G at 1, R at 2, X at 3.
        let (r, g, b) = PixelFormat::rgba32_le_channel_offsets();
        assert_eq!(b, 0);
        assert_eq!(g, 1);
        assert_eq!(r, 2);
    }

    #[test]
    fn rgba32_le_byte_layout_matches_wire_documentation() {
        // The byte layout has to be stable across releases; agents on
        // the other end of the wire will assume this exact format.
        let bytes = PixelFormat::rgba32_le().encode();
        assert_eq!(bytes[0], 32, "bits_per_pixel");
        assert_eq!(bytes[1], 24, "depth");
        assert_eq!(bytes[2], 0, "big_endian");
        assert_eq!(bytes[3], 1, "true_colour");
        assert_eq!(&bytes[4..6], &[0, 255], "red_max");
        assert_eq!(&bytes[6..8], &[0, 255], "green_max");
        assert_eq!(&bytes[8..10], &[0, 255], "blue_max");
        assert_eq!(bytes[10], 16, "red_shift");
        assert_eq!(bytes[11], 8, "green_shift");
        assert_eq!(bytes[12], 0, "blue_shift");
    }
}
