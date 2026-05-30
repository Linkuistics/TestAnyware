//! `screen size`, `screen capture`, and `screen find-text` handlers.
//!
//! `size`/`capture` open one RFB connection, run the handshake, request a
//! framebuffer update, then disconnect — one-shots. `find-text` adds an
//! OCR pass over the captured frame and, with `--timeout`, polls by
//! re-requesting frames on the same connection until a match appears.

use std::path::Path;
use std::time::{Duration, Instant};

use serde_json::json;
use testanyware_ocr_client::{find_text, FindOutcome, OcrDetection, OcrEngine};
use testanyware_rfb::{RfbConnection, ServerEvent};

use crate::output::{exit_code_for, print_error, print_success, OutputMode};
use crate::resolve::{resolve_vnc, ConnectionOptions, ResolveError};

/// `testanyware screen size` — print the framebuffer dimensions.
pub async fn run_screen_size(opts: ConnectionOptions, mode: OutputMode) {
    let endpoint = match resolve_vnc(&opts) {
        Ok(e) => e,
        Err(err) => exit_resolve_error(err, mode),
    };
    match RfbConnection::connect(&endpoint.host, endpoint.port, endpoint.password.as_deref().map(str::as_bytes)).await {
        Ok(conn) => {
            let (w, h) = conn.framebuffer_size();
            match mode {
                OutputMode::Text => println!("{w}x{h}"),
                OutputMode::Json => {
                    print_success(json!({ "width": w, "height": h }));
                }
            }
        }
        Err(err) => exit_rfb_error(err, mode),
    }
}

/// `testanyware screen capture` — capture one framebuffer update and
/// write it to disk as a PNG. `--region x,y,w,h` crops post-decode.
pub async fn run_screen_capture(
    opts: ConnectionOptions,
    output_path: Option<String>,
    region: Option<String>,
    mode: OutputMode,
) {
    let region = match region.as_deref().map(parse_region) {
        Some(Ok(r)) => Some(r),
        Some(Err(msg)) => {
            print_error(
                mode,
                "USAGE_ERROR",
                &format!("invalid --region: {msg}"),
                Some("Expected --region X,Y,W,H with non-negative integers."),
                json!({ "value": region.unwrap_or_default() }),
                2,
            );
        }
        None => None,
    };

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
    if let Err(err) = conn
        .request_framebuffer_update(false, 0, 0, fb_w as u16, fb_h as u16)
        .await
    {
        exit_rfb_error(err, mode);
    }

    // Drain server messages until at least one FramebufferUpdated arrives
    // with a rectangle. Some servers send no-op updates first.
    loop {
        match conn.next_message().await {
            Ok(ServerEvent::FramebufferUpdated { rectangles }) if rectangles > 0 => break,
            Ok(_) => continue,
            Err(err) => exit_rfb_error(err, mode),
        }
    }

    let fb = conn.framebuffer().clone();
    let crop = region.unwrap_or((0, 0, fb_w, fb_h));
    let png_bytes = match encode_png(&fb, crop) {
        Ok(b) => b,
        Err(err) => {
            print_error(
                mode,
                "INTERNAL",
                &format!("PNG encode failed: {err}"),
                None,
                json!({}),
                1,
            );
        }
    };

    let path = output_path.unwrap_or_else(|| "screen.png".to_string());
    if let Err(err) = std::fs::write(&path, &png_bytes) {
        print_error(
            mode,
            "IO_ERROR",
            &format!("failed to write {path}: {err}"),
            None,
            json!({ "path": path }),
            1,
        );
    }

    match mode {
        OutputMode::Text => println!("wrote {path} ({}x{})", crop.2, crop.3),
        OutputMode::Json => {
            print_success(json!({
                "path": Path::new(&path).display().to_string(),
                "width": crop.2,
                "height": crop.3,
            }));
        }
    }
}

/// `testanyware screen find-text` — capture the framebuffer, OCR it, and
/// report matches for `query` (case-insensitive substring), or every
/// recognized detection when `query` is `None`. With `timeout_secs`,
/// re-capture every 500ms until a match appears or the deadline expires.
///
/// Contract: default text mode (one detection per line); `--json` opt-in.
/// An empty result is exit 0 — except with `require_match`, which exits 3
/// (`TEXT_NOT_FOUND`). No-query dump mode caps at `limit` unless `all`.
pub async fn run_screen_find_text(
    opts: ConnectionOptions,
    query: Option<String>,
    timeout_secs: Option<u32>,
    require_match: bool,
    limit: Option<usize>,
    all: bool,
    mode: OutputMode,
) {
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

    let engine = OcrEngine::detect();
    let engine_name = engine.engine_name();
    let effective_limit = if all { usize::MAX } else { limit.unwrap_or(100) };
    let deadline = timeout_secs.map(|s| Instant::now() + Duration::from_secs(u64::from(s)));

    loop {
        // Request a fresh full-frame update on the live connection.
        if let Err(err) = conn
            .request_framebuffer_update(false, 0, 0, fb_w as u16, fb_h as u16)
            .await
        {
            engine.shutdown().await;
            exit_rfb_error(err, mode);
        }
        loop {
            match conn.next_message().await {
                Ok(ServerEvent::FramebufferUpdated { rectangles }) if rectangles > 0 => break,
                Ok(_) => continue,
                Err(err) => {
                    engine.shutdown().await;
                    exit_rfb_error(err, mode);
                }
            }
        }

        let fb = conn.framebuffer().clone();
        let png = match encode_png(&fb, (0, 0, fb_w, fb_h)) {
            Ok(b) => b,
            Err(err) => {
                engine.shutdown().await;
                print_error(
                    mode,
                    "INTERNAL",
                    &format!("PNG encode failed: {err}"),
                    None,
                    json!({}),
                    1,
                );
            }
        };

        let detections = match engine.recognize(&png).await {
            Ok(d) => d,
            Err(err) => {
                let code = err.code();
                engine.shutdown().await;
                print_error(
                    mode,
                    code,
                    &err.to_string(),
                    Some(ocr_remediation(code)),
                    json!({}),
                    exit_code_for(code),
                );
            }
        };

        let matched = match find_text(query.as_deref().unwrap_or(""), &detections) {
            FindOutcome::Found { matches, .. } => matches,
            FindOutcome::NotFound { .. } => Vec::new(),
        };
        let have_match = !matched.is_empty();

        // A no-query dump is "complete" on the first capture (it returns
        // whatever is on screen). A query keeps polling until a match or
        // the deadline; with no deadline it is a single shot.
        let timed_out = match deadline {
            Some(d) => Instant::now() >= d,
            None => true,
        };
        let done = have_match || query.is_none() || timed_out;

        if done {
            engine.shutdown().await;
            if query.is_some() && !have_match && require_match {
                print_error(
                    mode,
                    "TEXT_NOT_FOUND",
                    &format!("text {:?} not found on screen", query.as_deref().unwrap_or("")),
                    Some("Increase --timeout, or inspect the screen with `testanyware screen capture`."),
                    json!({ "query": query }),
                    3,
                );
            }
            emit_find_text(mode, query.as_deref(), engine_name, matched, effective_limit);
            return;
        }

        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

/// Emit a find-text result: structured envelope in JSON mode, one
/// detection per line (text output is not a parsing target) otherwise.
fn emit_find_text(
    mode: OutputMode,
    query: Option<&str>,
    engine: &str,
    detections: Vec<OcrDetection>,
    limit: usize,
) {
    match mode {
        OutputMode::Json => {
            print_success(find_text_envelope(query, engine, detections, limit));
        }
        OutputMode::Text => {
            let total = detections.len();
            for d in detections.iter().take(limit) {
                println!(
                    "{}\t({:.0},{:.0} {:.0}x{:.0}) conf={:.2}",
                    d.text, d.x, d.y, d.width, d.height, d.confidence
                );
            }
            if total > limit {
                println!("Showing {limit} of {total}. Use --limit N or --all to see more.");
            }
        }
    }
}

/// Remediation hint for the OCR error codes (§4.6).
fn ocr_remediation(code: &str) -> &'static str {
    match code {
        "OCR_UNAVAILABLE" => {
            "Ensure the EasyOCR daemon is installed (pipeline venv) and \
             set TESTANYWARE_OCR_PYTHON if the interpreter is elsewhere."
        }
        "OCR_CHILD_CRASHED" => "Re-run; if it persists, check the EasyOCR daemon logs.",
        "OCR_TIMEOUT" => "Increase the OCR deadline or reduce screen complexity, then retry.",
        _ => "See `testanyware llm-instructions`.",
    }
}

fn parse_region(value: &str) -> Result<(u32, u32, u32, u32), String> {
    let parts: Vec<&str> = value.split(',').collect();
    if parts.len() != 4 {
        return Err(format!("expected 4 comma-separated integers, got {}", parts.len()));
    }
    let mut out = [0u32; 4];
    for (i, p) in parts.iter().enumerate() {
        out[i] = p
            .trim()
            .parse::<u32>()
            .map_err(|_| format!("'{p}' is not a non-negative integer"))?;
    }
    let (x, y, w, h) = (out[0], out[1], out[2], out[3]);
    if w == 0 || h == 0 {
        return Err("width and height must be positive".into());
    }
    Ok((x, y, w, h))
}

fn encode_png(
    fb: &testanyware_rfb::Framebuffer,
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
    let img =
        image::RgbaImage::from_raw(w, h, cropped).expect("buffer length matches w*h*4");
    let mut out = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)?;
    Ok(out)
}

/// Build the `screen find-text --json` success payload — the fields
/// merged onto `{schema_version, ok:true}` by `print_success`.
///
/// `limit` caps the detection list and matters only in no-query dump mode
/// (§9.4); when a query is supplied the caller passes `usize::MAX`, so all
/// matches are returned and `truncated` stays `false`.
fn find_text_envelope(
    query: Option<&str>,
    engine: &str,
    mut detections: Vec<OcrDetection>,
    limit: usize,
) -> serde_json::Value {
    let total = detections.len();
    let truncated = total > limit;
    if truncated {
        detections.truncate(limit);
    }
    json!({
        "query": query,
        "engine": engine,
        "detections": detections,
        "returned": detections.len(),
        "total": total,
        "truncated": truncated,
    })
}

fn exit_resolve_error(err: ResolveError, mode: OutputMode) -> ! {
    let code = err.code();
    let message = err.to_string();
    print_error(mode, code, &message, None, err.details(), err.exit_code());
}

fn exit_rfb_error(err: testanyware_rfb::RfbError, mode: OutputMode) -> ! {
    use testanyware_rfb::RfbError;
    let (code, exit_code) = match &err {
        RfbError::Io(_) => ("CONNECTION_REFUSED", 1),
        RfbError::UnsupportedProtocolVersion(_) => ("INTERNAL", 1),
        RfbError::SecurityNegotiationFailed(_) => ("AUTH_REQUIRED", 4),
        RfbError::NoMutualSecurityType(_) => ("AUTH_REQUIRED", 4),
        RfbError::AuthFailed(_) => ("AUTH_REQUIRED", 4),
        RfbError::PasswordRequired => ("AUTH_REQUIRED", 4),
        RfbError::InvalidFramebufferSize { .. } => ("INTERNAL", 1),
        RfbError::UnexpectedMessageType(_) => ("INTERNAL", 1),
        RfbError::UnsupportedEncoding(_) => ("INTERNAL", 1),
        RfbError::Protocol(_) => ("INTERNAL", 1),
    };
    print_error(mode, code, &err.to_string(), None, json!({}), exit_code);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_region_ok() {
        assert_eq!(parse_region("0,0,800,600").unwrap(), (0, 0, 800, 600));
        assert_eq!(parse_region("10, 20, 30, 40").unwrap(), (10, 20, 30, 40));
    }

    #[test]
    fn parse_region_rejects_wrong_arity() {
        assert!(parse_region("1,2,3").is_err());
        assert!(parse_region("1,2,3,4,5").is_err());
    }

    #[test]
    fn parse_region_rejects_zero_dim() {
        assert!(parse_region("0,0,0,100").is_err());
        assert!(parse_region("0,0,100,0").is_err());
    }

    #[test]
    fn parse_region_rejects_non_integer() {
        assert!(parse_region("a,b,c,d").is_err());
        assert!(parse_region("0,0,-10,10").is_err());
    }

    fn det(text: &str) -> OcrDetection {
        OcrDetection::new(text, 1.0, 2.0, 3.0, 4.0, 0.9)
    }

    #[test]
    fn find_text_envelope_with_query_returns_all_matches_untruncated() {
        let dets = vec![det("Save"), det("Save All")];
        let env = find_text_envelope(Some("save"), "easyocr_daemon", dets, usize::MAX);
        assert_eq!(env["query"], "save");
        assert_eq!(env["engine"], "easyocr_daemon");
        assert_eq!(env["detections"].as_array().unwrap().len(), 2);
        assert_eq!(env["returned"], 2);
        assert_eq!(env["total"], 2);
        assert_eq!(env["truncated"], false);
    }

    #[test]
    fn find_text_envelope_no_query_truncates_to_limit_and_flags_it() {
        let dets = vec![det("a"), det("b"), det("c")];
        let env = find_text_envelope(None, "easyocr_daemon", dets, 2);
        assert!(env["query"].is_null());
        assert_eq!(env["detections"].as_array().unwrap().len(), 2);
        assert_eq!(env["returned"], 2);
        assert_eq!(env["total"], 3);
        assert_eq!(env["truncated"], true);
    }

    #[test]
    fn find_text_envelope_detection_carries_bbox_and_confidence() {
        let env = find_text_envelope(Some("x"), "easyocr_daemon", vec![det("x")], usize::MAX);
        let d = &env["detections"][0];
        assert_eq!(d["text"], "x");
        assert_eq!(d["x"], 1.0);
        assert_eq!(d["width"], 3.0);
        // confidence is f32; compare within epsilon after the f64 widening.
        assert!((d["confidence"].as_f64().unwrap() - 0.9).abs() < 1e-6);
    }
}
