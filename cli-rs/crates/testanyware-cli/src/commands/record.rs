//! `testanyware screen record` (and the `record` alias) — capture the live
//! guest framebuffer over RFB to an `.mp4` (ADR-0006).
//!
//! Unlike every other screen command (one-shot RFB connections, ADR-0004),
//! the recorder is a **bounded long-lived RFB consumer** — the second one
//! after the embedded viewer (ADR-0005). It opens its own connection, keeps
//! the `FramebufferUpdate` stream flowing, and samples the current frame at a
//! fixed `--fps` into a per-platform [`VideoEncoder`] until `--duration`
//! elapses. It is *non-interactive* (no input forwarding), so it is simpler
//! than the viewer: connect → stream → sample → finish.
//!
//! Encoding goes through the [`testanyware_video`] seam: native
//! AVFoundation/VideoToolbox on macOS, the Tier-2 `ffmpeg-next` encoder
//! elsewhere (until that lands, non-macOS reports `ACTION_UNSUPPORTED`).

use std::time::{Duration, Instant};

use serde_json::json;
use testanyware_rfb::{RfbConnection, ServerEvent};
use testanyware_video::{new_encoder, VideoCodec, VideoEncoderConfig};

use crate::commands::exit_resolve_error;
use crate::commands::screen::{exit_rfb_error, parse_region};
use crate::output::{exit_code_for, print_error, print_success, OutputMode};
use crate::resolve::{resolve_vnc, ConnectionOptions};

/// The hard cap on a recording's length, matching the Swift recorder's
/// server-side 300s limit. `--duration 0` means "record up to this cap".
const MAX_DURATION_SECS: u32 = 300;
/// Default frame rate when `--fps` is omitted (Swift default). The default
/// output path (`recording.mp4`, also the Swift default) is set by clap.
const DEFAULT_FPS: u32 = 30;

/// How often to ask the server for an incremental update so the framebuffer
/// stays fresh between samples (~30/s; the server answers only on change).
const POLL_INTERVAL: Duration = Duration::from_millis(33);

/// Resolve the effective recording length: `0` → the cap, otherwise the
/// request clamped to the cap (the `--duration` help promises "max 300s").
pub(crate) fn effective_duration(requested: u32) -> u32 {
    if requested == 0 {
        MAX_DURATION_SECS
    } else {
        requested.min(MAX_DURATION_SECS)
    }
}

/// `testanyware screen record` handler. `output`/`fps`/`duration` carry the
/// raw CLI values (already defaulted by clap where applicable); `region` is
/// the unparsed `X,Y,W,H` string.
pub async fn run_screen_record(
    opts: ConnectionOptions,
    output: String,
    fps: Option<u32>,
    duration: Option<u32>,
    region: Option<String>,
    mode: OutputMode,
    dry_run: bool,
) {
    let fps = fps.unwrap_or(DEFAULT_FPS);
    if fps == 0 {
        print_error(
            mode,
            "USAGE_ERROR",
            "--fps must be at least 1",
            Some("Pass a positive --fps, e.g. --fps 30."),
            json!({ "fps": 0 }),
            2,
        );
    }
    let secs = effective_duration(duration.unwrap_or(0));

    // Validate --region up front so a bad value fails fast (and identically
    // in --dry-run), before any connection is opened.
    let region = match region.as_deref().map(parse_region) {
        Some(Ok(r)) => Some(r),
        Some(Err(msg)) => print_error(
            mode,
            "USAGE_ERROR",
            &format!("invalid --region: {msg}"),
            Some("Expected --region X,Y,W,H with non-negative integers."),
            json!({ "value": region.unwrap_or_default() }),
            2,
        ),
        None => None,
    };

    // --dry-run (§9.3): validate + report the plan without connecting or
    // writing. Geometry is omitted unless a region pins it, since the guest
    // framebuffer size is only known after connecting.
    if dry_run {
        emit(
            mode,
            &output,
            fps,
            secs,
            region,
            region.map(|(_, _, w, h)| (w, h)),
            0,
            true,
        );
        return;
    }

    let endpoint = match resolve_vnc(&opts) {
        Ok(e) => e,
        Err(err) => exit_resolve_error(err, mode),
    };
    let mut conn = match RfbConnection::connect(
        &endpoint.host,
        endpoint.port,
        endpoint.password.as_deref().map(str::as_bytes),
    )
    .await
    {
        Ok(c) => c,
        Err(err) => exit_rfb_error(err, mode),
    };

    let (fb_w, fb_h) = conn.framebuffer_size();
    let crop = match region {
        Some((x, y, w, h)) if x + w > fb_w || y + h > fb_h => print_error(
            mode,
            "USAGE_ERROR",
            &format!("--region {x},{y},{w},{h} is outside the {fb_w}x{fb_h} framebuffer"),
            Some("Shrink the region or omit it to record the full screen."),
            json!({ "region": [x, y, w, h], "framebuffer": [fb_w, fb_h] }),
            2,
        ),
        Some(r) => r,
        None => (0, 0, fb_w, fb_h),
    };
    let (rec_w, rec_h) = (crop.2, crop.3);

    let mut encoder = match new_encoder(VideoEncoderConfig {
        width: rec_w,
        height: rec_h,
        fps,
        codec: VideoCodec::H264,
        output: output.clone().into(),
    }) {
        Ok(e) => e,
        Err(err) => {
            let code = err.code();
            print_error(mode, code, &err.to_string(), None, json!({}), exit_code_for(code));
        }
    };

    // Prime the stream with one full update and drain to the first real
    // frame, so frame 0 is genuine rather than a blank pre-decode buffer.
    if let Err(err) = conn
        .request_framebuffer_update(false, 0, 0, fb_w as u16, fb_h as u16)
        .await
    {
        exit_rfb_error(err, mode);
    }
    loop {
        match conn.next_message().await {
            Ok(ServerEvent::FramebufferUpdated { rectangles }) if rectangles > 0 => break,
            Ok(_) => continue,
            Err(err) => exit_rfb_error(err, mode),
        }
    }

    let frames = match capture_loop(&mut conn, encoder.as_mut(), crop, fps, secs).await {
        Ok(n) => n,
        Err(err) => exit_rfb_error(err, mode),
    };

    if let Err(err) = encoder.finish() {
        let code = err.code();
        print_error(mode, code, &err.to_string(), None, json!({}), exit_code_for(code));
    }

    emit(mode, &output, fps, secs, region, Some((rec_w, rec_h)), frames, false);
}

/// The bounded stream→sample loop. Keeps the framebuffer fresh by applying
/// incoming updates and re-requesting incrementals on [`POLL_INTERVAL`],
/// while a separate `fps` ticker snapshots the current (cropped) frame into
/// the encoder. Returns the number of frames appended. A frame the encoder
/// drops under backpressure still counts as a tick but not as an appended
/// frame, so the count reflects what actually landed in the file.
async fn capture_loop<T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin>(
    conn: &mut RfbConnection<T>,
    encoder: &mut dyn testanyware_video::VideoEncoder,
    crop: (u32, u32, u32, u32),
    fps: u32,
    secs: u32,
) -> Result<u64, testanyware_rfb::RfbError> {
    let (fb_w, fb_h) = conn.framebuffer_size();
    let frame_interval = Duration::from_secs_f64(1.0 / fps as f64);
    let mut sampler = tokio::time::interval(frame_interval);
    sampler.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut poll = tokio::time::interval(POLL_INTERVAL);
    poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    let deadline = Instant::now() + Duration::from_secs(u64::from(secs));
    let mut frames: u64 = 0;

    loop {
        tokio::select! {
            // Apply server updates in place; `framebuffer()` then reflects them.
            // The `ServerEvent` itself is unused — only its side effect matters.
            msg = conn.next_message() => { msg?; }
            _ = poll.tick() => {
                conn.request_framebuffer_update(true, 0, 0, fb_w as u16, fb_h as u16).await?;
            }
            _ = sampler.tick() => {
                if Instant::now() >= deadline {
                    break;
                }
                let fb = conn.framebuffer();
                let frame = crop_rgba(fb.rgba(), fb.width(), crop);
                // An encoder error here is INTERNAL, not an RFB fault; surface
                // it as such rather than masquerading as a connection drop.
                if encoder.append_frame(&frame).is_ok() {
                    frames += 1;
                }
            }
        }
    }
    Ok(frames)
}

/// Copy the `crop` rectangle out of a full-frame RGBA buffer (row stride
/// `src_width * 4`) into a tight `w*h*4` buffer. A full-frame crop is a plain
/// copy. Callers guarantee the rectangle is within bounds.
fn crop_rgba(src: &[u8], src_width: u32, crop: (u32, u32, u32, u32)) -> Vec<u8> {
    let (x, y, w, h) = crop;
    let stride = src_width as usize * 4;
    let mut out = Vec::with_capacity(w as usize * h as usize * 4);
    for row in 0..h as usize {
        let off = (y as usize + row) * stride + x as usize * 4;
        out.extend_from_slice(&src[off..off + w as usize * 4]);
    }
    out
}

/// Emit the `screen record` result (success or dry-run plan). Text mode is a
/// human summary; `--json` is the §3.1 envelope.
#[allow(clippy::too_many_arguments)]
fn emit(
    mode: OutputMode,
    output: &str,
    fps: u32,
    duration: u32,
    region: Option<(u32, u32, u32, u32)>,
    geometry: Option<(u32, u32)>,
    frames: u64,
    dry_run: bool,
) {
    match mode {
        OutputMode::Text => {
            if dry_run {
                println!("[dry-run] would record {output} at {fps} fps for up to {duration}s");
            } else {
                let geo = geometry
                    .map(|(w, h)| format!("{w}x{h}, "))
                    .unwrap_or_default();
                println!("wrote {output} ({geo}{frames} frames, {duration}s @ {fps}fps)");
            }
        }
        OutputMode::Json => {
            let mut payload = json!({
                "output": output,
                "fps": fps,
                "duration": duration,
                "codec": "h264",
                "region": region.map(|(x, y, w, h)| json!([x, y, w, h])),
                "dry_run": dry_run,
            });
            if let Some((w, h)) = geometry {
                payload["width"] = json!(w);
                payload["height"] = json!(h);
            }
            if !dry_run {
                payload["frames"] = json!(frames);
            }
            print_success(payload);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effective_duration_maps_zero_to_cap_and_clamps() {
        assert_eq!(effective_duration(0), MAX_DURATION_SECS);
        assert_eq!(effective_duration(10), 10);
        assert_eq!(effective_duration(MAX_DURATION_SECS), MAX_DURATION_SECS);
        assert_eq!(effective_duration(10_000), MAX_DURATION_SECS);
    }

    #[test]
    fn crop_rgba_full_frame_is_identity() {
        // 2x2 RGBA, distinct pixels.
        let src: Vec<u8> = (0..16).collect();
        let out = crop_rgba(&src, 2, (0, 0, 2, 2));
        assert_eq!(out, src);
    }

    #[test]
    fn crop_rgba_extracts_subrect_honouring_stride() {
        // 3x2 image; crop the bottom-right 2x1 starting at (1,1).
        // Row 0: px (0,0)=[0..4) (1,0)=[4..8) (2,0)=[8..12)
        // Row 1: px (0,1)=[12..16) (1,1)=[16..20) (2,1)=[20..24)
        let src: Vec<u8> = (0..24).collect();
        let out = crop_rgba(&src, 3, (1, 1, 2, 1));
        assert_eq!(out, (16..24).collect::<Vec<u8>>());
    }
}
