//! Serve HTTP/1.1 on the socket.

use std::convert::Infallible;
use std::path::PathBuf;
use std::sync::Arc;

use bytes::Bytes;
use http_body_util::Full;
use hyper::body::Incoming;
use hyper::header::{HeaderValue, CONTENT_TYPE};
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use serde_json::{json, Value};
use testanyware_app_protocol::hello::{BUILD_HEADER, PROTOCOL_HEADER};
use testanyware_app_protocol::{AppError, ClientHello, StatusResponse};
use tokio::net::UnixListener;

use crate::access::peer_allowed;
use crate::gate::{check_request, AppIdentity};

/// What each connection needs to know about the app.
#[derive(Debug)]
pub struct AppContext {
    pub identity: AppIdentity,
    /// The uid of the user who runs the app. A peer with another uid gets no service.
    pub own_uid: u32,
    /// The configuration path of the run. Nothing reads it yet: the configuration leaf does.
    pub config_path: Option<PathBuf>,
}

/// Accept connections until the listener fails for good. Each connection runs in its own task.
pub async fn serve(listener: UnixListener, ctx: Arc<AppContext>) -> std::io::Result<()> {
    loop {
        let (stream, _) = match listener.accept().await {
            Ok(accepted) => accepted,
            // A full descriptor table or an aborted connection must not stop the app.
            Err(e) if is_transient(&e) => {
                eprintln!("testanyware-app: accept failed: {e}");
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                continue;
            }
            Err(e) => return Err(e),
        };
        let ctx = Arc::clone(&ctx);
        tokio::spawn(async move {
            // The kernel states the peer's uid. Dropping the stream closes the connection
            // before it reads a byte.
            match stream.peer_cred() {
                Ok(cred) if peer_allowed(cred.uid(), ctx.own_uid) => {}
                _ => return,
            }
            let service = hyper::service::service_fn(move |req| handle(Arc::clone(&ctx), req));
            let conn = hyper::server::conn::http1::Builder::new()
                .serve_connection(TokioIo::new(stream), service);
            if let Err(e) = conn.await {
                eprintln!("testanyware-app: connection ended: {e}");
            }
        });
    }
}

fn is_transient(e: &std::io::Error) -> bool {
    use std::io::ErrorKind::*;
    matches!(
        e.kind(),
        ConnectionAborted | ConnectionReset | Interrupted | WouldBlock | OutOfMemory
    ) || matches!(e.raw_os_error(), Some(code) if code == 23 || code == 24) // ENFILE, EMFILE
}

async fn handle(
    ctx: Arc<AppContext>,
    req: Request<Incoming>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let hello =
        ClientHello::from_lookup(|name| req.headers().get(name).and_then(|v| v.to_str().ok()));
    let mut response = route(&ctx, req.method(), req.uri().path(), &hello);
    // Every response states the app's range and build.
    let headers = response.headers_mut();
    headers.insert(
        PROTOCOL_HEADER,
        header_value(&ctx.identity.protocol_range.to_string()),
    );
    headers.insert(BUILD_HEADER, header_value(&ctx.identity.build));
    Ok(response)
}

fn route(
    ctx: &AppContext,
    method: &Method,
    path: &str,
    hello: &ClientHello,
) -> Response<Full<Bytes>> {
    // `doctor` needs the status of an app it cannot otherwise talk to, so this request
    // skips the checks and changes nothing.
    if method == Method::GET && path == "/v1/status" {
        let body = StatusResponse {
            build: ctx.identity.build.clone(),
            protocol_range: ctx.identity.protocol_range,
            directories: ctx.identity.directories.clone(),
        };
        return json_response(
            StatusCode::OK,
            serde_json::to_value(body).expect("status serialises"),
        );
    }
    if let Err(err) = check_request(&ctx.identity, hello) {
        return error_response(StatusCode::CONFLICT, &err);
    }
    // The guest routes come in the guest-streams leaf. USAGE_ERROR is a code that exists.
    let err = AppError::new(
        "USAGE_ERROR",
        format!("The TestAnyware app has no route for {method} {path}."),
        None,
        json!({ "method": method.as_str(), "path": path }),
    );
    error_response(StatusCode::NOT_FOUND, &err)
}

fn error_response(status: StatusCode, err: &AppError) -> Response<Full<Bytes>> {
    json_response(status, serde_json::to_value(err).expect("error serialises"))
}

fn json_response(status: StatusCode, body: Value) -> Response<Full<Bytes>> {
    let mut response = Response::new(Full::new(Bytes::from(body.to_string())));
    *response.status_mut() = status;
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    response
}

fn header_value(s: &str) -> HeaderValue {
    HeaderValue::from_str(s).unwrap_or_else(|_| HeaderValue::from_static("unknown"))
}
