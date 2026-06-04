//! Integration tests for `OcrChildBridge` against the fake daemon
//! shell script `tests/fake-ocr-daemon.sh`.
//!
//! The fixture originated in the Swift test suite (`cli/Tests/Resources/`)
//! and moved into this crate when the Swift `cli/` tree was retired
//! (grove node `130`). It is the canonical fake EasyOCR daemon for the
//! Linux/Windows OCR path (`OcrEngine::Daemon`, ADR-0002): selecting a
//! `FAKE_OCR_BEHAVIOR` exercises each bridge failure mode (ready/echo,
//! die, malformed, hang, import-error) without a real Python interpreter.

use std::path::PathBuf;
use std::time::Duration;

use testanyware_ocr_client::{OcrBridgeError, OcrChildBridge, OcrChildBridgeConfig};

fn fake_daemon_script() -> PathBuf {
    // `CARGO_MANIFEST_DIR` is the OCR client crate root; the fixture lives
    // alongside this test under `tests/` (it moved here from the deleted
    // Swift `cli/Tests/Resources/` with grove node `130`).
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .join("tests/fake-ocr-daemon.sh")
        .canonicalize()
        .expect("fake-ocr-daemon.sh must exist under the crate's tests/ dir")
}

fn make_bridge(behavior: &str) -> OcrChildBridge {
    let config = OcrChildBridgeConfig::new("/bin/bash")
        .with_arguments(vec![fake_daemon_script().to_string_lossy().into_owned()])
        .with_environment(vec![("FAKE_OCR_BEHAVIOR".into(), behavior.into())])
        .with_warm_deadline(Duration::from_secs(5))
        .with_first_call_deadline(Duration::from_secs(10))
        .with_call_deadline(Duration::from_secs(10));
    OcrChildBridge::new(config)
}

#[tokio::test]
async fn cold_start_returns_detections() {
    let bridge = make_bridge("ready_then_echo");
    let detections = bridge
        .recognize(&[0x89, 0x50])
        .await
        .expect("ready_then_echo should succeed");
    assert!(!detections.is_empty());
    assert_eq!(detections[0].text, "fake");
    bridge.shutdown().await;
}

#[tokio::test]
async fn warm_path_reuses_same_child() {
    let bridge = make_bridge("ready_then_echo");
    let d1 = bridge.recognize(&[0x89]).await.expect("first");
    let d2 = bridge.recognize(&[0x89]).await.expect("second");
    assert_eq!(d1.len(), 1);
    assert_eq!(d2.len(), 1);
    bridge.shutdown().await;
}

#[tokio::test]
async fn shutdown_is_idempotent() {
    let bridge = make_bridge("ready_then_echo");
    let _ = bridge.recognize(&[0x89]).await.expect("first call");
    bridge.shutdown().await;
    bridge.shutdown().await; // no panic
}

#[tokio::test]
async fn import_error_yields_permanently_unavailable_and_latches() {
    let bridge = make_bridge("import_error");
    let err = bridge
        .recognize(&[0x89])
        .await
        .expect_err("import_error should fail");
    match err {
        OcrBridgeError::PermanentlyUnavailable(_) => {}
        other => panic!("expected PermanentlyUnavailable, got {other:?}"),
    }
    // Sticky: a second call returns the same terminal state without
    // re-spawning.
    let again = bridge
        .recognize(&[0x89])
        .await
        .expect_err("sticky should hold");
    assert!(matches!(
        again,
        OcrBridgeError::PermanentlyUnavailable(_)
    ));
    assert!(bridge.sticky_reason().await.is_some());
}

#[tokio::test]
async fn ready_then_die_on_first_call_surfaces_child_crashed() {
    // Daemon signals ready, then exits before answering the first
    // request. We hand off the request and the read returns EOF.
    let bridge = make_bridge("ready_then_die_on_request");
    let err = bridge
        .recognize(&[0x89])
        .await
        .expect_err("daemon dies on first call");
    assert!(matches!(err, OcrBridgeError::ChildCrashed));
}

#[tokio::test]
async fn malformed_response_surfaces_child_crashed() {
    let bridge = make_bridge("ready_then_malformed");
    let err = bridge
        .recognize(&[0x89])
        .await
        .expect_err("malformed JSON should be a crash");
    assert!(matches!(err, OcrBridgeError::ChildCrashed));
}

#[tokio::test]
async fn response_timeout_when_daemon_hangs() {
    let bridge = OcrChildBridge::new(
        OcrChildBridgeConfig::new("/bin/bash")
            .with_arguments(vec![fake_daemon_script().to_string_lossy().into_owned()])
            .with_environment(vec![(
                "FAKE_OCR_BEHAVIOR".into(),
                "ready_then_hang".into(),
            )])
            .with_warm_deadline(Duration::from_secs(5))
            // Tight call deadline — the daemon will sleep forever, so
            // this is the path under test.
            .with_first_call_deadline(Duration::from_millis(300))
            .with_call_deadline(Duration::from_millis(300)),
    );
    let err = bridge
        .recognize(&[0x89])
        .await
        .expect_err("hanging daemon should time out");
    assert!(matches!(err, OcrBridgeError::ResponseTimeout));
}

#[tokio::test]
async fn warm_deadline_exceeded_is_permanently_unavailable() {
    let bridge = OcrChildBridge::new(
        OcrChildBridgeConfig::new("/bin/bash")
            .with_arguments(vec![fake_daemon_script().to_string_lossy().into_owned()])
            .with_environment(vec![(
                "FAKE_OCR_BEHAVIOR".into(),
                "hang_forever".into(),
            )])
            .with_warm_deadline(Duration::from_millis(300))
            .with_first_call_deadline(Duration::from_secs(2))
            .with_call_deadline(Duration::from_secs(2)),
    );
    let err = bridge
        .recognize(&[0x89])
        .await
        .expect_err("daemon never signals ready");
    assert!(matches!(err, OcrBridgeError::PermanentlyUnavailable(_)));
}
