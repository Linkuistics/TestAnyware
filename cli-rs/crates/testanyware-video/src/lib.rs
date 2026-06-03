//! Per-platform video encoder for `testanyware screen record` (ADR-0006).
//!
//! See [`encoder`] for the seam. The macOS implementation
//! ([`avfoundation`]) is a pure-Rust `objc2` port of the Swift
//! `StreamingCapture` (`AVAssetWriter` + pixel-buffer adaptor), the same FFI
//! strategy ADR-0003 chose for Apple Vision.

pub mod encoder;

#[cfg(target_os = "macos")]
mod avfoundation;

pub use encoder::{new_encoder, VideoCodec, VideoEncoder, VideoEncoderConfig, VideoEncoderError};
