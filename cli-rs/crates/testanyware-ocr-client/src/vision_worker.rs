//! The Vision worker: the child process that runs Apple Vision for the macOS
//! engine (ADR-0002).
//!
//! On macOS 27.2.1 a Vision request sometimes never returns, and every later
//! request of that process then waits behind it. Only a new process recovers
//! (`docs/research/vision-ocr-hang-macos-27.md`). So the engine sends each
//! request to this worker through [`crate::OcrChildBridge`], which stops the
//! worker after a timeout and starts a new one for the next request.
//!
//! The worker speaks the protocol of the bridge, the same as the EasyOCR
//! daemon: it writes `{"ready": true}`, then answers each
//! `{"image_path": ...}` line with one `{"detections": [...]}` line, or with
//! `{"error": ...}` when Vision cannot read the image. It stops at the end of
//! its input, which is when the parent closes the pipe or exits.

use std::io::{BufRead, Write};

use serde::Deserialize;

use crate::bridge::OcrBridgeError;

#[derive(Deserialize)]
struct Request {
    image_path: String,
}

/// Serve requests from `input` until it ends, and write each answer to
/// `output`. Only a failure of the pipes is an error.
pub fn serve(input: impl BufRead, mut output: impl Write) -> std::io::Result<()> {
    writeln!(output, r#"{{"ready": true}}"#)?;
    output.flush()?;
    for line in input.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        writeln!(output, "{}", answer(&line))?;
        output.flush()?;
    }
    Ok(())
}

/// The answer line to one request line.
fn answer(line: &str) -> String {
    let request: Request = match serde_json::from_str(line) {
        Ok(request) => request,
        Err(e) => return error_line(&format!("Vision worker: the request is not valid: {e}")),
    };
    let png = match std::fs::read(&request.image_path) {
        Ok(png) => png,
        Err(e) => {
            return error_line(&format!(
                "Vision worker: cannot read {}: {e}",
                request.image_path
            ))
        }
    };
    match crate::vision::recognize(&png) {
        Ok(detections) => serde_json::json!({ "detections": detections }).to_string(),
        Err(OcrBridgeError::PermanentlyUnavailable(reason)) => error_line(&reason),
        Err(e) => error_line(&e.to_string()),
    }
}

fn error_line(reason: &str) -> String {
    serde_json::json!({ "error": reason }).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The blank 16x16 PNG of the tests of `vision.rs`.
    const BLANK_PNG: &[u8] = &[
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x10, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f,
        0xf3, 0xff, 0x61, 0x00, 0x00, 0x00, 0x16, 0x49, 0x44, 0x41, 0x54, 0x78, 0xda, 0x63, 0xf8,
        0x4f, 0x21, 0x60, 0x18, 0x35, 0x60, 0xd4, 0x80, 0x51, 0x03, 0x86, 0x8b, 0x01, 0x00, 0x5d,
        0x78, 0xfc, 0x2e, 0xad, 0x21, 0xa9, 0x5e, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44,
        0xae, 0x42, 0x60, 0x82,
    ];

    fn serve_lines(input: &str) -> Vec<serde_json::Value> {
        let mut out = Vec::new();
        serve(input.as_bytes(), &mut out).expect("serve");
        String::from_utf8(out)
            .expect("utf-8")
            .lines()
            .map(|l| serde_json::from_str(l).expect("each line is JSON"))
            .collect()
    }

    #[test]
    fn signals_ready_and_stops_at_the_end_of_input() {
        let lines = serve_lines("");
        assert_eq!(lines, vec![serde_json::json!({ "ready": true })]);
    }

    #[test]
    fn answers_each_request_with_one_detections_line() {
        let dir = tempfile::tempdir().expect("tempdir");
        let png = dir.path().join("blank.png");
        std::fs::write(&png, BLANK_PNG).expect("write png");
        let request = serde_json::json!({ "image_path": png }).to_string();
        let lines = serve_lines(&format!("{request}\n\n{request}\n"));
        assert_eq!(
            lines.len(),
            3,
            "ready, then one answer for each request: {lines:?}"
        );
        for answer in &lines[1..] {
            assert_eq!(answer, &serde_json::json!({ "detections": [] }));
        }
    }

    #[test]
    fn answers_an_unreadable_image_with_an_error_line() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("not.png");
        std::fs::write(&path, b"definitely not a png").expect("write");
        let missing = dir.path().join("missing.png");
        let input = format!(
            "{}\n{}\nnot json\n",
            serde_json::json!({ "image_path": path }),
            serde_json::json!({ "image_path": missing }),
        );
        let lines = serve_lines(&input);
        assert_eq!(lines.len(), 4);
        for answer in &lines[1..] {
            let reason = answer["error"].as_str().expect("an error line");
            assert!(
                !reason.contains("permanently unavailable"),
                "no doubled prefix: {reason}"
            );
        }
    }
}
