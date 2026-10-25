//! Serve HTTP/1.1 on the socket.

use std::convert::Infallible;
use std::sync::{Arc, Mutex};

use bytes::Bytes;
use http_body_util::{BodyExt, Limited};
use hyper::body::Incoming;
use hyper::header::{HeaderValue, CONNECTION, CONTENT_TYPE, UPGRADE};
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use serde_json::{json, Value};
use testanyware_app_protocol::hello::{BUILD_HEADER, BUNDLE_BUILD_HEADER, PROTOCOL_HEADER};
use testanyware_app_protocol::{
    older_than_bundle, parse_end_path, parse_stream_path, AppError, ClientHello, EndRoute,
    ListRequest, LoginItemState, RouteMatch, StatusResponse, StreamKind, StreamTarget, LIST_PATH,
    STREAM_UPGRADE_TOKEN,
};
use testanyware_vm::tools::ToolSearch;
use tokio::io::AsyncWriteExt;
use tokio::net::{UnixListener, UnixStream};

use crate::access::peer_allowed;
use crate::body::AppBody;
use crate::bundle::BundleSource;
use crate::config::{stat_file, AppConfig};
use crate::gate::{check_request, AppIdentity};
use crate::grant::GrantObservation;
use crate::streams::{
    close_guest, invalid_endpoint, open_guest_stream, relay, resolve_endpoint, vm_not_found,
    ClientWatch, ServeError,
};

/// What each connection needs to know about the app.
#[derive(Debug)]
pub struct AppContext {
    pub identity: AppIdentity,
    /// Where the build of the bundle on disk comes from. A cask upgrade replaces the bundle
    /// under a running app, so each request reads it afresh.
    pub bundle: BundleSource,
    /// The uid of the user who runs the app. A peer with another uid gets no service.
    pub own_uid: u32,
    /// What the app made of `config.rhai` when it started. A restart of the app reads the file
    /// again.
    pub config: AppConfig,
    /// The last observation of the Local Network grant. Only a dial to an attached network
    /// changes it.
    pub grant: Mutex<GrantObservation>,
    /// Where the app looks for `tart`, QEMU and the other tools it runs. The app has no shell
    /// environment, so it reads no `PATH`.
    pub tools: ToolSearch,
    /// The port of the In-VM agent in a tart guest. A test moves it (`--test-agent-port`).
    pub agent_port: u16,
    /// The ids that are being started now, so that one id has one start at a time.
    pub starts: crate::gate_start::StartGate,
    /// The queue for the macOS guest slots. A `vm start` of a macOS guest waits in it.
    pub admission: Arc<crate::admission::AdmissionQueue>,
    /// What the app did with each VM it found when it started.
    pub adoption: crate::adoption::AdoptionLog,
    /// How long a probe of an adopted VM may take. A test shortens it.
    pub probe_deadline: std::time::Duration,
    /// Asks macOS for the status of the login item, each time `GET /v1/status` needs it. The
    /// menu bar app sets it. A run with no login item, such as the test mode, states
    /// `unobserved`.
    pub login_item: LoginItemProbe,
}

/// A function that reads the status of the app's login item now.
pub type LoginItemProbe = fn() -> LoginItemState;

impl AppContext {
    pub fn new(identity: AppIdentity, own_uid: u32, config: AppConfig) -> Self {
        let admission = crate::admission::AdmissionQueue::for_paths(
            testanyware_vm::VmPaths::new(
                identity.directories.state_dir.clone(),
                identity.directories.data_dir.clone(),
                std::env::temp_dir(),
            ),
            config.macos_slots(),
        );
        Self {
            identity,
            bundle: BundleSource::this_process(),
            own_uid,
            config,
            grant: Mutex::new(GrantObservation::NotYetSeen),
            tools: ToolSearch::builtin(),
            agent_port: testanyware_vm::start_context::AGENT_PORT,
            starts: crate::gate_start::StartGate::default(),
            admission,
            adoption: crate::adoption::AdoptionLog::default(),
            probe_deadline: crate::adoption::PROBE_DEADLINE,
            login_item: || LoginItemState::Unobserved,
        }
    }

    /// The same context with a probe of the login item.
    pub fn with_login_item(mut self, probe: LoginItemProbe) -> Self {
        self.login_item = probe;
        self
    }

    /// The body of `GET /v1/status`. It reads state and changes none.
    pub fn status(&self) -> StatusResponse {
        let bundle_build = self.bundle.build();
        StatusResponse {
            build: self.identity.build.clone(),
            protocol_range: self.identity.protocol_range,
            directories: self.identity.directories.clone(),
            older_than_bundle: older_than_bundle(&self.identity.build, bundle_build.as_deref()),
            bundle_build,
            adoption: self.adoption.snapshot(),
            configuration: self.config.status(&stat_file),
            executable: std::env::current_exe().ok(),
            local_network: crate::grant::local_network_status(
                &self
                    .grant
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()),
            ),
            login_item: (self.login_item)(),
        }
    }

    /// The same context with another source of the bundle build. A test forges one.
    pub fn with_bundle(mut self, bundle: BundleSource) -> Self {
        self.bundle = bundle;
        self
    }

    /// The same context with another deadline for the probes of adoption.
    pub fn with_probe_deadline(mut self, deadline: std::time::Duration) -> Self {
        self.probe_deadline = deadline;
        self
    }

    /// The same context with another tool search. A test passes a directory of fake tools.
    pub fn with_tools(mut self, tools: ToolSearch) -> Self {
        self.tools = tools;
        self
    }

    /// The same context with the agent probes and the agent endpoint of a start on another
    /// port. A test points them at a fake agent, or at a port that refuses.
    pub fn with_agent_port(mut self, port: u16) -> Self {
        self.agent_port = port;
        self
    }
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
            // A guest stream upgrades the connection, so the connection must allow upgrades.
            // https://docs.rs/hyper/1.9.0/hyper/server/conn/http1/struct.Connection.html#method.with_upgrades
            let conn = hyper::server::conn::http1::Builder::new()
                .serve_connection(TokioIo::new(stream), service)
                .with_upgrades();
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
    mut req: Request<Incoming>,
) -> Result<Response<AppBody>, Infallible> {
    let hello =
        ClientHello::from_lookup(|name| req.headers().get(name).and_then(|v| v.to_str().ok()));
    let mut response = route(&ctx, &mut req, &hello).await;
    // Every response states the app's range and build.
    let headers = response.headers_mut();
    headers.insert(
        PROTOCOL_HEADER,
        header_value(&ctx.identity.protocol_range.to_string()),
    );
    headers.insert(BUILD_HEADER, header_value(&ctx.identity.build));
    // The bundle on disk may be newer than this app. A client needs the build to word the
    // remedy of a skew, and an app with no bundle states none.
    if let Some(bundle_build) = ctx.bundle.build() {
        headers.insert(BUNDLE_BUILD_HEADER, header_value(&bundle_build));
    }
    Ok(response)
}

async fn route(
    ctx: &Arc<AppContext>,
    req: &mut Request<Incoming>,
    hello: &ClientHello,
) -> Response<AppBody> {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    // `doctor` needs the status of an app it cannot otherwise talk to, so this request
    // skips the checks and changes nothing.
    if method == Method::GET && path == "/v1/status" {
        let body = ctx.status();
        return json_response(
            StatusCode::OK,
            serde_json::to_value(body).expect("status serialises"),
        );
    }
    if let Err(err) = check_request(
        &ctx.identity,
        hello,
        ctx.bundle.build().as_deref(),
        // Read the file only for a request that fails on its directories.
        &|| ctx.config.status(&stat_file),
    ) {
        return error_response(StatusCode::CONFLICT, &err);
    }
    if path == LIST_PATH && method == Method::GET {
        return list_vms(ctx, req.uri().query()).await;
    }
    if path == LIST_PATH && method == Method::POST {
        return start_vm(ctx, req).await;
    }
    match parse_end_path(&path) {
        EndRoute::Neither => {}
        end => return end_vm_or_golden(ctx, req, end).await,
    }
    match parse_stream_path(&path) {
        RouteMatch::Stream(target, kind) => match serve_stream(ctx, req, &target, kind).await {
            Ok(response) => response,
            Err(e) => error_response(e.status, &e.error),
        },
        // An id that is no plain id names no VM. The app has not looked at the disk.
        RouteMatch::BadVmId(id) => {
            let e = vm_not_found(&id, None);
            error_response(e.status, &e.error)
        }
        RouteMatch::BadEndpoint { value, reason } => {
            let e = invalid_endpoint(&value, &reason);
            error_response(e.status, &e.error)
        }
        RouteMatch::NotAStreamRoute => {
            // USAGE_ERROR is a code that exists.
            let err = AppError::new(
                "USAGE_ERROR",
                format!("The TestAnyware app has no route for {method} {path}."),
                None,
                json!({ "method": method.as_str(), "path": path }),
            );
            error_response(StatusCode::NOT_FOUND, &err)
        }
    }
}

/// Serve the route that stops a VM or deletes a golden. Both need `POST`.
async fn end_vm_or_golden(
    ctx: &Arc<AppContext>,
    req: &mut Request<Incoming>,
    route: EndRoute,
) -> Response<AppBody> {
    if req.method() != Method::POST {
        let err = AppError::new(
            "USAGE_ERROR",
            format!("{} needs POST, not {}.", req.uri().path(), req.method()),
            None,
            json!({ "method": req.method().as_str(), "path": req.uri().path() }),
        );
        return error_response(StatusCode::METHOD_NOT_ALLOWED, &err);
    }
    match route {
        EndRoute::Stop { id } => crate::stop_delete::stop_vm(ctx, id).await,
        EndRoute::BadVmId(id) => crate::stop_delete::not_a_vm(&id),
        EndRoute::Delete { name } => crate::stop_delete::delete_golden(ctx, req, name).await,
        EndRoute::BadGoldenName(name) => crate::stop_delete::not_a_golden(&name),
        EndRoute::Neither => unreachable!("the caller routes only the two end routes"),
    }
}

/// Serve `GET /v1/vms`: the view that `vm list` prints, for the options that the Host CLI parsed.
async fn list_vms(ctx: &Arc<AppContext>, query: Option<&str>) -> Response<AppBody> {
    let request = match ListRequest::parse(query) {
        Ok(request) => request,
        Err(reason) => {
            let err = AppError::new(
                "USAGE_ERROR",
                format!("The request for the VM listing is invalid: {reason}."),
                None,
                json!({ "method": "GET", "path": LIST_PATH, "query": query }),
            );
            return error_response(StatusCode::BAD_REQUEST, &err);
        }
    };
    // `tart list` is a child process that can take a while, and it blocks its thread.
    let blocking_ctx = Arc::clone(ctx);
    match tokio::task::spawn_blocking(move || crate::vms::list_view(&blocking_ctx, &request)).await
    {
        Ok(view) => json_response(
            StatusCode::OK,
            serde_json::to_value(view).expect("the view serialises"),
        ),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &AppError::new(
                "INTERNAL",
                format!("The VM listing failed inside the TestAnyware app: {e}."),
                None,
                Value::Null,
            ),
        ),
    }
}

/// The most bytes of a start request that the app reads. The options are far smaller.
const START_BODY_LIMIT: usize = 64 * 1024;

/// Serve `POST /v1/vms`: read the options that the Host CLI resolved, and answer with the
/// events of the start. A request that cannot start anything fails before the stream, as a
/// JSON error object.
async fn start_vm(ctx: &Arc<AppContext>, req: &mut Request<Incoming>) -> Response<AppBody> {
    let body = match Limited::new(req.body_mut(), START_BODY_LIMIT)
        .collect()
        .await
    {
        Ok(collected) => collected.to_bytes(),
        Err(e) => {
            let err = AppError::new(
                "USAGE_ERROR",
                format!("The TestAnyware app cannot read the request to start a VM: {e}."),
                None,
                json!({ "method": "POST", "path": LIST_PATH }),
            );
            return error_response(StatusCode::BAD_REQUEST, &err);
        }
    };
    match crate::start::parse_request(&body) {
        Ok((_, options)) => crate::start::start_response(ctx, options),
        Err(refusal) => error_response(refusal.status, &refusal.error),
    }
}

/// Serve one guest stream: check the request, find and dial the guest, and upgrade. Every
/// failure comes before the `101`, so the client reads an error object. After the `101`, the
/// bytes pass unchanged until either side closes.
async fn serve_stream(
    ctx: &AppContext,
    req: &mut Request<Incoming>,
    target: &StreamTarget,
    kind: StreamKind,
) -> Result<Response<AppBody>, ServeError> {
    if req.method() != Method::GET {
        return Err(usage_error(
            StatusCode::METHOD_NOT_ALLOWED,
            format!("A guest stream needs GET, not {}.", req.method()),
            req,
        ));
    }
    if !asks_for_stream_upgrade(req) {
        return Err(usage_error(
            StatusCode::BAD_REQUEST,
            format!(
                "A guest stream needs the headers Connection: Upgrade and Upgrade: {STREAM_UPGRADE_TOKEN}."
            ),
            req,
        ));
    }
    let endpoint = resolve_endpoint(&ctx.identity.directories, target, kind)?;
    let guest = open_guest_stream(&ctx.grant, &endpoint).await?;

    // The connection upgrades once the `101` is sent. The relay runs on its own task, with its
    // own guest connection, and drops both sides when it ends.
    // https://docs.rs/hyper/1.9.0/hyper/upgrade/fn.on.html
    let on_upgrade = hyper::upgrade::on(req);
    tokio::spawn(async move {
        let mut guest = guest;
        let upgraded = match on_upgrade.await {
            Ok(upgraded) => upgraded,
            Err(e) => return eprintln!("testanyware-app: upgrade failed: {e}"),
        };
        // The connection is the Unix stream that `serve` gave to hyper, so the downcast gives it
        // back. The relay needs the socket itself to tell a client that has gone from one that
        // half-closed.
        // https://docs.rs/hyper/1.9.0/hyper/upgrade/struct.Upgraded.html#method.downcast
        let Ok(parts) = upgraded.downcast::<TokioIo<UnixStream>>() else {
            return eprintln!("testanyware-app: the upgraded connection is not a Unix stream");
        };
        let mut client = parts.io.into_inner();
        // Bytes that hyper read past the request belong to the guest.
        if !parts.read_buf.is_empty() && guest.write_all(&parts.read_buf).await.is_err() {
            return;
        }
        let watch = match ClientWatch::new(&client) {
            Ok(watch) => watch,
            Err(e) => {
                eprintln!("testanyware-app: cannot watch a client of a stream: {e}");
                return;
            }
        };
        // A reset or a broken pipe is how a client leaves. It needs no report.
        let outcome = relay(&mut client, &mut guest, watch.gone()).await;
        close_guest(guest, &outcome);
    });
    let mut response = Response::new(AppBody::full(Bytes::new()));
    *response.status_mut() = StatusCode::SWITCHING_PROTOCOLS;
    let headers = response.headers_mut();
    headers.insert(CONNECTION, HeaderValue::from_static("upgrade"));
    headers.insert(UPGRADE, HeaderValue::from_static(STREAM_UPGRADE_TOKEN));
    Ok(response)
}

/// Does the request ask to upgrade to a guest stream?
fn asks_for_stream_upgrade(req: &Request<Incoming>) -> bool {
    let has_token = |name, token: &str| {
        req.headers()
            .get_all(name)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .flat_map(|v| v.split(','))
            .any(|t| t.trim().eq_ignore_ascii_case(token))
    };
    has_token(CONNECTION, "upgrade") && has_token(UPGRADE, STREAM_UPGRADE_TOKEN)
}

fn usage_error(status: StatusCode, message: String, req: &Request<Incoming>) -> ServeError {
    ServeError::new(
        status,
        AppError::new(
            "USAGE_ERROR",
            message,
            None,
            json!({ "method": req.method().as_str(), "path": req.uri().path() }),
        ),
    )
}

pub(crate) fn error_response(status: StatusCode, err: &AppError) -> Response<AppBody> {
    json_response(status, serde_json::to_value(err).expect("error serialises"))
}

pub(crate) fn json_response(status: StatusCode, body: Value) -> Response<AppBody> {
    let mut response = Response::new(AppBody::full(Bytes::from(body.to_string())));
    *response.status_mut() = status;
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    response
}

fn header_value(s: &str) -> HeaderValue {
    HeaderValue::from_str(s).unwrap_or_else(|_| HeaderValue::from_static("unknown"))
}
