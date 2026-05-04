//! Integration tests for `AgentClient` driven by `wiremock`.
//!
//! Each test stands up a local mock HTTP server, points the client at it,
//! and asserts that the request matches the expected wire shape and that
//! responses decode into the right typed result. This is the contract the
//! Rust client and the in-VM agent (Swift / Python / C#) share — drift on
//! either side surfaces here.

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use serde_json::json;
use testanyware_agent_client::{AgentClient, AgentConfig, AgentError};
use testanyware_protocol::{ElementQuery, ExecRequest, SnapshotRequest};
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn client_for(server: &MockServer) -> AgentClient {
    // server.uri() is `http://<host>:<port>`; split off the scheme and
    // parse the authority by hand so this test crate doesn't pull in
    // a `url` dependency just for two fields.
    let uri = server.uri();
    let authority = uri
        .strip_prefix("http://")
        .or_else(|| uri.strip_prefix("https://"))
        .expect("mock server uri starts with http(s)");
    let (host, port) = authority
        .rsplit_once(':')
        .expect("mock server uri has explicit port");
    let port: u16 = port.parse().expect("port is u16");
    AgentClient::new(AgentConfig::new(host, port)).expect("client builds")
}

#[tokio::test]
async fn health_returns_structured_body() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/health"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "accessible": true,
            "platform": "macos",
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    let health = client.health().await.expect("health call succeeds");
    assert!(health.accessible);
    assert_eq!(health.platform, "macos");
}

#[tokio::test]
async fn windows_decodes_snapshot_response() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/windows"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "windows": [
                {
                    "title": "My App",
                    "windowType": "window",
                    "sizeWidth": 800,
                    "sizeHeight": 600,
                    "positionX": 0,
                    "positionY": 0,
                    "appName": "MyApp",
                    "focused": true
                }
            ]
        })))
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    let response = client.windows().await.expect("windows call succeeds");
    assert_eq!(response.windows.len(), 1);
    assert_eq!(response.windows[0].title.as_deref(), Some("My App"));
    assert!(response.windows[0].focused);
}

#[tokio::test]
async fn snapshot_sends_only_set_fields() {
    let server = MockServer::start().await;
    let request = SnapshotRequest {
        mode: Some("interact".into()),
        window: Some("Settings".into()),
        role: None,
        label: None,
        depth: None,
    };
    Mock::given(method("POST"))
        .and(path("/snapshot"))
        .and(body_json(json!({ "mode": "interact", "window": "Settings" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "windows": [] })))
        .expect(1)
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    let response = client
        .snapshot(&request)
        .await
        .expect("snapshot call succeeds");
    assert!(response.windows.is_empty());
}

#[tokio::test]
async fn inspect_decodes_full_response() {
    let server = MockServer::start().await;
    let query = ElementQuery {
        role: Some("button".into()),
        label: Some("Save".into()),
        ..Default::default()
    };
    Mock::given(method("POST"))
        .and(path("/inspect"))
        .and(body_json(json!({ "role": "button", "label": "Save" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "element": {
                "role": "button",
                "label": "Save",
                "enabled": true,
                "focused": false,
                "childCount": 0,
                "actions": ["AXPress"],
            },
            "boundsX": 100,
            "boundsY": 200,
            "boundsWidth": 80,
            "boundsHeight": 30
        })))
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    let response = client.inspect(&query).await.expect("inspect succeeds");
    assert_eq!(response.element.label.as_deref(), Some("Save"));
    assert_eq!(response.bounds(), Some((100.0, 200.0, 80.0, 30.0)));
}

#[tokio::test]
async fn press_returns_action_response() {
    let server = MockServer::start().await;
    let query = ElementQuery {
        role: Some("button".into()),
        label: Some("OK".into()),
        ..Default::default()
    };
    Mock::given(method("POST"))
        .and(path("/press"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "success": true,
            "message": "Pressed button \"OK\""
        })))
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    let response = client.press(&query).await.expect("press succeeds");
    assert!(response.success);
    assert_eq!(response.message.as_deref(), Some("Pressed button \"OK\""));
}

#[tokio::test]
async fn exec_returns_full_result() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/exec"))
        .and(body_json(json!({
            "command": "echo hi",
            "timeout": 10,
            "detach": false
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "exitCode": 0,
            "stdout": "hi\n",
            "stderr": "",
            "timedOut": false
        })))
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    let result = client
        .exec(&ExecRequest {
            command: "echo hi".into(),
            timeout: 10,
            detach: false,
        })
        .await
        .expect("exec succeeds");
    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout, "hi\n");
    assert!(result.succeeded());
}

#[tokio::test]
async fn upload_round_trips_base64_content() {
    let server = MockServer::start().await;
    let payload = b"binary\x00\x01\x02".to_vec();
    let encoded = BASE64_STANDARD.encode(&payload);

    Mock::given(method("POST"))
        .and(path("/upload"))
        .and(body_json(json!({ "path": "/tmp/x.bin", "content": encoded })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "success": true,
            "message": "Uploaded"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    client
        .upload("/tmp/x.bin", &payload)
        .await
        .expect("upload succeeds");
}

#[tokio::test]
async fn upload_failure_surfaces_as_wire_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/upload"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "success": false,
            "message": "Upload failed: permission denied"
        })))
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    let err = client
        .upload("/forbidden", b"hi")
        .await
        .expect_err("upload should fail");
    assert_eq!(err.code(), "UPLOAD_FAILED");
}

#[tokio::test]
async fn download_decodes_base64_content() {
    let server = MockServer::start().await;
    let payload = b"hello world\n";
    let encoded = BASE64_STANDARD.encode(payload);

    Mock::given(method("POST"))
        .and(path("/download"))
        .and(body_json(json!({ "path": "/tmp/x.bin" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "content": encoded })))
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    let bytes = client
        .download("/tmp/x.bin")
        .await
        .expect("download succeeds");
    assert_eq!(bytes, payload);
}

#[tokio::test]
async fn download_error_envelope_maps_to_wire_error() {
    // Some agent versions return HTTP 200 with an `{error, details}`
    // envelope instead of an HTTP 500. The client must recognise both.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/download"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "error": "download_failed",
            "details": "ENOENT"
        })))
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    let err = client
        .download("/missing")
        .await
        .expect_err("download should fail");
    assert_eq!(err.code(), "DOWNLOAD_FAILED");
}

#[tokio::test]
async fn http_500_surfaces_as_http_status() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/press"))
        .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    let err = client
        .press(&ElementQuery::default())
        .await
        .expect_err("press should fail");
    assert!(matches!(err, AgentError::HttpStatus { .. }));
}
