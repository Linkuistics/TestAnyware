//! Framebuffer that stores pixels as RGBA bytes.
//!
//! The connection negotiates a 32 bpp little-endian pixel format with
//! `B` at byte 0, `G` at byte 1, `R` at byte 2 and a padding byte at
//! byte 3. The framebuffer translates that into RGBA on write, so its
//! internal buffer is directly consumable by image-encoding crates
//! (e.g. PNG).

use crate::error::RfbError;

/// Owned RGBA8 framebuffer.
#[derive(Debug, Clone)]
pub struct Framebuffer {
    width: u32,
    height: u32,
    /// Length is `width * height * 4`; layout is row-major RGBA, alpha
    /// always 0xFF for surfaces without per-pixel alpha (RFB has no
    /// concept of alpha).
    pixels: Vec<u8>,
}

impl Framebuffer {
    /// Allocate a fresh framebuffer; pixels start fully opaque black.
    pub fn new(width: u32, height: u32) -> Result<Self, RfbError> {
        if width == 0 || height == 0 {
            return Err(RfbError::InvalidFramebufferSize { width, height });
        }
        let len = (width as usize)
            .checked_mul(height as usize)
            .and_then(|n| n.checked_mul(4))
            .ok_or(RfbError::InvalidFramebufferSize { width, height })?;
        let mut pixels = vec![0u8; len];
        // Initialise alpha lane to 0xFF so undrawn regions render as
        // fully opaque rather than fully transparent in image viewers.
        for chunk in pixels.chunks_exact_mut(4) {
            chunk[3] = 0xFF;
        }
        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    /// Borrow the RGBA buffer.
    pub fn rgba(&self) -> &[u8] {
        &self.pixels
    }

    /// Apply a Raw-encoded rectangle. `pixels` length must be
    /// `width * height * 4`; channel layout is the negotiated 4-byte
    /// LE format (B, G, R, X).
    pub fn raw_rect(
        &mut self,
        x: u32,
        y: u32,
        w: u32,
        h: u32,
        pixels: &[u8],
    ) -> Result<(), RfbError> {
        let expected = (w as usize) * (h as usize) * 4;
        if pixels.len() != expected {
            return Err(RfbError::Protocol(format!(
                "Raw rect at ({x},{y}) {w}x{h}: expected {expected} bytes, got {}",
                pixels.len()
            )));
        }
        if x.saturating_add(w) > self.width || y.saturating_add(h) > self.height {
            return Err(RfbError::Protocol(format!(
                "Raw rect at ({x},{y}) {w}x{h} extends past framebuffer {}x{}",
                self.width, self.height
            )));
        }
        let stride = self.width as usize * 4;
        for row in 0..(h as usize) {
            let src_row_start = row * (w as usize) * 4;
            let src_row_end = src_row_start + (w as usize) * 4;
            let src = &pixels[src_row_start..src_row_end];
            let dst_row_start = (y as usize + row) * stride + (x as usize) * 4;
            for col in 0..(w as usize) {
                let s = &src[col * 4..col * 4 + 4];
                // Source bytes (LE 32-bit BGRX): B=s[0], G=s[1], R=s[2].
                let dst = dst_row_start + col * 4;
                self.pixels[dst] = s[2]; // R
                self.pixels[dst + 1] = s[1]; // G
                self.pixels[dst + 2] = s[0]; // B
                self.pixels[dst + 3] = 0xFF; // A
            }
        }
        Ok(())
    }

    /// Apply a CopyRect update. Source and destination are both within
    /// this framebuffer; the rectangle is copied in a direction-safe
    /// manner so overlapping rectangles do not corrupt themselves.
    pub fn copy_rect(
        &mut self,
        dst_x: u32,
        dst_y: u32,
        src_x: u32,
        src_y: u32,
        w: u32,
        h: u32,
    ) -> Result<(), RfbError> {
        if dst_x.saturating_add(w) > self.width
            || dst_y.saturating_add(h) > self.height
            || src_x.saturating_add(w) > self.width
            || src_y.saturating_add(h) > self.height
        {
            return Err(RfbError::Protocol(format!(
                "CopyRect ({src_x},{src_y})→({dst_x},{dst_y}) {w}x{h} out of bounds {}x{}",
                self.width, self.height
            )));
        }
        let stride = self.width as usize * 4;
        let row_bytes = (w as usize) * 4;
        // Walk rows top→bottom or bottom→top depending on whether the
        // destination is below or above the source, so an overlapping
        // copy does not overwrite data it has not yet read.
        let rows: Box<dyn Iterator<Item = usize>> = if dst_y > src_y {
            Box::new((0..h as usize).rev())
        } else {
            Box::new(0..h as usize)
        };
        for row in rows {
            let src_off = (src_y as usize + row) * stride + (src_x as usize) * 4;
            let dst_off = (dst_y as usize + row) * stride + (dst_x as usize) * 4;
            // copy_within handles overlap on the same row safely.
            self.pixels.copy_within(src_off..src_off + row_bytes, dst_off);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_zero_size_rejected() {
        assert!(matches!(
            Framebuffer::new(0, 100),
            Err(RfbError::InvalidFramebufferSize { .. })
        ));
        assert!(matches!(
            Framebuffer::new(100, 0),
            Err(RfbError::InvalidFramebufferSize { .. })
        ));
    }

    #[test]
    fn new_initialises_alpha_to_opaque() {
        let fb = Framebuffer::new(2, 1).unwrap();
        assert_eq!(fb.rgba()[3], 0xFF);
        assert_eq!(fb.rgba()[7], 0xFF);
    }

    #[test]
    fn raw_rect_fills_pixels_in_rgba_order() {
        let mut fb = Framebuffer::new(2, 1).unwrap();
        // Two BGRX pixels: pixel 0 = red (B=0,G=0,R=255), pixel 1 =
        // green (B=0,G=255,R=0).
        let pixels = [0, 0, 255, 0, 0, 255, 0, 0];
        fb.raw_rect(0, 0, 2, 1, &pixels).unwrap();
        let rgba = fb.rgba();
        assert_eq!(&rgba[0..4], &[255, 0, 0, 0xFF], "pixel 0 RGBA");
        assert_eq!(&rgba[4..8], &[0, 255, 0, 0xFF], "pixel 1 RGBA");
    }

    #[test]
    fn raw_rect_rejects_wrong_byte_count() {
        let mut fb = Framebuffer::new(2, 2).unwrap();
        let too_small = [0u8; 7];
        assert!(matches!(
            fb.raw_rect(0, 0, 2, 1, &too_small),
            Err(RfbError::Protocol(_))
        ));
    }

    #[test]
    fn raw_rect_rejects_out_of_bounds() {
        let mut fb = Framebuffer::new(4, 4).unwrap();
        let pixels = vec![0u8; 4 * 4];
        assert!(matches!(
            fb.raw_rect(3, 3, 2, 2, &pixels),
            Err(RfbError::Protocol(_))
        ));
    }

    #[test]
    fn copy_rect_moves_pixels_down() {
        let mut fb = Framebuffer::new(4, 4).unwrap();
        // Paint row 0 red.
        let red_row = [0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255, 0];
        fb.raw_rect(0, 0, 4, 1, &red_row).unwrap();
        // Copy row 0 down to row 2.
        fb.copy_rect(0, 2, 0, 0, 4, 1).unwrap();
        let stride = 4 * 4;
        let row2_start = 2 * stride;
        assert_eq!(&fb.rgba()[row2_start..row2_start + 4], &[255, 0, 0, 0xFF]);
    }

    #[test]
    fn copy_rect_handles_overlap_downward() {
        // 4-row framebuffer; row 0 red, others black. Copy rows 0..2
        // down by one (so destination overlaps source). We expect rows
        // 0 and 1 to become red without the source being clobbered
        // mid-iteration.
        let mut fb = Framebuffer::new(2, 4).unwrap();
        let red_block = [0, 0, 255, 0, 0, 0, 255, 0]; // 2 px red
        fb.raw_rect(0, 0, 2, 1, &red_block).unwrap();
        fb.copy_rect(0, 1, 0, 0, 2, 2).unwrap();
        // After the copy: row 0 red (unchanged source), row 1 red
        // (copied from old row 0), row 2 red (copied from old row 1
        // which started black — wait, our test paints only row 0).
        let row = |i: usize| {
            let s = i * 2 * 4;
            fb.rgba()[s..s + 8].to_vec()
        };
        // Row 1 should now be red (came from old row 0).
        assert_eq!(row(1), &[255, 0, 0, 0xFF, 255, 0, 0, 0xFF]);
        // Row 2 should be black (came from old row 1, which was black).
        assert_eq!(row(2), &[0, 0, 0, 0xFF, 0, 0, 0, 0xFF]);
    }

    #[test]
    fn copy_rect_rejects_out_of_bounds_source() {
        let mut fb = Framebuffer::new(4, 4).unwrap();
        assert!(matches!(
            fb.copy_rect(0, 0, 3, 3, 2, 2),
            Err(RfbError::Protocol(_))
        ));
    }
}
