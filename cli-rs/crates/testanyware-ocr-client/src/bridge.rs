//! Async actor managing a long-lived Python child process that holds
//! the EasyOCR reader warm between calls.
//!
//! Mirrors `cli/Sources/TestAnywareDriver/OCR/OCRChildBridge.swift`. The
//! Swift implementation uses the `actor` keyword to serialize calls; we
//! achieve the same with a `tokio::sync::Mutex` over the daemon state,
//! exposed through `&self` methods. One in-flight `recognize` per
//! bridge instance, by design.
//!
//! Lifecycle invariants (preserved from Swift):
//!
//! - **Cold start**: spawn child, write nothing until `{"ready": true}`
//!   arrives on stdout.
//! - **Warm path**: subsequent `recognize` calls reuse the same child.
//! - **Sticky unavailable**: if the daemon crashes or the import fails,
//!   the bridge records the reason and refuses further calls until
//!   restarted. Mirrors the contract that `OCR_DAEMON_UNAVAILABLE`
//!   means "give up, don't retry".
//! - **Temp file lifetime**: each `recognize` writes the PNG to a
//!   uniquely-named temp file under the system temp dir, hands the
//!   path to the daemon, and removes the file before returning. The
//!   daemon never holds a file descriptor across calls.

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout};
use tokio::sync::Mutex;
use tokio::time::timeout;

use crate::detection::OcrDetection;

/// Errors produced by the daemon bridge.
///
/// `code()` returns the contract §4.6 token surfaced in `--json` mode;
/// the enum variant captures the diagnostic context for the human
/// message and `details` payload.
#[derive(Debug, Error)]
pub enum OcrBridgeError {
    /// The daemon cannot be recovered (import error, missing
    /// interpreter, malformed handshake). Once latched, every
    /// subsequent `recognize` returns this error until the bridge is
    /// restarted; the host CLI should treat it as terminal.
    #[error("OCR daemon permanently unavailable: {0}")]
    PermanentlyUnavailable(String),

    /// The daemon process exited or its stdout pipe closed mid-call.
    /// Distinct from `PermanentlyUnavailable` so the caller can choose
    /// to retry on a fresh bridge.
    #[error("OCR daemon child process crashed")]
    ChildCrashed,

    /// The daemon did not respond within the configured deadline.
    /// Mapped to `OCR_TIMEOUT` (§5 exit code 7) by the CLI.
    #[error("OCR daemon did not respond in time")]
    ResponseTimeout,
}

impl OcrBridgeError {
    /// §4 / §5 catalogue token. Keep aligned with `output::exit_code_for`
    /// in `testanyware-cli`.
    pub fn code(&self) -> &'static str {
        match self {
            OcrBridgeError::PermanentlyUnavailable(_) => "OCR_UNAVAILABLE",
            OcrBridgeError::ChildCrashed => "OCR_CHILD_CRASHED",
            OcrBridgeError::ResponseTimeout => "OCR_TIMEOUT",
        }
    }
}

/// Configuration for `OcrChildBridge`. Mirrors Swift's
/// `OCRChildBridge.init` parameter list 1:1.
#[derive(Debug, Clone)]
pub struct OcrChildBridgeConfig {
    pub interpreter_path: PathBuf,
    pub daemon_arguments: Vec<String>,
    pub environment: Vec<(String, String)>,
    /// Deadline for the `{"ready": true}` handshake. Swift defaults to
    /// 8s; the EasyOCR import is the slow part on cold start.
    pub warm_deadline: Duration,
    /// Deadline for the first `recognize` after handshake — the
    /// runtime may need to allocate model buffers on first inference.
    /// Swift defaults to 15s.
    pub first_call_deadline: Duration,
    /// Deadline for any subsequent `recognize` call. Defaults to 15s
    /// matching `first_call_deadline`; the Swift code reuses the same
    /// timeout post-warmup, so we follow suit unless overridden.
    pub call_deadline: Duration,
}

impl OcrChildBridgeConfig {
    /// Sensible defaults for the canonical `python -m ocr_analyzer
    /// --daemon` invocation. Caller still has to pick the interpreter
    /// path — the Swift `ExecutablePath` resolver doesn't have a
    /// Rust counterpart yet (it would be a `which python3` chain).
    pub fn new(interpreter_path: impl Into<PathBuf>) -> Self {
        Self {
            interpreter_path: interpreter_path.into(),
            daemon_arguments: vec![
                "-m".into(),
                "ocr_analyzer".into(),
                "--daemon".into(),
            ],
            environment: Vec::new(),
            warm_deadline: Duration::from_secs(8),
            first_call_deadline: Duration::from_secs(15),
            call_deadline: Duration::from_secs(15),
        }
    }

    pub fn with_arguments(mut self, args: Vec<String>) -> Self {
        self.daemon_arguments = args;
        self
    }

    pub fn with_environment(mut self, env: Vec<(String, String)>) -> Self {
        self.environment = env;
        self
    }

    pub fn with_warm_deadline(mut self, d: Duration) -> Self {
        self.warm_deadline = d;
        self
    }

    pub fn with_first_call_deadline(mut self, d: Duration) -> Self {
        self.first_call_deadline = d;
        self
    }

    pub fn with_call_deadline(mut self, d: Duration) -> Self {
        self.call_deadline = d;
        self
    }
}

/// Tracks the daemon process plus the framed stdin/stdout it
/// communicates over. Held inside a `Mutex` so concurrent
/// `recognize` calls serialize.
struct ChildState {
    process: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    /// Set on permanent failure (sticky). Once latched, the state
    /// stays in this terminal mode for the lifetime of the bridge.
    sticky_unavailable: Option<String>,
    /// Whether the next call should use `first_call_deadline`. Flips
    /// to `false` after the first successful response.
    is_first_call: bool,
}

/// Long-lived OCR daemon bridge.
pub struct OcrChildBridge {
    config: OcrChildBridgeConfig,
    state: Arc<Mutex<Option<ChildState>>>,
    /// Sticky-unavailable reason captured even after the child handle
    /// is dropped. Read first by every `recognize` so the latching
    /// behaviour survives a child kill.
    sticky: Arc<Mutex<Option<String>>>,
}

impl OcrChildBridge {
    pub fn new(config: OcrChildBridgeConfig) -> Self {
        Self {
            config,
            state: Arc::new(Mutex::new(None)),
            sticky: Arc::new(Mutex::new(None)),
        }
    }

    /// Recognise text in the supplied PNG bytes. Spawns the child on
    /// first call; reuses the warm child afterwards.
    pub async fn recognize(&self, png_data: &[u8]) -> Result<Vec<OcrDetection>, OcrBridgeError> {
        if let Some(reason) = self.sticky.lock().await.clone() {
            return Err(OcrBridgeError::PermanentlyUnavailable(reason));
        }

        // Write PNG to a uniquely-named temp file. Drop guard removes
        // it whether the call succeeds or the daemon crashes mid-call.
        let tmp = TempPng::write(png_data).map_err(|e| {
            OcrBridgeError::PermanentlyUnavailable(format!(
                "failed to write OCR temp PNG: {e}"
            ))
        })?;

        let mut state_guard = self.state.lock().await;
        if state_guard.is_none() {
            let new_state = self.spawn_child().await?;
            *state_guard = Some(new_state);
        }
        let state = state_guard.as_mut().expect("just populated");

        let deadline = if state.is_first_call {
            self.config.first_call_deadline
        } else {
            self.config.call_deadline
        };

        let request_line = serde_json::to_string(&RequestEnvelope {
            image_path: tmp.path_str(),
        })
        .map_err(|e| {
            OcrBridgeError::PermanentlyUnavailable(format!("encode request: {e}"))
        })?;

        let result = call_once(state, &request_line, deadline).await;
        match &result {
            Ok(_) => {
                state.is_first_call = false;
            }
            Err(OcrBridgeError::PermanentlyUnavailable(reason)) => {
                // Latch the sticky bit *and* tear the child down so the
                // next call doesn't try to reuse a dead pipe.
                *self.sticky.lock().await = Some(reason.clone());
                kill_state(state_guard.take()).await;
            }
            Err(OcrBridgeError::ChildCrashed) => {
                kill_state(state_guard.take()).await;
            }
            Err(OcrBridgeError::ResponseTimeout) => {
                // A timed-out daemon call leaves the pipe in an
                // unknown state — we cannot safely send another
                // request without resync. Tear it down; the caller may
                // restart the bridge.
                kill_state(state_guard.take()).await;
            }
        }
        result
    }

    /// Terminate the child if running. Idempotent — calling twice is
    /// harmless. The bridge can be reused after `shutdown`; the next
    /// `recognize` will spawn a fresh child unless the sticky-
    /// unavailable bit has been latched.
    pub async fn shutdown(&self) {
        let taken = self.state.lock().await.take();
        kill_state(taken).await;
    }

    /// Sticky-unavailable reason if latched. For diagnostics and the
    /// `OcrStatusFile` writer.
    pub async fn sticky_reason(&self) -> Option<String> {
        self.sticky.lock().await.clone()
    }

    async fn spawn_child(&self) -> Result<ChildState, OcrBridgeError> {
        let mut cmd = tokio::process::Command::new(&self.config.interpreter_path);
        cmd.args(&self.config.daemon_arguments);
        for (k, v) in &self.config.environment {
            cmd.env(k, v);
        }
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        cmd.kill_on_drop(true);

        let mut process = cmd.spawn().map_err(|e| {
            let reason = format!("failed to spawn child: {e}");
            OcrBridgeError::PermanentlyUnavailable(reason)
        })?;

        let stdin = process
            .stdin
            .take()
            .ok_or_else(|| OcrBridgeError::PermanentlyUnavailable("no stdin pipe".into()))?;
        let stdout_raw = process
            .stdout
            .take()
            .ok_or_else(|| OcrBridgeError::PermanentlyUnavailable("no stdout pipe".into()))?;
        let mut stdout = BufReader::new(stdout_raw);

        // Wait for {"ready": true} within warm_deadline.
        let mut ready_line = String::new();
        let read_result = timeout(self.config.warm_deadline, stdout.read_line(&mut ready_line))
            .await;

        match read_result {
            Err(_) => {
                let _ = process.kill().await;
                let reason = format!(
                    "child did not signal ready within {:?}",
                    self.config.warm_deadline
                );
                *self.sticky.lock().await = Some(reason.clone());
                return Err(OcrBridgeError::PermanentlyUnavailable(reason));
            }
            Ok(Err(e)) => {
                let _ = process.kill().await;
                let reason = format!("read from child failed: {e}");
                *self.sticky.lock().await = Some(reason.clone());
                return Err(OcrBridgeError::PermanentlyUnavailable(reason));
            }
            Ok(Ok(0)) => {
                let _ = process.kill().await;
                let reason = "child exited before signaling ready".to_string();
                *self.sticky.lock().await = Some(reason.clone());
                return Err(OcrBridgeError::PermanentlyUnavailable(reason));
            }
            Ok(Ok(_)) => {}
        }

        let ready: ReadyEnvelope = match serde_json::from_str(ready_line.trim()) {
            Ok(r) => r,
            Err(_) => {
                let _ = process.kill().await;
                let reason = format!("child did not send ready signal: {:?}", ready_line);
                *self.sticky.lock().await = Some(reason.clone());
                return Err(OcrBridgeError::PermanentlyUnavailable(reason));
            }
        };
        if !ready.ready {
            let _ = process.kill().await;
            let reason = "child signalled ready=false".to_string();
            *self.sticky.lock().await = Some(reason.clone());
            return Err(OcrBridgeError::PermanentlyUnavailable(reason));
        }

        Ok(ChildState {
            process,
            stdin,
            stdout,
            sticky_unavailable: None,
            is_first_call: true,
        })
    }
}

async fn call_once(
    state: &mut ChildState,
    request_line: &str,
    deadline: Duration,
) -> Result<Vec<OcrDetection>, OcrBridgeError> {
    if state.sticky_unavailable.is_some() {
        return Err(OcrBridgeError::PermanentlyUnavailable(
            state
                .sticky_unavailable
                .clone()
                .expect("checked is_some above"),
        ));
    }

    if let Err(e) = write_line(&mut state.stdin, request_line).await {
        return Err(io_to_bridge_error(e));
    }

    let mut response_line = String::new();
    let read_result = timeout(deadline, state.stdout.read_line(&mut response_line)).await;
    match read_result {
        Err(_) => Err(OcrBridgeError::ResponseTimeout),
        Ok(Err(e)) => Err(io_to_bridge_error(e)),
        Ok(Ok(0)) => {
            // EOF — child died.
            Err(OcrBridgeError::ChildCrashed)
        }
        Ok(Ok(_)) => parse_response(&response_line),
    }
}

fn parse_response(line: &str) -> Result<Vec<OcrDetection>, OcrBridgeError> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        // Blank line is undefined in the protocol; the Swift
        // implementation treats this as a child crash signal.
        return Err(OcrBridgeError::ChildCrashed);
    }
    let value: serde_json::Value = serde_json::from_str(trimmed).map_err(|_| {
        // Malformed JSON ≠ canonical error envelope; treat as crash so
        // the caller can restart and retry on a fresh bridge.
        OcrBridgeError::ChildCrashed
    })?;

    if let Some(error_msg) = value.get("error").and_then(|v| v.as_str()) {
        // Daemon-reported error means the daemon decided it cannot
        // serve this request (or any further request). Latch sticky.
        return Err(OcrBridgeError::PermanentlyUnavailable(error_msg.to_string()));
    }

    let detections = value
        .get("detections")
        .and_then(|v| v.as_array())
        .ok_or(OcrBridgeError::ChildCrashed)?;

    let mut out = Vec::with_capacity(detections.len());
    for d in detections {
        let text = d
            .get("text")
            .and_then(|v| v.as_str())
            .or_else(|| d.get("label").and_then(|v| v.as_str()))
            .map(|s| s.to_string());
        let Some(text) = text else { continue };

        let (x, y, width, height) = if let Some(bbox) = d.get("bbox").and_then(|v| v.as_array()) {
            // bbox shape: [x_min, y_min, x_max, y_max] — width/height
            // computed by subtraction. Mirrors the Swift parser.
            if bbox.len() != 4 {
                continue;
            }
            let xs: Vec<f64> = bbox.iter().filter_map(|v| v.as_f64()).collect();
            if xs.len() != 4 {
                continue;
            }
            (xs[0], xs[1], xs[2] - xs[0], xs[3] - xs[1])
        } else {
            (
                d.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0),
                d.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0),
                d.get("width").and_then(|v| v.as_f64()).unwrap_or(0.0),
                d.get("height").and_then(|v| v.as_f64()).unwrap_or(0.0),
            )
        };
        let confidence = d
            .get("confidence")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0) as f32;
        out.push(OcrDetection::new(text, x, y, width, height, confidence));
    }
    Ok(out)
}

async fn write_line(stdin: &mut ChildStdin, line: &str) -> std::io::Result<()> {
    stdin.write_all(line.as_bytes()).await?;
    if !line.ends_with('\n') {
        stdin.write_all(b"\n").await?;
    }
    stdin.flush().await
}

fn io_to_bridge_error(e: std::io::Error) -> OcrBridgeError {
    use std::io::ErrorKind::*;
    match e.kind() {
        BrokenPipe | UnexpectedEof | ConnectionReset => OcrBridgeError::ChildCrashed,
        _ => OcrBridgeError::PermanentlyUnavailable(e.to_string()),
    }
}

async fn kill_state(state: Option<ChildState>) {
    let Some(mut state) = state else { return };
    // Closing stdin first lets a well-behaved daemon exit gracefully;
    // the kill() is the safety net for daemons that ignore EOF.
    drop(state.stdin);
    let _ = state.process.kill().await;
    let _ = state.process.wait().await;
}

/// Self-cleaning temp PNG holder. The drop impl removes the file so
/// that an early `?` return in `recognize` doesn't leak temp files
/// into the system temp dir.
struct TempPng {
    path: PathBuf,
}

impl TempPng {
    fn write(bytes: &[u8]) -> std::io::Result<Self> {
        // We want the daemon to read from a known path; tempfile's
        // NamedTempFile returns a handle that the daemon process
        // wouldn't be able to open by name on Windows-style locking
        // semantics. Use `Builder` + `into_temp_path` to materialize
        // a path while keeping the cleanup-on-drop guarantee.
        let named = tempfile::Builder::new()
            .prefix("testanyware-ocr-")
            .suffix(".png")
            .tempfile()?;
        // Replace contents with the supplied bytes (NamedTempFile is
        // empty by default).
        std::fs::write(named.path(), bytes)?;
        let path = named.into_temp_path();
        let kept = path.to_path_buf();
        // Persist by `keep`-ing — Drop on TempPath would unlink, but
        // we want our own Drop semantics so the path survives
        // long enough for the daemon to read it back.
        path.keep().map_err(|e| e.error)?;
        Ok(Self { path: kept })
    }

    fn path_str(&self) -> String {
        self.path.to_string_lossy().into_owned()
    }
}

impl Drop for TempPng {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

#[derive(Serialize)]
struct RequestEnvelope {
    image_path: String,
}

#[derive(Deserialize)]
struct ReadyEnvelope {
    #[serde(default)]
    ready: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_response_handles_x_y_w_h_form() {
        let line = r#"{"detections":[{"text":"hi","x":10,"y":20,"width":30,"height":40,"confidence":0.8}]}"#;
        let detections = parse_response(line).unwrap();
        assert_eq!(detections.len(), 1);
        assert_eq!(detections[0].text, "hi");
        assert_eq!(detections[0].x, 10.0);
        assert_eq!(detections[0].height, 40.0);
        assert!((detections[0].confidence - 0.8).abs() < 1e-6);
    }

    #[test]
    fn parse_response_handles_bbox_form_and_subtracts_to_wh() {
        // bbox is [x_min, y_min, x_max, y_max]; width/height derive
        // by subtraction. EasyOCR emits this form when configured
        // with bounding-polygon output.
        let line = r#"{"detections":[{"text":"hi","bbox":[10,20,40,60],"confidence":0.9}]}"#;
        let detections = parse_response(line).unwrap();
        assert_eq!(detections.len(), 1);
        let d = &detections[0];
        assert_eq!(d.x, 10.0);
        assert_eq!(d.y, 20.0);
        assert_eq!(d.width, 30.0);
        assert_eq!(d.height, 40.0);
    }

    #[test]
    fn parse_response_falls_back_to_label_when_text_missing() {
        // Some EasyOCR builds use `label` instead of `text`. Parity
        // with Swift's `dict["text"] ?? dict["label"]`.
        let line = r#"{"detections":[{"label":"alt","x":0,"y":0,"width":1,"height":1,"confidence":1}]}"#;
        let detections = parse_response(line).unwrap();
        assert_eq!(detections.len(), 1);
        assert_eq!(detections[0].text, "alt");
    }

    #[test]
    fn parse_response_skips_detections_with_neither_text_nor_label() {
        let line = r#"{"detections":[{"confidence":0.5},{"text":"keep","x":0,"y":0,"width":1,"height":1,"confidence":1}]}"#;
        let detections = parse_response(line).unwrap();
        assert_eq!(detections.len(), 1);
        assert_eq!(detections[0].text, "keep");
    }

    #[test]
    fn parse_response_treats_blank_line_as_child_crash() {
        let err = parse_response("").unwrap_err();
        assert!(matches!(err, OcrBridgeError::ChildCrashed));
    }

    #[test]
    fn parse_response_treats_invalid_json_as_child_crash() {
        let err = parse_response("not-valid-json").unwrap_err();
        assert!(matches!(err, OcrBridgeError::ChildCrashed));
    }

    #[test]
    fn parse_response_surfaces_daemon_error_as_permanent() {
        let line = r#"{"error":"easyocr not installed"}"#;
        let err = parse_response(line).unwrap_err();
        match err {
            OcrBridgeError::PermanentlyUnavailable(reason) => {
                assert_eq!(reason, "easyocr not installed");
            }
            other => panic!("expected PermanentlyUnavailable, got {other:?}"),
        }
    }

    #[test]
    fn error_codes_map_to_contract_tokens() {
        // The §4.6 catalogue token surfaced in --json output.
        assert_eq!(
            OcrBridgeError::PermanentlyUnavailable("x".into()).code(),
            "OCR_UNAVAILABLE"
        );
        assert_eq!(OcrBridgeError::ChildCrashed.code(), "OCR_CHILD_CRASHED");
        assert_eq!(OcrBridgeError::ResponseTimeout.code(), "OCR_TIMEOUT");
    }

    #[test]
    fn temp_png_writes_bytes_and_cleans_up() {
        let path: PathBuf;
        {
            let tmp = TempPng::write(&[0x89, 0x50, 0x4e, 0x47]).unwrap();
            path = tmp.path.clone();
            assert!(path.exists(), "temp file should exist while held");
            let bytes = std::fs::read(&path).unwrap();
            assert_eq!(bytes, vec![0x89, 0x50, 0x4e, 0x47]);
        }
        // Drop should have unlinked the file.
        assert!(!path.exists(), "temp file should be removed on drop");
    }
}
