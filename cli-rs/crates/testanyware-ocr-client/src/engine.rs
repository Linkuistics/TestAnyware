//! Platform-dispatched OCR engine selection.
//!
//! Per ADR-0002 the host picks the best *native* OCR facility for the
//! platform it runs on, rather than forcing one engine everywhere:
//!
//!   - **macOS**  → Apple Vision in a worker child by default (ADR-0002); the
//!     EasyOCR daemon only when `TESTANYWARE_OCR_FALLBACK=1`.
//!   - **Windows** → in-process native Windows.Media.Ocr, unconditionally
//!     (ADR-0011 — no fallback env; EasyOCR is uninstallable on win-arm64).
//!   - **Linux** → the EasyOCR Python daemon ([`OcrChildBridge`]).
//!
//! The macOS Vision engine (ADR-0003, leaf `040-macos-vision-ocr`) is a
//! pure-Rust `objc2` binding in [`crate::vision`]. It runs in the worker of
//! [`crate::vision_worker`], the hidden command `testanyware _vision-ocr`,
//! because a Vision request can hang its process on macOS 27.2.1 (ADR-0002).
//! [`OcrEngine::detect`] selects it on macOS unless
//! `TESTANYWARE_OCR_FALLBACK=1`. The Windows
//! engine (ADR-0011, leaf `220/070-windows-media-ocr-engine`) is a pure-Rust
//! WinRT binding in [`crate::windows_ocr`]; `detect()` selects it on Windows
//! unconditionally.

use std::path::PathBuf;
#[cfg(target_os = "macos")]
use std::time::Duration;

use crate::bridge::{OcrBridgeError, OcrChildBridge, OcrChildBridgeConfig};
use crate::detection::OcrDetection;

/// The hidden command of the `testanyware` binary that runs the Vision worker.
#[cfg(target_os = "macos")]
const VISION_WORKER_COMMAND: &str = "_vision-ocr";

/// The binary that runs the Vision worker, when it is not the running binary.
/// A test binary of the workspace sets it to `target/debug/testanyware`. Listed
/// in `docs/reference/env-vars.md`.
#[cfg(target_os = "macos")]
const VISION_WORKER_ENV: &str = "TESTANYWARE_VISION_WORKER";

/// The time for the worker to write `{"ready": true}`. It writes that line
/// before it loads Vision.
#[cfg(target_os = "macos")]
const VISION_READY_DEADLINE: Duration = Duration::from_secs(10);

/// The time for the first request of a worker. `aned` compiles the model of
/// Vision once for a new binary, which took 28 s to 36 s on 2026-11-22, so the
/// first request after an install or a build can take that long.
#[cfg(target_os = "macos")]
const VISION_FIRST_CALL_DEADLINE: Duration = Duration::from_secs(90);

/// The time for a later request. A frame of 1024x768 takes less than 0.3 s.
/// On macOS 27.2.1 some requests never return.
#[cfg(target_os = "macos")]
const VISION_CALL_DEADLINE: Duration = Duration::from_secs(30);

/// The worker binary: `TESTANYWARE_VISION_WORKER` when set, otherwise the
/// running binary, which is `testanyware` in production.
#[cfg(target_os = "macos")]
fn vision_worker_binary() -> PathBuf {
    match std::env::var_os(VISION_WORKER_ENV) {
        Some(p) if !p.is_empty() => PathBuf::from(p),
        _ => std::env::current_exe().unwrap_or_else(|_| PathBuf::from("testanyware")),
    }
}

/// The bridge configuration that runs `binary _vision-ocr` as the worker.
#[cfg(target_os = "macos")]
fn vision_worker_config(binary: &std::path::Path) -> OcrChildBridgeConfig {
    OcrChildBridgeConfig::new(binary)
        .with_arguments(vec![VISION_WORKER_COMMAND.into()])
        .with_warm_deadline(VISION_READY_DEADLINE)
        .with_first_call_deadline(VISION_FIRST_CALL_DEADLINE)
        .with_call_deadline(VISION_CALL_DEADLINE)
}

/// Send one request to the worker. After a timeout or a crash the bridge has
/// stopped the worker, so the request goes once more to a new worker. A hung
/// Vision request wedges its process, and a new process recovers.
#[cfg(target_os = "macos")]
async fn recognize_in_worker(
    bridge: &OcrChildBridge,
    worker: &std::path::Path,
    png: &[u8],
) -> Result<Vec<OcrDetection>, OcrBridgeError> {
    let result = match bridge.recognize(png).await {
        Err(e @ (OcrBridgeError::ResponseTimeout | OcrBridgeError::ChildCrashed)) => {
            eprintln!("testanyware: the Vision worker failed ({e}); trying a new worker");
            bridge.recognize(png).await
        }
        other => other,
    };
    result.map_err(|e| match e {
        OcrBridgeError::PermanentlyUnavailable(reason) => {
            OcrBridgeError::PermanentlyUnavailable(format!(
                "{reason} (Vision worker: {} {VISION_WORKER_COMMAND}; \
                 {VISION_WORKER_ENV} names another binary)",
                worker.display()
            ))
        }
        other => other,
    })
}

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
/// platform; the in-process native variants are platform-gated — Apple Vision
/// on macOS (ADR-0003), Windows.Media.Ocr on Windows (ADR-0011).
pub enum OcrEngine {
    /// EasyOCR Python daemon via the long-lived child bridge.
    Daemon(OcrChildBridge),
    /// Apple Vision (macOS native, ADR-0003) in a worker child (ADR-0002).
    /// `worker` is the binary that the bridge runs with `_vision-ocr`.
    #[cfg(target_os = "macos")]
    Vision {
        bridge: OcrChildBridge,
        worker: PathBuf,
    },
    /// In-process native Windows.Media.Ocr (ADR-0011). Stateless: each
    /// `recognize` builds its own WinRT `OcrEngine`, so the variant carries no
    /// data.
    #[cfg(windows)]
    WindowsMediaOcr,
}

impl OcrEngine {
    /// Select the engine for the current platform and environment.
    pub fn detect() -> Self {
        // Per-platform native-facility selection (ADR-0002 / ADR-0003 / ADR-0011):
        //   macOS   → Apple Vision in a worker child by default; the EasyOCR
        //             daemon when TESTANYWARE_OCR_FALLBACK=1.
        //   Windows → in-process native Windows.Media.Ocr, unconditionally
        //             (no fallback env — EasyOCR is uninstallable on win-arm64).
        //   Linux   → EasyOCR daemon only.
        #[cfg(target_os = "macos")]
        {
            if fallback_requested() {
                Self::daemon()
            } else {
                Self::vision(vision_worker_binary())
            }
        }
        #[cfg(windows)]
        {
            OcrEngine::WindowsMediaOcr
        }
        #[cfg(not(any(target_os = "macos", windows)))]
        {
            Self::daemon()
        }
    }

    // Constructed by `detect()` on macOS (fallback) and on Linux; on Windows
    // `detect()` always picks the native engine (ADR-0011), so the constructor
    // is unreferenced there.
    #[cfg_attr(windows, allow(dead_code))]
    fn daemon() -> Self {
        OcrEngine::Daemon(OcrChildBridge::new(OcrChildBridgeConfig::new(
            resolve_interpreter(),
        )))
    }

    /// The Vision engine with `worker` as the binary of its worker.
    #[cfg(target_os = "macos")]
    fn vision(worker: PathBuf) -> Self {
        let bridge = OcrChildBridge::new(vision_worker_config(&worker));
        OcrEngine::Vision { bridge, worker }
    }

    /// The `engine` token reported in the `--json` envelope and the
    /// `OcrResponse.engine` field — `"easyocr_daemon"`, `"vision"`, or
    /// `"windows_media_ocr"`.
    pub fn engine_name(&self) -> &'static str {
        match self {
            OcrEngine::Daemon(_) => "easyocr_daemon",
            #[cfg(target_os = "macos")]
            OcrEngine::Vision { .. } => "vision",
            #[cfg(windows)]
            OcrEngine::WindowsMediaOcr => "windows_media_ocr",
        }
    }

    /// Recognise text in the supplied PNG bytes.
    pub async fn recognize(&self, png: &[u8]) -> Result<Vec<OcrDetection>, OcrBridgeError> {
        match self {
            OcrEngine::Daemon(bridge) => bridge.recognize(png).await,
            #[cfg(target_os = "macos")]
            OcrEngine::Vision { bridge, worker } => recognize_in_worker(bridge, worker, png).await,
            // Windows.Media.Ocr's WinRT calls are synchronous (driven to
            // completion via IAsyncOperation::join) and its WinRT objects are
            // not `Send`, so run on the blocking pool and let only the `Send`
            // `Vec<OcrDetection>` cross back.
            #[cfg(windows)]
            OcrEngine::WindowsMediaOcr => {
                let png = png.to_vec();
                tokio::task::spawn_blocking(move || crate::windows_ocr::recognize(&png))
                    .await
                    .map_err(|e| {
                        OcrBridgeError::PermanentlyUnavailable(format!(
                            "Windows.Media.Ocr task failed to complete: {e}"
                        ))
                    })?
            }
        }
    }

    /// Terminate any long-lived subprocess: the daemon or the Vision worker.
    /// Idempotent. Windows.Media.Ocr runs in process, so its arm is a no-op.
    pub async fn shutdown(&self) {
        match self {
            OcrEngine::Daemon(bridge) => bridge.shutdown().await,
            #[cfg(target_os = "macos")]
            OcrEngine::Vision { bridge, .. } => bridge.shutdown().await,
            #[cfg(windows)]
            OcrEngine::WindowsMediaOcr => {}
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

    #[cfg(not(any(target_os = "macos", windows)))]
    #[test]
    fn detect_returns_a_daemon_engine_named_easyocr() {
        let engine = OcrEngine::detect();
        assert!(matches!(engine, OcrEngine::Daemon(_)));
        assert_eq!(engine.engine_name(), "easyocr_daemon");
    }

    #[cfg(windows)]
    #[test]
    fn detect_picks_windows_media_ocr_unconditionally() {
        // ADR-0011: Windows always uses the native engine — no fallback env.
        let engine = OcrEngine::detect();
        assert!(matches!(engine, OcrEngine::WindowsMediaOcr));
        assert_eq!(engine.engine_name(), "windows_media_ocr");
    }

    /// A Vision engine whose worker is the fake daemon in `behavior`, with
    /// deadlines of 1 s.
    #[cfg(target_os = "macos")]
    fn fake_vision_engine(behavior: &str, env: Vec<(String, String)>) -> OcrEngine {
        let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fake-ocr-daemon.sh");
        let mut environment = vec![("FAKE_OCR_BEHAVIOR".to_string(), behavior.to_string())];
        environment.extend(env);
        let config = OcrChildBridgeConfig::new("/bin/bash")
            .with_arguments(vec![script.to_string_lossy().into_owned()])
            .with_environment(environment)
            .with_warm_deadline(Duration::from_secs(5))
            .with_first_call_deadline(Duration::from_secs(1))
            .with_call_deadline(Duration::from_secs(1));
        OcrEngine::Vision {
            bridge: OcrChildBridge::new(config),
            worker: script,
        }
    }

    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn vision_survives_a_hung_worker_on_a_new_worker() {
        let dir = tempfile::tempdir().expect("tempdir");
        let marker = dir.path().join("first-child-hung");
        let engine = fake_vision_engine(
            "hang_first_child_then_echo",
            vec![(
                "FAKE_OCR_MARKER".into(),
                marker.to_string_lossy().into_owned(),
            )],
        );
        let detections = engine
            .recognize(&[0x89])
            .await
            .expect("the second worker answers");
        assert!(
            marker.exists(),
            "the first worker took the request and hung"
        );
        assert_eq!(detections[0].text, "fake");
        // The new worker stays and answers the next request.
        engine
            .recognize(&[0x89])
            .await
            .expect("the new worker answers again");
        engine.shutdown().await;
    }

    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn vision_gives_up_after_one_new_worker() {
        let engine = fake_vision_engine("ready_then_hang", Vec::new());
        let started = std::time::Instant::now();
        let err = engine.recognize(&[0x89]).await.unwrap_err();
        assert_eq!(err.code(), "OCR_TIMEOUT");
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "two attempts of 1 s each"
        );
        engine.shutdown().await;
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn the_vision_worker_is_the_running_binary_unless_named() {
        let prev = std::env::var_os(VISION_WORKER_ENV);
        std::env::remove_var(VISION_WORKER_ENV);
        assert_eq!(vision_worker_binary(), std::env::current_exe().unwrap());
        std::env::set_var(VISION_WORKER_ENV, "/opt/x/testanyware");
        assert_eq!(vision_worker_binary(), PathBuf::from("/opt/x/testanyware"));
        let config = vision_worker_config(&vision_worker_binary());
        assert_eq!(config.daemon_arguments, vec!["_vision-ocr".to_string()]);
        assert_eq!(config.first_call_deadline, VISION_FIRST_CALL_DEADLINE);
        assert_eq!(config.call_deadline, VISION_CALL_DEADLINE);
        match prev {
            Some(v) => std::env::set_var(VISION_WORKER_ENV, v),
            None => std::env::remove_var(VISION_WORKER_ENV),
        }
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
