//! Persistent OCR daemon health status.
//!
//! Mirrors `cli/Sources/TestAnywareDriver/OCR/OCRStatusFile.swift`. The
//! file is written when the daemon enters the
//! `PermanentlyUnavailable` state, and is consumed by `testanyware
//! doctor` to print a remediation message. The Swift docstring claims
//! the file is "cleared by `testanyware doctor` on recovery" — that
//! wiring is aspirational on the Swift side too (see memory
//! `doctor-ocrstatusfile-clear-on-recovery-is-aspirational-not-yet-wired`).
//!
//! Path strategy:
//!
//! - Swift hardcodes `~/Library/Application Support/testanyware/ocr-status.json`,
//!   which is macOS-only.
//! - The Rust port targets Linux primary, so we route through the
//!   `directories` crate: the project-data dir on Linux is
//!   `$XDG_DATA_HOME/testanyware/ocr-status.json` (or
//!   `~/.local/share/testanyware/...` if `XDG_DATA_HOME` is unset).
//! - On macOS the `ProjectDirs` data dir lands at
//!   `~/Library/Application Support/com.linkuistics.testanyware/`,
//!   which is *not* byte-equivalent to the Swift path. That's
//!   acceptable: the Rust port is a cross-platform replacement, and
//!   the file is internal — no caller outside the CLI reads it.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// On-disk status payload. `last_check` is an ISO 8601 timestamp
/// chosen by the writer; we don't enforce a specific format here, so
/// callers can pass whatever the rest of the CLI uses. The Swift code
/// uses ISO-8601 from `Date().description`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OcrStatus {
    pub degraded: bool,
    pub reason: String,
    pub last_check: String,
}

impl OcrStatus {
    pub fn new(degraded: bool, reason: impl Into<String>, last_check: impl Into<String>) -> Self {
        Self {
            degraded,
            reason: reason.into(),
            last_check: last_check.into(),
        }
    }
}

/// Read/write the OCR status file at a configurable path.
pub struct OcrStatusFile;

impl OcrStatusFile {
    /// Resolve the default path. Callers in tests should pass an
    /// explicit path to avoid touching the user's data dir.
    pub fn default_path() -> Option<PathBuf> {
        let dirs = directories::ProjectDirs::from("com", "linkuistics", "testanyware")?;
        Some(dirs.data_dir().join("ocr-status.json"))
    }

    pub fn read(path: &Path) -> Option<OcrStatus> {
        let bytes = std::fs::read(path).ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    pub fn write(status: &OcrStatus, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let bytes = serde_json::to_vec(status).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, e)
        })?;
        std::fs::write(path, bytes)
    }

    pub fn clear(path: &Path) {
        let _ = std::fs::remove_file(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp() -> tempfile::TempDir {
        tempfile::tempdir().expect("tempdir")
    }

    #[test]
    fn write_then_read_round_trips() {
        let dir = tmp();
        let path = dir.path().join("ocr-status.json");
        let status = OcrStatus::new(true, "easyocr not installed", "2026-05-04T10:00:00Z");
        OcrStatusFile::write(&status, &path).expect("write");
        let read = OcrStatusFile::read(&path).expect("read");
        assert_eq!(read, status);
    }

    #[test]
    fn read_returns_none_for_missing_file() {
        let dir = tmp();
        let path = dir.path().join("does-not-exist.json");
        assert!(OcrStatusFile::read(&path).is_none());
    }

    #[test]
    fn read_returns_none_for_malformed_json() {
        let dir = tmp();
        let path = dir.path().join("garbage.json");
        std::fs::write(&path, b"not json").unwrap();
        assert!(OcrStatusFile::read(&path).is_none());
    }

    #[test]
    fn write_creates_parent_directory_if_missing() {
        let dir = tmp();
        let path = dir.path().join("subdir").join("ocr-status.json");
        let status = OcrStatus::new(false, "ok", "2026-05-04");
        OcrStatusFile::write(&status, &path).expect("write should mkdir");
        assert!(path.exists());
    }

    #[test]
    fn clear_is_idempotent() {
        let dir = tmp();
        let path = dir.path().join("ocr-status.json");
        OcrStatusFile::clear(&path);
        OcrStatusFile::write(&OcrStatus::new(true, "x", "y"), &path).unwrap();
        assert!(path.exists());
        OcrStatusFile::clear(&path);
        assert!(!path.exists());
        // Calling again on an already-absent path is a no-op.
        OcrStatusFile::clear(&path);
    }

    #[test]
    fn default_path_is_resolved() {
        // Smoke test only — on CI/sandboxed environments without HOME,
        // the directories crate may return None. We accept either; the
        // important property is "it doesn't panic".
        let _ = OcrStatusFile::default_path();
    }
}
