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

/// `POST /upload` request body. Content is base64-encoded; the agent
/// writes the decoded bytes to `path`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UploadRequest {
    pub path: String,
    pub content: String,
}

/// `POST /download` request body.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DownloadRequest {
    pub path: String,
}

/// `POST /download` response body. Content is base64-encoded.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DownloadResponse {
    pub content: String,
}

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

    #[test]
    fn upload_request_round_trip() {
        let r = UploadRequest {
            path: "/tmp/x".into(),
            content: "aGVsbG8=".into(),
        };
        let json = serde_json::to_string(&r).unwrap();
        let back: UploadRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back, r);
    }
}
