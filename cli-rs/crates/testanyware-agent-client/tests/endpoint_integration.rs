//! Integration tests for `AgentClient` driven by `wiremock`.
//!
//! Each test stands up a local mock HTTP server, points the client at it,
//! and asserts that the request matches the expected wire shape and that
//! responses decode into the right typed result. This is the contract the
//! Rust client and the in-VM agent (Swift / Python / C#) share — drift on
//! either side surfaces here.

use serde_json::json;
use testanyware_agent_client::{AgentClient, AgentConfig, AgentError};
use testanyware_protocol::{
    ElementQuery, ExecRequest, SetValueRequest, SnapshotRequest, WaitRequest, WindowMoveRequest,
    WindowResizeRequest, WindowTarget,
};
use wiremock::matchers::{body_bytes, body_json, header, method, path, query_param};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

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
async fn upload_streams_raw_octet_stream_body_with_path_query() {
    let server = MockServer::start().await;
    let payload = b"binary\x00\x01\x02".to_vec();

    // ADR-0001 wire form: raw octet-stream body, path in the `?path=` query.
    // A space in the path must encode as %20 (not +) for cross-agent parity.
    Mock::given(method("POST"))
        .and(path("/upload"))
        .and(query_param("path", "/tmp/my docs/x.bin"))
        .and(header("content-type", "application/octet-stream"))
        .and(body_bytes(payload.clone()))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "success": true,
            "message": "Uploaded"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let src = tempfile::NamedTempFile::new().expect("temp src");
    std::fs::write(src.path(), &payload).expect("write src");

    let client = client_for(&server).await;
    let sent = client
        .upload("/tmp/my docs/x.bin", src.path())
        .await
        .expect("upload succeeds");
    assert_eq!(sent, payload.len() as u64);
}

/// Regression for the Linux/`http.server` agent: the upload body MUST be
/// framed with a `Content-Length` (not `Transfer-Encoding: chunked`). Python's
/// `http.server` reads only `Content-Length` and silently writes a 0-byte file
/// for a chunked body — so a streamed upload with no length advertised loses
/// all data against that agent. The client knows the file size, so it must
/// advertise it. (Discovered by leaf 070 e2e against a real Linux VM.)
#[tokio::test]
async fn upload_frames_body_with_content_length_not_chunked() {
    let server = MockServer::start().await;
    let payload = vec![0xABu8; 4096];
    let len = payload.len();

    Mock::given(method("POST"))
        .and(path("/upload"))
        .and(header("content-length", len.to_string().as_str()))
        .and(|req: &Request| {
            // A chunked body would carry no Content-Length and instead a
            // `Transfer-Encoding: chunked` header — assert that is absent.
            req.headers
                .get("transfer-encoding")
                .map(|v| !v.to_str().unwrap_or("").contains("chunked"))
                .unwrap_or(true)
        })
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "success": true })))
        .expect(1)
        .mount(&server)
        .await;

    let src = tempfile::NamedTempFile::new().expect("temp src");
    std::fs::write(src.path(), &payload).expect("write src");

    let client = client_for(&server).await;
    client
        .upload("/tmp/x.bin", src.path())
        .await
        .expect("upload succeeds with Content-Length framing");
}

#[tokio::test]
async fn upload_failure_surfaces_as_wire_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/upload"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "error": "upload_failed",
            "details": "permission denied"
        })))
        .mount(&server)
        .await;

    let src = tempfile::NamedTempFile::new().expect("temp src");
    std::fs::write(src.path(), b"hi").expect("write src");

    let client = client_for(&server).await;
    let err = client
        .upload("/forbidden", src.path())
        .await
        .expect_err("upload should fail");
    assert_eq!(err.code(), "UPLOAD_FAILED");
}

#[tokio::test]
async fn upload_missing_local_file_is_io_error() {
    // No HTTP call should be needed — the local open fails first.
    let server = MockServer::start().await;
    let client = client_for(&server).await;
    let err = client
        .upload("/tmp/x.bin", std::path::Path::new("/no/such/local/file"))
        .await
        .expect_err("upload should fail on missing source");
    assert_eq!(err.code(), "IO_ERROR");
}

#[tokio::test]
async fn download_streams_octet_stream_body_to_local_file() {
    let server = MockServer::start().await;
    let payload = b"hello world\n".to_vec();

    Mock::given(method("POST"))
        .and(path("/download"))
        .and(query_param("path", "/tmp/x.bin"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/octet-stream")
                .set_body_bytes(payload.clone()),
        )
        .mount(&server)
        .await;

    let dir = tempfile::tempdir().expect("temp dir");
    let dest = dir.path().join("out.bin");

    let client = client_for(&server).await;
    let written = client
        .download("/tmp/x.bin", &dest)
        .await
        .expect("download succeeds");
    assert_eq!(written, payload.len() as u64);
    assert_eq!(std::fs::read(&dest).expect("read dest"), payload);
    // No temp files should remain in the destination directory.
    let leftovers: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
        .collect();
    assert!(leftovers.is_empty(), "temp files left behind: {leftovers:?}");
}

#[tokio::test]
async fn download_error_envelope_maps_to_wire_error_and_leaves_no_file() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/download"))
        .respond_with(ResponseTemplate::new(404).set_body_json(json!({
            "error": "download_failed",
            "details": "ENOENT"
        })))
        .mount(&server)
        .await;

    let dir = tempfile::tempdir().expect("temp dir");
    let dest = dir.path().join("out.bin");

    let client = client_for(&server).await;
    let err = client
        .download("/missing", &dest)
        .await
        .expect_err("download should fail");
    assert_eq!(err.code(), "DOWNLOAD_FAILED");
    // A failed download must not create the destination at all.
    assert!(!dest.exists(), "destination should not exist after failure");
}

/// Belt-and-braces: assert no `+` ever appears in the encoded query for a
/// path with spaces, by capturing the raw request URL.
#[tokio::test]
async fn upload_query_uses_percent20_not_plus() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/upload"))
        .and(|req: &Request| !req.url.query().unwrap_or_default().contains('+'))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "success": true })))
        .expect(1)
        .mount(&server)
        .await;

    let src = tempfile::NamedTempFile::new().expect("temp src");
    std::fs::write(src.path(), b"x").expect("write src");

    let client = client_for(&server).await;
    client
        .upload("/a b/c d.bin", src.path())
        .await
        .expect("upload succeeds");
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

// ---------------------------------------------------------------------------
// agent action parity (port leaf 010): set-value, focus, wait, window-*
// ---------------------------------------------------------------------------

#[tokio::test]
async fn set_value_posts_flattened_body_and_decodes_action() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/set-value"))
        .and(body_json(
            json!({ "role": "textfield", "label": "Email", "value": "a@b.com" }),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "success": true,
            "message": "Set value"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    let req = SetValueRequest {
        query: ElementQuery {
            role: Some("textfield".into()),
            label: Some("Email".into()),
            ..Default::default()
        },
        value: "a@b.com".into(),
    };
    let response = client.set_value(&req).await.expect("set-value succeeds");
    assert!(response.success);
    assert_eq!(response.message.as_deref(), Some("Set value"));
}

#[tokio::test]
async fn focus_posts_element_query_and_decodes_action() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/focus"))
        .and(body_json(json!({ "role": "button", "label": "OK" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "success": true })))
        .expect(1)
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    let query = ElementQuery {
        role: Some("button".into()),
        label: Some("OK".into()),
        ..Default::default()
    };
    let response = client.focus(&query).await.expect("focus succeeds");
    assert!(response.success);
}

#[tokio::test]
async fn wait_posts_window_and_timeout_and_decodes_action() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/wait"))
        .and(body_json(json!({ "window": "Safari", "timeout": 10 })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "success": true,
            "message": "ready"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    let req = WaitRequest {
        window: Some("Safari".into()),
        timeout: Some(10),
    };
    let response = client.wait(&req).await.expect("wait succeeds");
    assert!(response.success);
}

#[tokio::test]
async fn window_focus_posts_window_target() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/window-focus"))
        .and(body_json(json!({ "window": "Safari" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "success": true })))
        .expect(1)
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    let response = client
        .window_focus(&WindowTarget {
            window: "Safari".into(),
        })
        .await
        .expect("window-focus succeeds");
    assert!(response.success);
}

#[tokio::test]
async fn window_resize_posts_dimensions() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/window-resize"))
        .and(body_json(
            json!({ "window": "Safari", "width": 800, "height": 600 }),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "success": true })))
        .expect(1)
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    let response = client
        .window_resize(&WindowResizeRequest {
            window: "Safari".into(),
            width: 800,
            height: 600,
        })
        .await
        .expect("window-resize succeeds");
    assert!(response.success);
}

#[tokio::test]
async fn window_move_posts_position() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/window-move"))
        .and(body_json(json!({ "window": "Safari", "x": 100, "y": 200 })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "success": true })))
        .expect(1)
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    let response = client
        .window_move(&WindowMoveRequest {
            window: "Safari".into(),
            x: 100,
            y: 200,
        })
        .await
        .expect("window-move succeeds");
    assert!(response.success);
}

#[tokio::test]
async fn window_close_posts_window_target() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/window-close"))
        .and(body_json(json!({ "window": "Safari" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "success": true })))
        .expect(1)
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    let response = client
        .window_close(&WindowTarget {
            window: "Safari".into(),
        })
        .await
        .expect("window-close succeeds");
    assert!(response.success);
}

#[tokio::test]
async fn window_minimize_posts_window_target() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/window-minimize"))
        .and(body_json(json!({ "window": "Safari" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "success": true })))
        .expect(1)
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    let response = client
        .window_minimize(&WindowTarget {
            window: "Safari".into(),
        })
        .await
        .expect("window-minimize succeeds");
    assert!(response.success);
}

/// A logical agent failure is reported as a `{error, details}` envelope;
/// `decode_body` maps the wire token to the §4.5 code (here WINDOW_NOT_FOUND).
#[tokio::test]
async fn window_action_error_envelope_maps_to_window_not_found() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/window-focus"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "error": "window_not_found",
            "details": "no window titled Safari"
        })))
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    let err = client
        .window_focus(&WindowTarget {
            window: "Safari".into(),
        })
        .await
        .expect_err("window-focus should surface the wire error");
    assert_eq!(err.code(), "WINDOW_NOT_FOUND");
}
