//! The per-platform **video-encoder seam** (ADR-0006).
//!
//! `screen record` feeds a live RFB framebuffer stream into a [`VideoEncoder`]
//! one RGBA frame at a time, then finalises the file. The seam mirrors
//! `OcrEngine` in `testanyware-ocr-client`: the host picks the best *native*
//! encoder for the platform it runs on, behind one trait.
//!
//!   - **macOS** → in-process **AVFoundation / VideoToolbox** via pure-Rust
//!     `objc2` ([`crate::avfoundation`]); hardware-accelerated, no ffmpeg in
//!     the primary bundle. True parity with the Swift `AVAssetWriter`
//!     recorder.
//!   - **Linux / Windows** → **`ffmpeg-next`** (embedded libav). Tier-2 work
//!     that plugs into this same seam without reshaping it; until it lands,
//!     [`new_encoder`] returns [`VideoEncoderError::Unsupported`].
//!
//! Frames are **RGBA, top-left origin** — the byte layout `Framebuffer::rgba`
//! produces and `screen capture` already writes as PNG, so the recorder feeds
//! the encoder exactly what the rest of the CLI speaks.

use std::path::PathBuf;

/// How to encode a recording: target geometry, frame rate, codec, and output
/// path. The geometry is the *recorded* frame size — the crop region when one
/// is given, else the full guest framebuffer.
#[derive(Debug, Clone)]
pub struct VideoEncoderConfig {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub codec: VideoCodec,
    pub output: PathBuf,
}

/// Video codec, matching the Swift recorder's two choices. H.264 is the
/// portable default; HEVC is smaller but less universally playable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoCodec {
    H264,
    Hevc,
}

/// A failure somewhere in the encode lifecycle. [`VideoEncoderError::code`]
/// maps each to a contract error code so the `screen record` `--json`
/// envelope reports a stable code (§4).
#[derive(Debug, thiserror::Error)]
pub enum VideoEncoderError {
    /// No native encoder is wired for this platform yet (the Tier-2
    /// `ffmpeg-next` path). Maps to the contract `ACTION_UNSUPPORTED`.
    #[error("video recording is not supported on this platform yet: {0}")]
    Unsupported(String),
    /// The encoder could not be constructed / started (writer init, input
    /// add, session start).
    #[error("video encoder setup failed: {0}")]
    Setup(String),
    /// Appending a frame failed (pixel-buffer allocation or append).
    #[error("appending a video frame failed: {0}")]
    Append(String),
    /// Finalising the file failed (writer did not finish cleanly).
    #[error("finalising the recording failed: {0}")]
    Finish(String),
    /// A frame's byte length did not match `width * height * 4`.
    #[error(
        "frame size mismatch: expected {expected} bytes for {width}x{height} RGBA, got {got}"
    )]
    FrameSize {
        expected: usize,
        got: usize,
        width: u32,
        height: u32,
    },
}

impl VideoEncoderError {
    /// The contract error code (§4) this maps to.
    pub fn code(&self) -> &'static str {
        match self {
            VideoEncoderError::Unsupported(_) => "ACTION_UNSUPPORTED",
            VideoEncoderError::FrameSize { .. } => "USAGE_ERROR",
            VideoEncoderError::Setup(_)
            | VideoEncoderError::Append(_)
            | VideoEncoderError::Finish(_) => "INTERNAL",
        }
    }
}

/// A live encoder writing one growing video file. Stateful: `append_frame`
/// is called once per captured frame in order, then `finish` flushes and
/// closes the file (consuming the encoder so it cannot be reused).
pub trait VideoEncoder {
    /// Append one RGBA frame (`width * height * 4` bytes, top-left origin) at
    /// the next presentation timestamp (`frame_index / fps`).
    fn append_frame(&mut self, rgba: &[u8]) -> Result<(), VideoEncoderError>;

    /// Finalise and close the file. Consumes the encoder.
    fn finish(self: Box<Self>) -> Result<(), VideoEncoderError>;
}

/// Bytes in one RGBA frame of the given geometry.
pub(crate) fn frame_len(width: u32, height: u32) -> usize {
    width as usize * height as usize * 4
}

/// Construct the native encoder for the current platform, or
/// [`VideoEncoderError::Unsupported`] where none is wired yet.
pub fn new_encoder(
    config: VideoEncoderConfig,
) -> Result<Box<dyn VideoEncoder>, VideoEncoderError> {
    #[cfg(target_os = "macos")]
    {
        Ok(Box::new(crate::avfoundation::AvAssetWriterEncoder::new(config)?))
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = config;
        Err(VideoEncoderError::Unsupported(
            "the ffmpeg-next encoder for Linux/Windows is Tier-2 work and is \
             not yet built (ADR-0006)"
                .to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_len_is_rgba_4_bytes_per_pixel() {
        assert_eq!(frame_len(2, 3), 24);
        assert_eq!(frame_len(0, 10), 0);
    }

    #[test]
    fn error_codes_map_to_contract_sections() {
        assert_eq!(VideoEncoderError::Unsupported("x".into()).code(), "ACTION_UNSUPPORTED");
        assert_eq!(VideoEncoderError::Setup("x".into()).code(), "INTERNAL");
        assert_eq!(
            VideoEncoderError::FrameSize { expected: 4, got: 3, width: 1, height: 1 }.code(),
            "USAGE_ERROR"
        );
    }
}
