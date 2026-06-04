//! Per-platform video encoder for `testanyware screen record` (ADR-0006).
//!
//! See [`encoder`] for the seam. The macOS implementation
//! ([`avfoundation`]) is a pure-Rust `objc2` port of the Swift
//! `StreamingCapture` (`AVAssetWriter` + pixel-buffer adaptor), the same FFI
//! strategy ADR-0003 chose for Apple Vision. The Linux/Windows implementation
//! ([`ffmpeg`]) is the embedded-libav sibling behind the same seam.

pub mod encoder;

#[cfg(target_os = "macos")]
mod avfoundation;

#[cfg(not(target_os = "macos"))]
mod ffmpeg;

pub use encoder::{new_encoder, VideoCodec, VideoEncoder, VideoEncoderConfig, VideoEncoderError};
