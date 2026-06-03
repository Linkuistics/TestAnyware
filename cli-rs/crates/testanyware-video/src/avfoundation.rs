//! macOS native video encoder: `AVAssetWriter` + an
//! `AVAssetWriterInputPixelBufferAdaptor`, driven through pure-Rust `objc2`
//! bindings (ADR-0006; the FFI strategy ADR-0003 set for Apple Vision — no
//! Swift toolchain at build time).
//!
//! A faithful port of the Swift recorder
//! (`cli/Sources/TestAnywareDriver/Capture/StreamingCapture.swift`): build a
//! writer for an `.mp4`, add one H.264/HEVC video input, wrap it in a
//! pixel-buffer adaptor over a 32-BGRA pool, then per frame pull a buffer
//! from the pool, blit the RGBA frame into it (swapping to BGRA), and append
//! it at presentation time `frame_index / fps`. `finish` marks the input done
//! and finalises the file.
//!
//! Everything here is `!Send` Objective-C/CoreFoundation state; the encoder
//! lives on one thread for the whole recording (the `screen record` command's
//! async task, polled on the `block_on` thread — no work-stealing, so the
//! objects never migrate).

use std::ptr::{self, NonNull};

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{AnyThread, Message};
use objc2_av_foundation::{
    AVAssetWriter, AVAssetWriterInput, AVAssetWriterInputPixelBufferAdaptor, AVFileTypeMPEG4,
    AVMediaTypeVideo, AVVideoCodecKey, AVVideoCodecTypeH264, AVVideoCodecTypeHEVC,
    AVVideoHeightKey, AVVideoWidthKey,
};
use objc2_core_foundation::{CFRetained, CFString};
use objc2_core_media::CMTime;
use objc2_core_video::{
    kCVPixelBufferHeightKey, kCVPixelBufferPixelFormatTypeKey, kCVPixelBufferWidthKey,
    kCVPixelFormatType_32BGRA, kCVReturnSuccess, CVPixelBuffer, CVPixelBufferGetBaseAddress,
    CVPixelBufferGetBytesPerRow, CVPixelBufferLockBaseAddress, CVPixelBufferLockFlags,
    CVPixelBufferPool, CVPixelBufferUnlockBaseAddress,
};
use objc2_foundation::{NSDictionary, NSNumber, NSString, NSURL};

use crate::encoder::{frame_len, VideoCodec, VideoEncoder, VideoEncoderConfig, VideoEncoderError};

/// Native AVFoundation encoder. Holds the writer graph and the running
/// presentation-frame counter.
pub(crate) struct AvAssetWriterEncoder {
    writer: Retained<AVAssetWriter>,
    input: Retained<AVAssetWriterInput>,
    adaptor: Retained<AVAssetWriterInputPixelBufferAdaptor>,
    /// Index of the next frame; its presentation time is `frame_count / fps`.
    frame_count: i64,
    config: VideoEncoderConfig,
}

impl AvAssetWriterEncoder {
    pub(crate) fn new(config: VideoEncoderConfig) -> Result<Self, VideoEncoderError> {
        // Overwrite any prior file (mirrors the Swift `removeItem` up front);
        // AVAssetWriter refuses to write to an existing URL.
        let _ = std::fs::remove_file(&config.output);

        // ---- video output settings: codec + geometry --------------------
        let codec_value = match config.codec {
            VideoCodec::H264 => unsafe { AVVideoCodecTypeH264 },
            VideoCodec::Hevc => unsafe { AVVideoCodecTypeHEVC },
        }
        .ok_or_else(|| VideoEncoderError::Setup("AVVideoCodecType constant missing".into()))?;
        let width_num = NSNumber::numberWithInt(config.width as i32);
        let height_num = NSNumber::numberWithInt(config.height as i32);
        let codec_key = unsafe { AVVideoCodecKey }
            .ok_or_else(|| VideoEncoderError::Setup("AVVideoCodecKey missing".into()))?;
        let width_key = unsafe { AVVideoWidthKey }
            .ok_or_else(|| VideoEncoderError::Setup("AVVideoWidthKey missing".into()))?;
        let height_key = unsafe { AVVideoHeightKey }
            .ok_or_else(|| VideoEncoderError::Setup("AVVideoHeightKey missing".into()))?;
        let settings: Retained<NSDictionary<NSString, AnyObject>> = NSDictionary::from_slices(
            &[codec_key, width_key, height_key],
            &[any(codec_value), any(&*width_num), any(&*height_num)],
        );

        // ---- the video input --------------------------------------------
        let media_type = unsafe { AVMediaTypeVideo }
            .ok_or_else(|| VideoEncoderError::Setup("AVMediaTypeVideo missing".into()))?;
        let input = unsafe {
            AVAssetWriterInput::assetWriterInputWithMediaType_outputSettings(
                media_type,
                Some(&settings),
            )
        };
        // The frames arrive at roughly real time off the RFB stream.
        unsafe { input.setExpectsMediaDataInRealTime(true) };

        // ---- pixel-buffer adaptor over a 32-BGRA pool -------------------
        let fmt_num = NSNumber::numberWithUnsignedInt(kCVPixelFormatType_32BGRA);
        let attr_w = NSNumber::numberWithInt(config.width as i32);
        let attr_h = NSNumber::numberWithInt(config.height as i32);
        let attrs: Retained<NSDictionary<NSString, AnyObject>> = NSDictionary::from_slices(
            &[
                cf_string_as_ns(unsafe { kCVPixelBufferPixelFormatTypeKey }),
                cf_string_as_ns(unsafe { kCVPixelBufferWidthKey }),
                cf_string_as_ns(unsafe { kCVPixelBufferHeightKey }),
            ],
            &[any(&*fmt_num), any(&*attr_w), any(&*attr_h)],
        );
        let adaptor = unsafe {
            AVAssetWriterInputPixelBufferAdaptor::assetWriterInputPixelBufferAdaptorWithAssetWriterInput_sourcePixelBufferAttributes(
                &input,
                Some(&attrs),
            )
        };

        // ---- the writer + session ---------------------------------------
        let path_ns = NSString::from_str(&config.output.to_string_lossy());
        let url = NSURL::fileURLWithPath(&path_ns);
        let file_type = unsafe { AVFileTypeMPEG4 }
            .ok_or_else(|| VideoEncoderError::Setup("AVFileTypeMPEG4 missing".into()))?;
        let writer = unsafe {
            AVAssetWriter::initWithURL_fileType_error(AVAssetWriter::alloc(), &url, file_type)
        }
        .map_err(|e| VideoEncoderError::Setup(format!("AVAssetWriter init failed: {e:?}")))?;

        unsafe { writer.addInput(&input) };
        if !unsafe { writer.startWriting() } {
            return Err(VideoEncoderError::Setup(format!(
                "startWriting failed: {:?}",
                unsafe { writer.error() }
            )));
        }
        // Start the session at t=0; presentation times are measured from here.
        unsafe { writer.startSessionAtSourceTime(CMTime::new(0, config.fps.max(1) as i32)) };

        Ok(Self {
            writer,
            input,
            adaptor,
            frame_count: 0,
            config,
        })
    }
}

impl VideoEncoder for AvAssetWriterEncoder {
    fn append_frame(&mut self, rgba: &[u8]) -> Result<(), VideoEncoderError> {
        let expected = frame_len(self.config.width, self.config.height);
        if rgba.len() != expected {
            return Err(VideoEncoderError::FrameSize {
                expected,
                got: rgba.len(),
                width: self.config.width,
                height: self.config.height,
            });
        }

        // Backpressure: if the input cannot take more data right now, drop
        // this frame (as the Swift recorder did). The next accepted frame
        // takes the next index, so presentation times stay monotonic.
        if !unsafe { self.input.isReadyForMoreMediaData() } {
            return Ok(());
        }

        let pool = unsafe { self.adaptor.pixelBufferPool() }
            .ok_or_else(|| VideoEncoderError::Append("pixel buffer pool unavailable".into()))?;

        let mut raw: *mut CVPixelBuffer = ptr::null_mut();
        let rc =
            unsafe { CVPixelBufferPool::create_pixel_buffer(None, &pool, NonNull::from(&mut raw)) };
        if rc != kCVReturnSuccess {
            return Err(VideoEncoderError::Append(format!(
                "CVPixelBufferPoolCreatePixelBuffer failed (CVReturn {rc})"
            )));
        }
        let raw = NonNull::new(raw)
            .ok_or_else(|| VideoEncoderError::Append("pool returned a null buffer".into()))?;
        // Take ownership (+1 from the Create rule); released on drop.
        let buffer = unsafe { CFRetained::from_raw(raw) };

        // Blit RGBA → the buffer's BGRA, honouring the buffer's row stride
        // (which may exceed width*4 for alignment).
        unsafe {
            CVPixelBufferLockBaseAddress(&buffer, CVPixelBufferLockFlags(0));
            let base = CVPixelBufferGetBaseAddress(&buffer) as *mut u8;
            let bytes_per_row = CVPixelBufferGetBytesPerRow(&buffer);
            if !base.is_null() {
                let w = self.config.width as usize;
                let h = self.config.height as usize;
                for y in 0..h {
                    let dst = std::slice::from_raw_parts_mut(base.add(y * bytes_per_row), w * 4);
                    let src = &rgba[y * w * 4..(y + 1) * w * 4];
                    rgba_row_to_bgra(src, dst);
                }
            }
            CVPixelBufferUnlockBaseAddress(&buffer, CVPixelBufferLockFlags(0));
        }

        let pts = unsafe { CMTime::new(self.frame_count, self.config.fps.max(1) as i32) };
        if !unsafe {
            self.adaptor
                .appendPixelBuffer_withPresentationTime(&buffer, pts)
        } {
            return Err(VideoEncoderError::Append(format!(
                "appendPixelBuffer failed: {:?}",
                unsafe { self.writer.error() }
            )));
        }
        self.frame_count += 1;
        Ok(())
    }

    fn finish(self: Box<Self>) -> Result<(), VideoEncoderError> {
        unsafe { self.input.markAsFinished() };
        // The synchronous `finishWriting` is deprecated in favour of the
        // completion-handler form, but we run off the main thread (the CLI's
        // tokio worker), where the blocking call is safe and avoids a block2
        // callback round-trip — so deliberately keep the blocking form.
        #[allow(deprecated)]
        let finished = unsafe { self.writer.finishWriting() };
        if !finished {
            return Err(VideoEncoderError::Finish(format!(
                "finishWriting failed: {:?}",
                unsafe { self.writer.error() }
            )));
        }
        Ok(())
    }
}

/// Copy one RGBA row into a 32-BGRA destination row (in-memory byte order
/// `B, G, R, A`). The destination may be longer than the source (row
/// padding); only `src.len() / 4` pixels are written.
fn rgba_row_to_bgra(src: &[u8], dst: &mut [u8]) {
    for (s, d) in src.chunks_exact(4).zip(dst.chunks_exact_mut(4)) {
        d[0] = s[2]; // B
        d[1] = s[1]; // G
        d[2] = s[0]; // R
        d[3] = s[3]; // A
    }
}

/// Reinterpret any Objective-C object reference as `&AnyObject` for use as a
/// heterogeneous `NSDictionary` value.
///
/// SAFETY: every Objective-C object pointer is a valid `id` / `AnyObject`.
fn any<T: Message>(o: &T) -> &AnyObject {
    unsafe { &*(ptr::from_ref(o).cast::<AnyObject>()) }
}

/// Toll-free-bridge a CoreFoundation `CFString` to `&NSString`.
///
/// SAFETY: `CFString` and `NSString` are the canonical toll-free-bridged
/// pair — identical layout and object model — so the reference is valid.
fn cf_string_as_ns(s: &CFString) -> &NSString {
    unsafe { &*(ptr::from_ref(s).cast::<NSString>()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgba_row_to_bgra_swaps_red_and_blue_keeps_alpha() {
        let src = [10, 20, 30, 40, 50, 60, 70, 80]; // two RGBA pixels
        let mut dst = [0u8; 8];
        rgba_row_to_bgra(&src, &mut dst);
        // pixel 0: R=10,G=20,B=30,A=40 → B,G,R,A = 30,20,10,40
        assert_eq!(&dst[0..4], &[30, 20, 10, 40]);
        // pixel 1: R=50,G=60,B=70,A=80 → 70,60,50,80
        assert_eq!(&dst[4..8], &[70, 60, 50, 80]);
    }

    #[test]
    fn rgba_row_to_bgra_ignores_destination_row_padding() {
        let src = [1, 2, 3, 4]; // one pixel
        let mut dst = [0u8; 12]; // room for three pixels (padding)
        rgba_row_to_bgra(&src, &mut dst);
        assert_eq!(&dst[0..4], &[3, 2, 1, 4]);
        assert_eq!(&dst[4..12], &[0; 8], "padding bytes are untouched");
    }
}
