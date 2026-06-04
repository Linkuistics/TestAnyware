//! Linux / Windows video encoder: embedded **libav** via `ffmpeg-next`
//! (ADR-0006), behind the same [`VideoEncoder`] seam the native macOS
//! AVFoundation encoder ([`crate::avfoundation`]) sits behind.
//!
//! The lifecycle mirrors the AVFoundation port one-for-one so the two arms
//! behave identically from the recorder's point of view:
//!
//!   - **setup** — open an `.mp4` output, find the libx264/libx265 encoder for
//!     the requested codec, add one video stream, configure the encoder context
//!     (geometry, `1/fps` time base, YUV420P), open it, and write the header;
//!   - **append** — convert the incoming RGBA frame to YUV420P with `swscale`,
//!     stamp it at presentation time `frame_index / fps`, send it to the
//!     encoder, and mux every packet the encoder hands back;
//!   - **finish** — flush the encoder (send EOF), drain the tail packets, and
//!     write the trailer.
//!
//! Frames are **RGBA, top-left origin** — the same byte layout the macOS arm
//! blits into a BGRA `CVPixelBuffer`; here `swscale` does the conversion to the
//! encoder's planar YUV420P. Errors map to the seam's three phases
//! (`Setup`/`Append`/`Finish`), exactly like the AVFoundation arm.
//!
//! Everything here is single-threaded `!Send` libav state; the encoder lives
//! on one thread for the whole recording (the `screen record` command's async
//! task), so the contexts never migrate.

use std::sync::Once;

use ffmpeg_next as ffmpeg;
use ffmpeg::software::scaling::{context::Context as Scaler, flag::Flags as ScaleFlags};
use ffmpeg::util::format::Pixel;
use ffmpeg::{codec, encoder, format, frame, Dictionary, Packet, Rational};

use crate::encoder::{frame_len, VideoCodec, VideoEncoder, VideoEncoderConfig, VideoEncoderError};

/// `ffmpeg::init()` registers libav globally; run it exactly once per process.
static FFMPEG_INIT: Once = Once::new();

/// Embedded-libav encoder. Owns the muxer, the opened video encoder, the
/// RGBA→YUV420P scaler, and the running presentation-frame counter.
pub(crate) struct FfmpegEncoder {
    octx: format::context::Output,
    encoder: encoder::Video,
    scaler: Scaler,
    /// Reused RGBA source frame; refilled from the caller's bytes each append.
    src: frame::Video,
    stream_index: usize,
    /// The encoder's own `1/fps` time base, the source for packet rescaling.
    encoder_time_base: Rational,
    /// The muxer's chosen stream time base (set by `write_header`).
    stream_time_base: Rational,
    /// Index of the next frame; its PTS is `frame_count` in `1/fps` units.
    frame_count: i64,
    config: VideoEncoderConfig,
}

impl FfmpegEncoder {
    pub(crate) fn new(config: VideoEncoderConfig) -> Result<Self, VideoEncoderError> {
        FFMPEG_INIT.call_once(|| {
            // A failed init surfaces later as a "not found"/setup error; the
            // Once only guarantees we attempt it a single time.
            let _ = ffmpeg::init();
        });

        // YUV420P is 4:2:0 chroma-subsampled, so both dimensions must be even.
        // Guard up front with an actionable message (the leaf's odd-dimension
        // note) rather than letting libx264 fail opaquely on `open`.
        if config.width % 2 != 0 || config.height % 2 != 0 {
            return Err(VideoEncoderError::Setup(format!(
                "recording geometry {}x{} must have even width and height for \
                 H.264/HEVC (YUV420P); adjust --region to even dimensions",
                config.width, config.height
            )));
        }

        // Overwrite any prior file (mirrors the AVFoundation arm's remove-first).
        let _ = std::fs::remove_file(&config.output);

        let mut octx = format::output(&config.output)
            .map_err(|e| VideoEncoderError::Setup(format!("open output {:?}: {e}", config.output)))?;

        let codec = encoder::find(codec_id(config.codec)).ok_or_else(|| {
            VideoEncoderError::Setup(format!(
                "no libav encoder for {:?} (is this ffmpeg built with libx264/libx265?)",
                config.codec
            ))
        })?;

        // mp4 needs the codec's extradata in the container header, not in-band.
        let global_header = octx
            .format()
            .flags()
            .contains(format::Flags::GLOBAL_HEADER);

        let mut ost = octx
            .add_stream(codec)
            .map_err(|e| VideoEncoderError::Setup(format!("add video stream: {e}")))?;
        let stream_index = ost.index();
        let encoder_time_base = Rational(1, config.fps.max(1) as i32);
        ost.set_time_base(encoder_time_base);
        // Drop the stream borrow before re-borrowing the context below.
        drop(ost);

        let mut enc = codec::context::Context::new_with_codec(codec)
            .encoder()
            .video()
            .map_err(|e| VideoEncoderError::Setup(format!("create video encoder: {e}")))?;
        enc.set_width(config.width);
        enc.set_height(config.height);
        enc.set_format(Pixel::YUV420P);
        enc.set_time_base(encoder_time_base);
        enc.set_frame_rate(Some(Rational(config.fps.max(1) as i32, 1)));
        if global_header {
            enc.set_flags(codec::Flags::GLOBAL_HEADER);
        }

        let encoder = enc
            .open_with(Dictionary::new())
            .map_err(|e| VideoEncoderError::Setup(format!("open encoder: {e}")))?;

        // Copy the opened encoder's parameters (extradata, profile, …) onto the
        // muxer stream so the header is well-formed.
        octx.stream_mut(stream_index)
            .expect("stream just added")
            .set_parameters(&encoder);

        octx.write_header()
            .map_err(|e| VideoEncoderError::Setup(format!("write header: {e}")))?;
        let stream_time_base = octx
            .stream(stream_index)
            .expect("stream just added")
            .time_base();

        let scaler = Scaler::get(
            Pixel::RGBA,
            config.width,
            config.height,
            Pixel::YUV420P,
            config.width,
            config.height,
            ScaleFlags::BILINEAR,
        )
        .map_err(|e| VideoEncoderError::Setup(format!("create RGBA→YUV420P scaler: {e}")))?;

        let src = frame::Video::new(Pixel::RGBA, config.width, config.height);

        Ok(Self {
            octx,
            encoder,
            scaler,
            src,
            stream_index,
            encoder_time_base,
            stream_time_base,
            frame_count: 0,
            config,
        })
    }

    /// Mux every packet the encoder currently has ready. Stops on the
    /// encoder's "need more input" / EOF signal (a non-`Ok` receive).
    fn drain_packets(&mut self) -> Result<(), ffmpeg::Error> {
        let mut packet = Packet::empty();
        while self.encoder.receive_packet(&mut packet).is_ok() {
            packet.set_stream(self.stream_index);
            packet.rescale_ts(self.encoder_time_base, self.stream_time_base);
            packet.write_interleaved(&mut self.octx)?;
        }
        Ok(())
    }
}

impl VideoEncoder for FfmpegEncoder {
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

        // Copy the tightly-packed RGBA bytes into the source frame, honouring
        // the frame's row stride (libav may pad rows for alignment).
        let w = self.config.width as usize;
        let h = self.config.height as usize;
        let stride = self.src.stride(0);
        let data = self.src.data_mut(0);
        for y in 0..h {
            data[y * stride..y * stride + w * 4].copy_from_slice(&rgba[y * w * 4..(y + 1) * w * 4]);
        }

        // Convert to the encoder's YUV420P. `run` allocates the empty output.
        let mut yuv = frame::Video::empty();
        self.scaler
            .run(&self.src, &mut yuv)
            .map_err(|e| VideoEncoderError::Append(format!("scale RGBA→YUV420P: {e}")))?;
        yuv.set_pts(Some(self.frame_count));

        self.encoder
            .send_frame(&yuv)
            .map_err(|e| VideoEncoderError::Append(format!("send frame to encoder: {e}")))?;
        self.drain_packets()
            .map_err(|e| VideoEncoderError::Append(format!("mux packet: {e}")))?;

        self.frame_count += 1;
        Ok(())
    }

    fn finish(mut self: Box<Self>) -> Result<(), VideoEncoderError> {
        // Flush: signal end-of-stream, then drain the encoder's tail packets.
        self.encoder
            .send_eof()
            .map_err(|e| VideoEncoderError::Finish(format!("flush encoder: {e}")))?;
        self.drain_packets()
            .map_err(|e| VideoEncoderError::Finish(format!("mux tail packet: {e}")))?;
        self.octx
            .write_trailer()
            .map_err(|e| VideoEncoderError::Finish(format!("write trailer: {e}")))?;
        Ok(())
    }
}

/// Map the seam's codec choice to the libav encoder id. H.264 → libx264,
/// HEVC → libx265 (the GPL ffmpeg builds bundle both).
fn codec_id(codec: VideoCodec) -> codec::Id {
    match codec {
        VideoCodec::H264 => codec::Id::H264,
        VideoCodec::Hevc => codec::Id::HEVC,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codec_id_maps_seam_codecs_to_libav_ids() {
        assert_eq!(codec_id(VideoCodec::H264), codec::Id::H264);
        assert_eq!(codec_id(VideoCodec::Hevc), codec::Id::HEVC);
    }

    #[test]
    fn odd_dimensions_are_rejected_at_setup() {
        // Odd width is rejected before any libav object is built, so this is a
        // pure guard test that needs no ffmpeg runtime. (`FfmpegEncoder` holds
        // non-`Debug` libav state, so match rather than `unwrap_err`.)
        let result = FfmpegEncoder::new(VideoEncoderConfig {
            width: 101,
            height: 100,
            fps: 30,
            codec: VideoCodec::H264,
            output: std::path::PathBuf::from("/tmp/taw-odd-dim-guard.mp4"),
        });
        let err = match result {
            Ok(_) => panic!("odd width should be rejected at setup"),
            Err(e) => e,
        };
        assert_eq!(err.code(), "INTERNAL"); // Setup → INTERNAL
        assert!(err.to_string().contains("even width and height"));
    }
}
