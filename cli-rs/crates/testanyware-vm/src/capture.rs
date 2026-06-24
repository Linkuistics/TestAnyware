//! Shared RFB frame-capture + PNG encode (platform-neutral).
//!
//! The frame-refresh snapshot step — *request a fresh full-frame update →
//! drain `next_message` until a non-empty `FramebufferUpdated` → clone the
//! framebuffer → encode to PNG* — was inlined in
//! `commands/screen.rs::run_screen_find_text`. ADR-0008 extracts it here so
//! the macOS recovery driver ([`crate::recovery`]) and `screen find-text`
//! share one implementation rather than duplicating the fiddly drain loop.
//!
//! This module is **not** macOS-gated: `screen find-text` runs on every
//! platform, so the capture pipeline must too. Only [`crate::recovery`] (which
//! also drives RFB *input* over the recovery framebuffer) is macOS-only.
//!
//! The helpers are generic over the RFB transport `T` so both the live TCP
//! connection and test fixtures can drive them.

use std::time::Duration;

use testanyware_rfb::{Framebuffer, RfbConnection, RfbError, ServerEvent};
use tokio::io::{AsyncRead, AsyncWrite};

/// Upper bound on how long [`capture_frame`] waits for the server to answer a
/// full-frame request before returning the framebuffer it already holds.
///
/// A `FramebufferUpdateRequest` is normally answered within milliseconds, so on
/// a live, changing screen this never trips and behaviour is unchanged. But a
/// **static** screen (the recovery desktop sitting idle, ADR-0008) can leave
/// the server with nothing to send after a no-op update — and a bare
/// `next_message().await` would then block *forever*. Bounding the wait makes
/// capture return the last-known frame instead of hanging; the caller's own
/// poll loop (`wait_for_text` / `screen find-text --timeout`) governs retries.
const CAPTURE_MSG_TIMEOUT: Duration = Duration::from_secs(5);

/// A failure during [`capture_frame_png`]: either the RFB exchange or the
/// PNG encode. Kept as two arms (rather than collapsing to one string) so the
/// CLI's `screen` handlers preserve their distinct §4 error mapping —
/// `CONNECTION_REFUSED`/`INTERNAL` for RFB, `INTERNAL` for encode.
#[derive(Debug, thiserror::Error)]
pub enum CaptureError {
    #[error(transparent)]
    Rfb(#[from] RfbError),
    #[error("PNG encode failed: {0}")]
    Encode(#[from] image::ImageError),
}

/// Request a fresh **full-frame** update on the live connection and drain
/// server messages until one carries at least one rectangle, then return the
/// applied framebuffer.
///
/// `incremental = false` forces a full re-send (not just changes), matching
/// what `screen find-text` and the recovery driver both want: a complete,
/// OCR-ready frame. Some servers emit no-op updates first, so the drain loop
/// skips zero-rectangle updates (and `Bell` / cut-text / colour-map events)
/// until real pixels arrive — identical to the loop it replaces.
pub async fn capture_frame<T>(conn: &mut RfbConnection<T>) -> Result<Framebuffer, RfbError>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    let (w, h) = conn.framebuffer_size();
    conn.request_framebuffer_update(false, 0, 0, w as u16, h as u16)
        .await?;
    loop {
        // Bound each read: a static screen may yield no further messages after
        // a no-op update, which would otherwise hang the drain indefinitely.
        match tokio::time::timeout(CAPTURE_MSG_TIMEOUT, conn.next_message()).await {
            Ok(Ok(ServerEvent::FramebufferUpdated { rectangles })) if rectangles > 0 => break,
            Ok(Ok(_)) => continue,
            Ok(Err(e)) => return Err(e),
            // No message within the window — return the frame we already hold.
            Err(_elapsed) => break,
        }
    }
    Ok(conn.framebuffer().into_owned())
}

/// [`capture_frame`] followed by a full-frame PNG encode — the OCR-ready
/// snapshot used by `screen find-text` and [`crate::recovery`]'s
/// `wait_for_text`.
pub async fn capture_frame_png<T>(conn: &mut RfbConnection<T>) -> Result<Vec<u8>, CaptureError>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    let fb = capture_frame(conn).await?;
    let (w, h) = (fb.width(), fb.height());
    Ok(encode_png(&fb, (0, 0, w, h))?)
}

/// Encode a framebuffer region to PNG. `region` is `(x, y, w, h)` in
/// framebuffer pixels; an out-of-bounds region is a `DimensionMismatch`
/// parameter error rather than a panic. Ports the helper formerly private to
/// `commands/screen.rs` (used there by `screen capture` for region crops and
/// `screen find-text` for full frames).
pub fn encode_png(
    fb: &Framebuffer,
    region: (u32, u32, u32, u32),
) -> Result<Vec<u8>, image::ImageError> {
    let (x, y, w, h) = region;
    if x + w > fb.width() || y + h > fb.height() {
        return Err(image::ImageError::Parameter(
            image::error::ParameterError::from_kind(
                image::error::ParameterErrorKind::DimensionMismatch,
            ),
        ));
    }
    let stride = fb.width() as usize * 4;
    let mut cropped = Vec::with_capacity((w as usize) * (h as usize) * 4);
    for row in 0..h as usize {
        let src_off = (y as usize + row) * stride + (x as usize) * 4;
        cropped.extend_from_slice(&fb.rgba()[src_off..src_off + (w as usize) * 4]);
    }
    let img = image::RgbaImage::from_raw(w, h, cropped).expect("buffer length matches w*h*4");
    let mut out = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 2×2 framebuffer with distinct pixels, for region-crop assertions.
    fn fb_2x2() -> Framebuffer {
        let mut fb = Framebuffer::new(2, 2).unwrap();
        // raw_rect takes BGRX (the server pixel layout); the exact colours do
        // not matter here — only that encode succeeds and bounds are honoured.
        fb.raw_rect(0, 0, 2, 2, &[0u8; 2 * 2 * 4]).unwrap();
        fb
    }

    #[test]
    fn encode_png_full_frame_succeeds() {
        let fb = fb_2x2();
        let png = encode_png(&fb, (0, 0, 2, 2)).unwrap();
        // PNG signature — proves we produced a real PNG, not raw bytes.
        assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
    }

    #[test]
    fn encode_png_subregion_succeeds() {
        let fb = fb_2x2();
        assert!(encode_png(&fb, (1, 1, 1, 1)).is_ok());
    }

    #[test]
    fn encode_png_rejects_out_of_bounds_region() {
        let fb = fb_2x2();
        // Width overflow and offset overflow both fault, never panic.
        assert!(encode_png(&fb, (0, 0, 3, 2)).is_err());
        assert!(encode_png(&fb, (2, 0, 1, 1)).is_err());
    }
}
