//! Wire-format request and reply types for endpoints whose response is
//! not already a `SnapshotResponse` / `ActionResponse` / `InspectResponse`.
//!
//! Mirrors the inline `struct Req: Encodable` types declared per call site
//! in `cli/Sources/TestAnywareDriver/Agent/AgentTCPClient.swift`. Pulling
//! them out as named types in the Rust port lets us share the wire schema
//! with mock servers and integration tests.

use serde::{Deserialize, Serialize};

/// `POST /snapshot` request body.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SnapshotRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth: Option<i64>,
}

/// Element-targeting query body shared by `/inspect`, `/press`, `/focus`,
/// `/show-menu`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ElementQuery {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<i64>,
}

/// `POST /exec` request body.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecRequest {
    pub command: String,
    pub timeout: i64,
    pub detach: bool,
}

/// `POST /exec` response. The `timed_out` field is optional for
/// compatibility with agents built before it was added (mirrors Swift's
/// `timedOut: Bool?`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecResult {
    #[serde(rename = "exitCode")]
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    #[serde(rename = "timedOut", default, skip_serializing_if = "Option::is_none")]
    pub timed_out: Option<bool>,
}

impl ExecResult {
    pub fn succeeded(&self) -> bool {
        self.exit_code == 0
    }
}

/// `GET /health` response. The macOS agent emits `{accessible, platform}`;
/// the Linux and Windows agents conform to the same shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HealthResponse {
    pub accessible: bool,
    pub platform: String,
}

/// `POST /set-value` request body. Mirrors Swift's inline
/// `{role,label,window,id,index,value}` — the element query fields are
/// flattened alongside the literal `value` so the wire shape matches the
/// other element-targeting endpoints (`/inspect`, `/press`, `/focus`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetValueRequest {
    #[serde(flatten)]
    pub query: ElementQuery,
    pub value: String,
}

/// `POST /wait` request body: poll the agent until accessibility is ready
/// (optionally scoped to `window`), bounded by `timeout` seconds. Both
/// fields are omitted when unset, mirroring Swift's `{window?, timeout?}`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct WaitRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<i64>,
}

/// `POST /window-{focus,close,minimize}` request body — a bare window target.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WindowTarget {
    pub window: String,
}

/// `POST /window-resize` request body.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WindowResizeRequest {
    pub window: String,
    pub width: i64,
    pub height: i64,
}

/// `POST /window-move` request body.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WindowMoveRequest {
    pub window: String,
    pub x: i64,
    pub y: i64,
}

// File transfer (`/upload`, `/download`) carries no JSON request/response
// body: per ADR-0001 the path rides in a percent-encoded `?path=` query
// parameter and the file bytes stream raw over `application/octet-stream`.
// There are deliberately no `UploadRequest` / `DownloadRequest` /
// `DownloadResponse` types — the payload is the file itself. `/upload`
// still answers with `ActionResponse`; `/download` answers with the raw
// bytes on success or `ErrorResponse` on failure.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_request_omits_unset_fields() {
        let req = SnapshotRequest::default();
        let json = serde_json::to_string(&req).unwrap();
        assert_eq!(json, "{}");
    }

    #[test]
    fn snapshot_request_round_trip() {
        let req = SnapshotRequest {
            mode: Some("interact".into()),
            window: Some("Settings".into()),
            role: None,
            label: None,
            depth: Some(3),
        };
        let json = serde_json::to_string(&req).unwrap();
        let back: SnapshotRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back, req);
    }

    #[test]
    fn element_query_round_trip() {
        let q = ElementQuery {
            role: Some("button".into()),
            label: Some("Save".into()),
            window: None,
            id: None,
            index: None,
        };
        let json = serde_json::to_string(&q).unwrap();
        let back: ElementQuery = serde_json::from_str(&json).unwrap();
        assert_eq!(back, q);
    }

    #[test]
    fn exec_result_camel_case_keys() {
        let r = ExecResult {
            exit_code: 0,
            stdout: "hello\n".into(),
            stderr: String::new(),
            timed_out: Some(false),
        };
        let v: serde_json::Value = serde_json::from_str(&serde_json::to_string(&r).unwrap()).unwrap();
        let map = v.as_object().unwrap();
        assert!(map.contains_key("exitCode"));
        assert!(map.contains_key("timedOut"));
        assert!(!map.contains_key("exit_code"));
    }

    #[test]
    fn exec_result_omits_timed_out_when_none() {
        let r = ExecResult {
            exit_code: 0,
            stdout: String::new(),
            stderr: String::new(),
            timed_out: None,
        };
        let json = serde_json::to_string(&r).unwrap();
        assert!(!json.contains("timedOut"));
    }

    #[test]
    fn health_response_round_trip() {
        let h = HealthResponse {
            accessible: true,
            platform: "macos".into(),
        };
        let json = serde_json::to_string(&h).unwrap();
        let back: HealthResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(back, h);
    }

    // ----- agent action parity (port leaf 010) -----

    #[test]
    fn set_value_request_flattens_query_and_includes_value() {
        // Mirrors Swift's inline `{role,label,window,id,index,value}` body:
        // the element query is flattened alongside the literal `value`, with
        // unset query fields omitted (Swift's synthesized Codable uses
        // `encodeIfPresent`).
        let req = SetValueRequest {
            query: ElementQuery {
                role: Some("textfield".into()),
                label: Some("Email".into()),
                ..Default::default()
            },
            value: "a@b.com".into(),
        };
        let v: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&req).unwrap()).unwrap();
        assert_eq!(
            v,
            serde_json::json!({ "role": "textfield", "label": "Email", "value": "a@b.com" })
        );
    }

    #[test]
    fn set_value_request_round_trip() {
        let req = SetValueRequest {
            query: ElementQuery {
                id: Some("field-1".into()),
                index: Some(2),
                ..Default::default()
            },
            value: "hello".into(),
        };
        let back: SetValueRequest =
            serde_json::from_str(&serde_json::to_string(&req).unwrap()).unwrap();
        assert_eq!(back, req);
    }

    #[test]
    fn wait_request_omits_unset_fields() {
        let req = WaitRequest::default();
        assert_eq!(serde_json::to_string(&req).unwrap(), "{}");
    }

    #[test]
    fn wait_request_round_trip() {
        let req = WaitRequest {
            window: Some("Safari".into()),
            timeout: Some(10),
        };
        let back: WaitRequest =
            serde_json::from_str(&serde_json::to_string(&req).unwrap()).unwrap();
        assert_eq!(back, req);
    }

    #[test]
    fn window_target_serializes_single_field() {
        let req = WindowTarget {
            window: "Safari".into(),
        };
        assert_eq!(serde_json::to_string(&req).unwrap(), r#"{"window":"Safari"}"#);
    }

    #[test]
    fn window_resize_request_serializes_all_fields() {
        let req = WindowResizeRequest {
            window: "Safari".into(),
            width: 800,
            height: 600,
        };
        let v: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&req).unwrap()).unwrap();
        assert_eq!(
            v,
            serde_json::json!({ "window": "Safari", "width": 800, "height": 600 })
        );
    }

    #[test]
    fn window_move_request_serializes_all_fields() {
        let req = WindowMoveRequest {
            window: "Safari".into(),
            x: 100,
            y: 200,
        };
        let v: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&req).unwrap()).unwrap();
        assert_eq!(
            v,
            serde_json::json!({ "window": "Safari", "x": 100, "y": 200 })
        );
    }
}
