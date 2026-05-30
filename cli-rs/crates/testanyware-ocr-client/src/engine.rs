//! Platform-dispatched OCR engine selection.
//!
//! Per ADR-0002 the host picks the best *native* OCR facility for the
//! platform it runs on, rather than forcing one engine everywhere:
//!
//!   - **macOS**  → in-process Apple Vision by default; the EasyOCR
//!     daemon only when `TESTANYWARE_OCR_FALLBACK=1`.
//!   - **Linux / Windows** → the EasyOCR Python daemon
//!     ([`OcrChildBridge`]).
//!
//! The macOS Vision engine is not yet built — it lands in a follow-up
//! leaf (`040-macos-vision-ocr`). Until then every platform routes to the
//! daemon, but [`OcrEngine::detect`] already carries the `#[cfg]` seam
//! where the macOS arm will switch to Vision, so wiring it in later is
//! additive, not a rewrite.

use std::path::PathBuf;

use crate::bridge::{OcrBridgeError, OcrChildBridge, OcrChildBridgeConfig};
use crate::detection::OcrDetection;

/// Override the Python interpreter used for the EasyOCR daemon. Listed in
/// `docs/reference/env-vars.md`.
const PYTHON_ENV: &str = "TESTANYWARE_OCR_PYTHON";

/// Resolve the Python interpreter that hosts the EasyOCR daemon.
///
/// Mirrors the Swift `TestAnywareServer.resolveOCRInterpreterPath` chain:
///
/// 1. `TESTANYWARE_OCR_PYTHON` env override.
/// 2. Install layout: `<prefix>/libexec/venv/bin/python`, relative to the
///    running binary (`<prefix>/bin/testanyware`).
/// 3. Dev layout: a `pipeline/.venv/bin/python` in some ancestor of the
///    binary's directory.
/// 4. Fallback: `/usr/bin/python3` (likely to fail, but yields a clear
///    "interpreter not usable" error rather than a silent misconfigure).
pub fn resolve_interpreter() -> PathBuf {
    if let Some(p) = std::env::var_os(PYTHON_ENV) {
        if !p.is_empty() {
            return PathBuf::from(p);
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(bin_dir) = exe.parent() {
            // Install layout: <prefix>/bin/testanyware → <prefix>/libexec/...
            let installed = bin_dir.join("../libexec/venv/bin/python");
            if installed.is_file() {
                return installed;
            }
            // Dev layout: a checked-out `pipeline/.venv/bin/python` above us.
            let mut dir = Some(bin_dir);
            while let Some(d) = dir {
                let candidate = d.join("pipeline/.venv/bin/python");
                if candidate.is_file() {
                    return candidate;
                }
                dir = d.parent();
            }
        }
    }
    PathBuf::from("/usr/bin/python3")
}

/// A selected OCR engine. Today a single daemon-backed variant; the macOS
/// Vision variant joins it in the follow-up leaf (see module docs).
pub enum OcrEngine {
    /// EasyOCR Python daemon via the long-lived child bridge.
    Daemon(OcrChildBridge),
}

impl OcrEngine {
    /// Select the engine for the current platform and environment.
    pub fn detect() -> Self {
        // Per-platform native-facility selection (ADR-0002):
        //   macOS  → in-process Apple Vision by default; the EasyOCR
        //            daemon when TESTANYWARE_OCR_FALLBACK=1.
        //   others → EasyOCR daemon only.
        // The macOS Vision engine is not yet built (follow-up leaf
        // 040-macos-vision-ocr); until it lands the macOS arm also returns
        // the daemon. This `#[cfg]` block is the seam where Vision plugs in.
        #[cfg(target_os = "macos")]
        {
            // TODO(040-macos-vision-ocr): unless TESTANYWARE_OCR_FALLBACK=1,
            // return OcrEngine::Vision(..). Until then, fall through.
            Self::daemon()
        }
        #[cfg(not(target_os = "macos"))]
        {
            Self::daemon()
        }
    }

    fn daemon() -> Self {
        OcrEngine::Daemon(OcrChildBridge::new(OcrChildBridgeConfig::new(
            resolve_interpreter(),
        )))
    }

    /// The `engine` token reported in the `--json` envelope and the
    /// `OcrResponse.engine` field — `"easyocr_daemon"` or `"vision"`.
    pub fn engine_name(&self) -> &'static str {
        match self {
            OcrEngine::Daemon(_) => "easyocr_daemon",
        }
    }

    /// Recognise text in the supplied PNG bytes.
    pub async fn recognize(&self, png: &[u8]) -> Result<Vec<OcrDetection>, OcrBridgeError> {
        match self {
            OcrEngine::Daemon(bridge) => bridge.recognize(png).await,
        }
    }

    /// Terminate any long-lived subprocess. Idempotent.
    pub async fn shutdown(&self) {
        match self {
            OcrEngine::Daemon(bridge) => bridge.shutdown().await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_interpreter_honours_python_env_override() {
        let prev = std::env::var_os(PYTHON_ENV);
        std::env::set_var(PYTHON_ENV, "/opt/custom/python3");
        let resolved = resolve_interpreter();
        match prev {
            Some(v) => std::env::set_var(PYTHON_ENV, v),
            None => std::env::remove_var(PYTHON_ENV),
        }
        assert_eq!(resolved, PathBuf::from("/opt/custom/python3"));
    }

    #[test]
    fn detect_returns_a_daemon_engine_named_easyocr() {
        let engine = OcrEngine::detect();
        assert!(matches!(engine, OcrEngine::Daemon(_)));
        assert_eq!(engine.engine_name(), "easyocr_daemon");
    }
}
