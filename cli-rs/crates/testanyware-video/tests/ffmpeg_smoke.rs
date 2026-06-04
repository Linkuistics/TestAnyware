//! Linux/Windows encoder smoke test: synthetic RGBA frames must produce a
//! readable MP4 through the `ffmpeg-next` arm — the non-macOS mirror of
//! `avfoundation_smoke.rs` (ADR-0006 / leaf 170). It exercises the full libav
//! graph (output → encoder → swscale → mux → trailer) without a live VM.
//!
//! It needs an ffmpeg runtime with libx264 present, so it runs on the build
//! host where that is true — Linux first, where leaf 190's harness verifies a
//! real recording. (On this macOS dev host the whole module is `cfg`'d out, so
//! these tests never compile here; they are link-checked via `cargo zigbuild
//! --tests` and run on Linux.)
#![cfg(not(target_os = "macos"))]

use testanyware_video::{new_encoder, VideoCodec, VideoEncoderConfig};

/// Fill a `w*h` RGBA buffer with a solid colour.
fn solid(w: u32, h: u32, color: [u8; 4]) -> Vec<u8> {
    let mut buf = vec![0u8; (w as usize) * (h as usize) * 4];
    for px in buf.chunks_exact_mut(4) {
        px.copy_from_slice(&color);
    }
    buf
}

#[test]
fn encodes_solid_color_frames_to_a_readable_mp4() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("rec.mp4");
    let (w, h, fps) = (64u32, 48u32, 30u32);

    let mut enc = new_encoder(VideoEncoderConfig {
        width: w,
        height: h,
        fps,
        codec: VideoCodec::H264,
        output: out.clone(),
    })
    .expect("encoder constructs");

    // Alternate red/blue so there is genuine inter-frame change to encode.
    for i in 0..15 {
        let color = if i % 2 == 0 { [200, 30, 30, 255] } else { [30, 30, 200, 255] };
        enc.append_frame(&solid(w, h, color)).expect("append frame");
    }
    enc.finish().expect("finish writing");

    let bytes = std::fs::read(&out).expect("read encoded mp4");
    assert!(
        bytes.len() > 1000,
        "an encoded 15-frame mp4 should be non-trivial, got {} bytes",
        bytes.len()
    );
    // MP4/QuickTime files open with a `ftyp` box: a 4-byte big-endian size
    // then the type tag `ftyp` at offset 4.
    assert_eq!(&bytes[4..8], b"ftyp", "file must begin with an ftyp box");
}

#[test]
fn rejects_a_frame_of_the_wrong_size() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("rec.mp4");
    let mut enc = new_encoder(VideoEncoderConfig {
        width: 64,
        height: 48,
        fps: 30,
        codec: VideoCodec::H264,
        output: out,
    })
    .expect("encoder constructs");

    // 10 bytes is not 64*48*4 — must be a typed FrameSize error, not a panic.
    let err = enc.append_frame(&[0u8; 10]).expect_err("wrong-size frame rejected");
    assert_eq!(err.code(), "USAGE_ERROR");
}
