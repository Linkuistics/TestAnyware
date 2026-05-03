//! Cross-language contract test.
//!
//! Each fixture under `cli-rs/tests/fixtures/protocol/` is a canonical JSON
//! document that both the Swift and Rust sides must accept. The Rust
//! parser must:
//!
//!   1. Decode the document into the typed Rust struct.
//!   2. Re-encode the typed value to a `serde_json::Value`.
//!   3. Produce a value that is semantically equal to the original
//!      parsed JSON value (recursive object/array equality).
//!
//! Step (3) catches the failure modes that matter: a renamed field
//! disappears from the re-encoded value; a dropped field disappears; a
//! field whose JSON type is wrong fails at step (1). The matching Swift
//! test (`CrossLangFixturesTests.swift`) does the equivalent on its side.

use std::fs;
use std::path::{Path, PathBuf};

use testanyware_protocol::{
    ActionResponse, ElementInfo, ErrorResponse, InspectResponse, SnapshotResponse, WindowInfo,
};

fn fixture_dir() -> PathBuf {
    // CARGO_MANIFEST_DIR points at crates/testanyware-protocol; fixtures
    // live two levels up at cli-rs/tests/fixtures/protocol.
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    crate_root
        .join("..")
        .join("..")
        .join("tests")
        .join("fixtures")
        .join("protocol")
}

fn load(path: impl AsRef<Path>) -> (String, serde_json::Value) {
    let full = fixture_dir().join(path);
    let raw = fs::read_to_string(&full)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", full.display()));
    let value: serde_json::Value = serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("failed to parse {} as JSON: {e}", full.display()));
    (raw, value)
}

/// Decode -> re-encode -> compare. Panics with a useful diff on mismatch.
fn assert_round_trip<T>(name: &str, raw: &str, original: &serde_json::Value)
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    let typed: T = serde_json::from_str(raw)
        .unwrap_or_else(|e| panic!("[{name}] could not decode into {}: {e}", std::any::type_name::<T>()));
    let re_encoded = serde_json::to_value(&typed)
        .unwrap_or_else(|e| panic!("[{name}] could not re-encode {}: {e}", std::any::type_name::<T>()));
    if !json_semantic_eq(original, &re_encoded) {
        panic!(
            "[{name}] round-trip mismatch for {}:\n  original:    {}\n  re-encoded: {}",
            std::any::type_name::<T>(),
            serde_json::to_string_pretty(original).unwrap(),
            serde_json::to_string_pretty(&re_encoded).unwrap(),
        );
    }
}

/// Recursive structural equality with numeric-tolerance: integers and
/// whole-number floats are equal (`8` == `8.0`). Swift's `JSONEncoder`
/// emits whole-number `Double`s as integer literals, while serde-json
/// always emits `Double` as `<n>.0`. Without this normalisation, every
/// fixture with whole-number coordinates fails comparison even though
/// the numbers are identical to f64 precision.
fn json_semantic_eq(a: &serde_json::Value, b: &serde_json::Value) -> bool {
    use serde_json::Value;
    match (a, b) {
        (Value::Number(an), Value::Number(bn)) => {
            // f64 covers any integer up to 2^53; coordinates are
            // well within that, so this is lossless.
            match (an.as_f64(), bn.as_f64()) {
                (Some(af), Some(bf)) => af == bf,
                _ => an == bn,
            }
        }
        (Value::Object(ao), Value::Object(bo)) => {
            if ao.len() != bo.len() {
                return false;
            }
            ao.iter()
                .all(|(k, av)| bo.get(k).is_some_and(|bv| json_semantic_eq(av, bv)))
        }
        (Value::Array(aa), Value::Array(ba)) => {
            aa.len() == ba.len()
                && aa.iter().zip(ba.iter()).all(|(x, y)| json_semantic_eq(x, y))
        }
        _ => a == b,
    }
}

#[test]
fn element_info_full() {
    let (raw, original) = load("element-info-full.json");
    assert_round_trip::<ElementInfo>("element-info-full", &raw, &original);
}

#[test]
fn element_info_minimal() {
    let (raw, original) = load("element-info-minimal.json");
    assert_round_trip::<ElementInfo>("element-info-minimal", &raw, &original);
}

#[test]
fn element_info_with_children() {
    let (raw, original) = load("element-info-with-children.json");
    assert_round_trip::<ElementInfo>("element-info-with-children", &raw, &original);
}

#[test]
fn window_info_with_title() {
    let (raw, original) = load("window-info-with-title.json");
    assert_round_trip::<WindowInfo>("window-info-with-title", &raw, &original);
}

#[test]
fn window_info_without_title() {
    let (raw, original) = load("window-info-without-title.json");
    assert_round_trip::<WindowInfo>("window-info-without-title", &raw, &original);
}

#[test]
fn window_info_with_elements() {
    let (raw, original) = load("window-info-with-elements.json");
    assert_round_trip::<WindowInfo>("window-info-with-elements", &raw, &original);
}

#[test]
fn snapshot_response_typical() {
    let (raw, original) = load("snapshot-response-typical.json");
    assert_round_trip::<SnapshotResponse>("snapshot-response-typical", &raw, &original);
}

#[test]
fn snapshot_response_empty() {
    let (raw, original) = load("snapshot-response-empty.json");
    assert_round_trip::<SnapshotResponse>("snapshot-response-empty", &raw, &original);
}

#[test]
fn action_response_success() {
    let (raw, original) = load("action-response-success.json");
    assert_round_trip::<ActionResponse>("action-response-success", &raw, &original);
}

#[test]
fn action_response_failure() {
    let (raw, original) = load("action-response-failure.json");
    assert_round_trip::<ActionResponse>("action-response-failure", &raw, &original);
}

#[test]
fn error_response_with_details() {
    let (raw, original) = load("error-response-with-details.json");
    assert_round_trip::<ErrorResponse>("error-response-with-details", &raw, &original);
}

#[test]
fn error_response_no_details() {
    let (raw, original) = load("error-response-no-details.json");
    assert_round_trip::<ErrorResponse>("error-response-no-details", &raw, &original);
}

#[test]
fn inspect_response_full() {
    let (raw, original) = load("inspect-response-full.json");
    assert_round_trip::<InspectResponse>("inspect-response-full", &raw, &original);
}

#[test]
fn inspect_response_minimal() {
    let (raw, original) = load("inspect-response-minimal.json");
    assert_round_trip::<InspectResponse>("inspect-response-minimal", &raw, &original);
}
