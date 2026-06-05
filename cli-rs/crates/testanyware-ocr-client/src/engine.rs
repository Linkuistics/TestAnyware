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
//! The macOS Vision engine (ADR-0003, leaf `040-macos-vision-ocr`) is a
//! pure-Rust `objc2` binding in [`crate::vision`]; [`OcrEngine::detect`]
//! selects it on macOS unless `TESTANYWARE_OCR_FALLBACK=1`.

use std::path::PathBuf;

use crate::bridge::{OcrBridgeError, OcrChildBridge, OcrChildBridgeConfig};
use crate::detection::OcrDetection;

/// Override the Python interpreter used for the EasyOCR daemon. Listed in
/// `docs/reference/env-vars.md`.
const PYTHON_ENV: &str = "TESTANYWARE_OCR_PYTHON";

/// Force the EasyOCR daemon on macOS instead of in-process Apple Vision.
/// Set to `"1"`. Listed in `docs/reference/env-vars.md`.
#[cfg(target_os = "macos")]
const FALLBACK_ENV: &str = "TESTANYWARE_OCR_FALLBACK";

/// The venv-relative path to the Python interpreter for the current host.
/// A POSIX venv puts it at `bin/python`; a Windows venv at
/// `Scripts\python.exe` (the install layout `210` builds, mirrored for
/// Windows by `050`). Split out so both the install and dev layouts pick
/// the right sub-path.
#[cfg(not(target_os = "windows"))]
const VENV_PYTHON_REL: &str = "bin/python";
#[cfg(target_os = "windows")]
const VENV_PYTHON_REL: &str = "Scripts/python.exe";

/// Last-resort interpreter when no venv is found: a PATH-resolved
/// `python3` on Unix, `python` on Windows (the launcher name there). Both
/// are *likely* to fail for EasyOCR, but yield a clear "interpreter not
/// usable" error rather than a silent misconfigure.
#[cfg(not(target_os = "windows"))]
const FALLBACK_PYTHON: &str = "/usr/bin/python3";
#[cfg(target_os = "windows")]
const FALLBACK_PYTHON: &str = "python";

/// Resolve the Python interpreter that hosts the EasyOCR daemon.
///
/// Mirrors the Swift `TestAnywareServer.resolveOCRInterpreterPath` chain,
/// with per-host venv layout (`bin/python` vs `Scripts\python.exe`):
///
/// 1. `TESTANYWARE_OCR_PYTHON` env override (the self-hosted harness sets
///    this to the in-guest venv — ADR-0009 — so the harness path does not
///    depend on the auto-discovery below).
/// 2. Install layout: `<prefix>/libexec/venv/<venv-python>`, relative to
///    the running binary (`<prefix>/bin/testanyware`).
/// 3. Dev layout: a `pipeline/.venv/<venv-python>` in some ancestor of the
///    binary's directory.
/// 4. Fallback: the PATH-resolved host launcher.
pub fn resolve_interpreter() -> PathBuf {
    if let Some(p) = std::env::var_os(PYTHON_ENV) {
        if !p.is_empty() {
            return PathBuf::from(p);
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(bin_dir) = exe.parent() {
            // Install layout: <prefix>/bin/testanyware → <prefix>/libexec/...
            let installed = bin_dir.join("../libexec/venv").join(VENV_PYTHON_REL);
            if installed.is_file() {
                return installed;
            }
            // Dev layout: a checked-out `pipeline/.venv/...` above us.
            let mut dir = Some(bin_dir);
            while let Some(d) = dir {
                let candidate = d.join("pipeline/.venv").join(VENV_PYTHON_REL);
                if candidate.is_file() {
                    return candidate;
                }
                dir = d.parent();
            }
        }
    }
    PathBuf::from(FALLBACK_PYTHON)
}

/// A selected OCR engine. The daemon-backed variant exists on every
/// platform; the in-process Apple Vision variant is macOS-only (ADR-0003).
pub enum OcrEngine {
    /// EasyOCR Python daemon via the long-lived child bridge.
    Daemon(OcrChildBridge),
    /// In-process Apple Vision (macOS native, ADR-0003). Stateless: each
    /// `recognize` builds its own `VNImageRequestHandler`, so the variant
    /// carries no data.
    #[cfg(target_os = "macos")]
    Vision,
}

impl OcrEngine {
    /// Select the engine for the current platform and environment.
    pub fn detect() -> Self {
        // Per-platform native-facility selection (ADR-0002 / ADR-0003):
        //   macOS  → in-process Apple Vision by default; the EasyOCR
        //            daemon when TESTANYWARE_OCR_FALLBACK=1.
        //   others → EasyOCR daemon only.
        #[cfg(target_os = "macos")]
        {
            if fallback_requested() {
                Self::daemon()
            } else {
                OcrEngine::Vision
            }
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
            #[cfg(target_os = "macos")]
            OcrEngine::Vision => "vision",
        }
    }

    /// Recognise text in the supplied PNG bytes.
    pub async fn recognize(&self, png: &[u8]) -> Result<Vec<OcrDetection>, OcrBridgeError> {
        match self {
            OcrEngine::Daemon(bridge) => bridge.recognize(png).await,
            // Vision's `performRequests` is synchronous and blocking, and
            // its Objective-C objects are not `Send`. Run it on the
            // blocking pool so it never stalls the async reactor and only
            // the `Send` `Vec<OcrDetection>` crosses back.
            #[cfg(target_os = "macos")]
            OcrEngine::Vision => {
                let png = png.to_vec();
                tokio::task::spawn_blocking(move || crate::vision::recognize(&png))
                    .await
                    .map_err(|e| {
                        OcrBridgeError::PermanentlyUnavailable(format!(
                            "Vision OCR task failed to complete: {e}"
                        ))
                    })?
            }
        }
    }

    /// Terminate any long-lived subprocess. Idempotent. Vision holds no
    /// subprocess, so its arm is a no-op.
    pub async fn shutdown(&self) {
        match self {
            OcrEngine::Daemon(bridge) => bridge.shutdown().await,
            #[cfg(target_os = "macos")]
            OcrEngine::Vision => {}
        }
    }
}

/// Whether `TESTANYWARE_OCR_FALLBACK=1` asks macOS to use the daemon.
#[cfg(target_os = "macos")]
fn fallback_requested() -> bool {
    std::env::var_os(FALLBACK_ENV).as_deref() == Some(std::ffi::OsStr::new("1"))
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

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn detect_returns_a_daemon_engine_named_easyocr() {
        let engine = OcrEngine::detect();
        assert!(matches!(engine, OcrEngine::Daemon(_)));
        assert_eq!(engine.engine_name(), "easyocr_daemon");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn detect_picks_vision_by_default_and_daemon_on_fallback() {
        // Mutates a process-global env var. Only this test touches
        // FALLBACK_ENV, and `engine_name()` reads no other env, so it does
        // not race with the PYTHON_ENV test above.
        let prev = std::env::var_os(FALLBACK_ENV);

        std::env::remove_var(FALLBACK_ENV);
        assert_eq!(OcrEngine::detect().engine_name(), "vision");

        std::env::set_var(FALLBACK_ENV, "1");
        assert_eq!(OcrEngine::detect().engine_name(), "easyocr_daemon");

        // A value other than "1" must not trigger fallback.
        std::env::set_var(FALLBACK_ENV, "0");
        assert_eq!(OcrEngine::detect().engine_name(), "vision");

        match prev {
            Some(v) => std::env::set_var(FALLBACK_ENV, v),
            None => std::env::remove_var(FALLBACK_ENV),
        }
    }
}
