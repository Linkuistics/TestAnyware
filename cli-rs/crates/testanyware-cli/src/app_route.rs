//! Routing of agent connections and of `vm list` through the TestAnyware app (spec: "The protocol").
//!
//! On a macOS host, and on any host with `TESTANYWARE_APP_SOCKET` set, every command that uses
//! the agent client gets each connection from the app as a guest stream, and the Host CLI dials
//! nothing itself. The
//! agent client runs its HTTP/1.1 on that stream as it runs it on a TCP connection, so the
//! output of a command does not change.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use testanyware_agent_client::{
    AgentClient, AgentConfig, AgentConnector, BoxedStream, ConnectFuture, ConnectorFailure,
};
use testanyware_app_client::{AppClient, AppStream, StartEnd};
use testanyware_app_protocol::version::SUPPORTED_RANGE;
use testanyware_app_protocol::{
    decide, AppError, ClientHello, Decision, DeleteResult, Directories, ListRequest, Peer,
    ServerHello, StartRequest, StatusResponse, StopResult, StreamKind, StreamTarget,
    APP_UNREACHABLE,
};
use testanyware_vm::listing::ListView;

use crate::resolve::{default_app_socket_with_env, home_dir_with_env, EnvProvider, ResolvedAgent};

/// The name of the app bundle that the cask installs (spec: "Packaging and identity").
const BUNDLE_NAME: &str = "TestAnyware.app";

/// The places where a person installs the app: the cask's `/Applications`, and the user's own
/// `Applications` directory.
pub fn bundle_search_paths(home: &Path) -> Vec<PathBuf> {
    vec![
        PathBuf::from("/Applications").join(BUNDLE_NAME),
        home.join("Applications").join(BUNDLE_NAME),
    ]
}

/// A bundle of the app found in one of the search paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledApp {
    pub path: PathBuf,
    /// The build in the bundle's `Info.plist`, or `None` when it does not read.
    pub version: Option<String>,
}

/// The first of `searched` that holds a bundle. `doctor` uses this to say what the commands
/// mean by an installed app (see [`name_missing_install`]).
pub fn find_installed(
    searched: &[PathBuf],
    exists: impl Fn(&Path) -> bool,
    build_of: impl Fn(&Path) -> Option<String>,
) -> Option<InstalledApp> {
    searched.iter().find(|path| exists(path)).map(|path| InstalledApp {
        path: path.clone(),
        version: build_of(path),
    })
}

/// The installed app on this host.
pub fn installed_app_here() -> (Vec<PathBuf>, Option<InstalledApp>) {
    let searched = bundle_search_paths(&home_dir_with_env(&EnvProvider::process()));
    let installed = find_installed(
        &searched,
        Path::exists,
        testanyware_app_protocol::bundle_build_at,
    );
    (searched, installed)
}

/// Read `GET /v1/status` of the app on `socket`. The request changes nothing in the app, and the
/// app answers it for any protocol range and any directories. The client states no notice of
/// a skew, because the `doctor` report states it.
pub async fn status(socket: PathBuf) -> Result<StatusResponse, AppError> {
    AppClient::new(socket, client_hello())
        .get_json("/v1/status")
        .await
}

/// Name the missing install. Nothing answers on `socket`, and no search path holds a bundle,
/// so the error is `APP_NOT_INSTALLED` instead of `APP_UNREACHABLE`. This holds for the
/// default socket only. A socket that a person named, such as that of a development bundle,
/// is an app that the Host CLI has no reason to look for in `/Applications`.
fn name_missing_install(
    error: AppError,
    socket: &Path,
    default_socket: &Path,
    searched: &[PathBuf],
    exists: impl Fn(&Path) -> bool,
) -> AppError {
    let not_running = error.code == APP_UNREACHABLE && error.details["cause"] == "not_running";
    if not_running && socket == default_socket && !searched.iter().any(|path| exists(path)) {
        return AppError::not_installed(socket, searched);
    }
    error
}

/// Open a guest stream through the app. A failure to reach the app on the default socket
/// says whether the app is installed.
pub(crate) async fn open_stream(
    client: &AppClient,
    target: &StreamTarget,
    kind: StreamKind,
) -> Result<AppStream, AppError> {
    client
        .open_stream(target, kind)
        .await
        .map_err(|error| name_missing_install_here(error, client.socket()))
}

/// [`name_missing_install`] for this process: its environment and the file system.
fn name_missing_install_here(error: AppError, socket: &Path) -> AppError {
    let env = EnvProvider::process();
    name_missing_install(
        error,
        socket,
        &default_app_socket_with_env(&env),
        &bundle_search_paths(&home_dir_with_env(&env)),
        Path::exists,
    )
}

/// Ask the app for the listing that `vm list` prints. A failure to reach the app on the default
/// socket says whether the app is installed, as for a guest stream.
pub async fn list_vms(socket: PathBuf, request: &ListRequest) -> Result<ListView, AppError> {
    let client = app_client(socket);
    client
        .get_json(&request.path_and_query())
        .await
        .map_err(|error| name_missing_install_here(error, client.socket()))
}

/// Ask the app to stop a VM. A failure to reach the app on the default socket says whether the
/// app is installed, as for a guest stream.
pub async fn stop_vm(socket: PathBuf, id: &str) -> Result<StopResult, AppError> {
    let client = app_client(socket);
    client
        .stop_vm(id)
        .await
        .map_err(|error| name_missing_install_here(error, client.socket()))
}

/// Ask the app to delete a golden image. A failure to reach the app on the default socket says
/// whether the app is installed, as for a guest stream.
pub async fn delete_golden(
    socket: PathBuf,
    name: &str,
    force: bool,
) -> Result<DeleteResult, AppError> {
    let client = app_client(socket);
    client
        .delete_golden(name, force)
        .await
        .map_err(|error| name_missing_install_here(error, client.socket()))
}

/// Ask the app to start a VM, and hand each progress line to `on_progress`. A failure to reach
/// the app on the default socket says whether the app is installed, as for a guest stream.
pub async fn start_vm(
    socket: PathBuf,
    request: &StartRequest,
    on_progress: impl FnMut(&str),
) -> Result<StartEnd, AppError> {
    let client = app_client(socket);
    client
        .start_vm(request, on_progress)
        .await
        .map_err(|error| name_missing_install_here(error, client.socket()))
}

/// What each request states about this Host CLI: the protocol range it speaks, its build, and
/// the directories it resolved. The app reads the VM records in the state directory, so the
/// directory that goes here is the one the Host CLI reads its own records from.
pub fn client_hello() -> ClientHello {
    let paths = crate::host_config::vm_paths();
    ClientHello::new(
        SUPPORTED_RANGE,
        env!("CARGO_PKG_VERSION"),
        Directories {
            state_dir: paths.state_dir().to_path_buf(),
            data_dir: paths.data_dir().to_path_buf(),
        },
    )
}

/// A client of the app on `socket` that states the hello of this Host CLI. Each response that it
/// reads goes through the version check: when the app's build differs from this one and the
/// ranges overlap, the Host CLI writes one notice line to stderr (spec: "Version skew").
pub fn app_client(socket: PathBuf) -> AppClient {
    AppClient::new(socket, client_hello()).with_response_observer(observe_response)
}

/// Whether this process has written its notice. One command opens many connections, and it
/// writes the notice once.
static NOTICE_WRITTEN: AtomicBool = AtomicBool::new(false);

fn observe_response(app: &ServerHello) {
    if let Some(line) = skew_notice(&this_cli(), app) {
        write_notice_once(&NOTICE_WRITTEN, &line, &mut std::io::stderr());
    }
}

/// This Host CLI as the version check sees it.
fn this_cli() -> Peer {
    Peer {
        build: Some(env!("CARGO_PKG_VERSION").to_string()),
        protocol_range: Some(SUPPORTED_RANGE),
    }
}

/// The notice line for what a response states, or `None` when the pair needs none. A pair
/// whose ranges do not overlap needs none here: the app has refused the request, and the error
/// of the refusal carries the remedy.
fn skew_notice(cli: &Peer, app: &ServerHello) -> Option<String> {
    let app_peer = Peer {
        build: app.build.clone(),
        protocol_range: app.protocol_range,
    };
    match decide(cli, &app_peer, app.bundle_build.as_deref()) {
        Decision::ServeWithNotice(notice) => Some(notice.line()),
        Decision::Serve | Decision::Fail(_) => None,
    }
}

/// Write `line` to `out` unless `written` says an earlier call did. The line goes to stderr
/// whatever the output mode is, and no option suppresses it.
fn write_notice_once(written: &AtomicBool, line: &str, out: &mut impl Write) {
    if !written.swap(true, Ordering::SeqCst) {
        // A closed stderr is no reason to fail the command.
        let _ = writeln!(out, "{line}");
    }
}

/// An agent client whose every connection is a guest stream of the app on `socket`.
pub fn agent_client(socket: PathBuf, target: StreamTarget) -> AgentClient {
    // The config only names the agent in the `Host` header and in messages. A VM is named by
    // its id, because the Host CLI does not read its spec on this route.
    let (host, port) = match &target {
        StreamTarget::Vm { id } => (id.clone(), ResolvedAgent::DEFAULT_PORT),
        StreamTarget::Endpoint { host, port } => (host.clone(), *port),
    };
    let connector = AppAgentConnector {
        client: app_client(socket),
        target,
    };
    AgentClient::with_connector(AgentConfig::new(host, port), Arc::new(connector))
}

/// Gets each agent connection from the app.
struct AppAgentConnector {
    client: AppClient,
    target: StreamTarget,
}

impl AgentConnector for AppAgentConnector {
    fn connect(&self) -> ConnectFuture {
        let client = self.client.clone();
        let target = self.target.clone();
        Box::pin(async move {
            open_stream(&client, &target, StreamKind::Agent)
                .await
                .map(|stream| Box::new(stream) as BoxedStream)
                .map_err(|error| failure_of(error).into())
        })
    }
}

/// The app's error object as the failure that the agent client hands on. The Host CLI prints
/// it in its own envelope, so the object keeps its code, message, remedy and details.
pub(crate) fn failure_of(error: AppError) -> ConnectorFailure {
    ConnectorFailure {
        code: error.code,
        message: error.message,
        remediation: error.remediation,
        details: error.details,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn an_error_object_keeps_every_field() {
        let failure = failure_of(AppError::new(
            "CONNECTION_REFUSED",
            "m",
            Some("r".into()),
            json!({ "errno": 61, "cause": "host_unreachable" }),
        ));
        assert_eq!(failure.code, "CONNECTION_REFUSED");
        assert_eq!(failure.message, "m");
        assert_eq!(failure.remediation.as_deref(), Some("r"));
        assert_eq!(failure.details["errno"], 61);
    }

    fn unreachable(cause: &str) -> AppError {
        AppError::new(
            "APP_UNREACHABLE",
            "m",
            Some("r".into()),
            json!({ "cause": cause, "socket": "/h/s.sock" }),
        )
    }

    fn search() -> Vec<PathBuf> {
        bundle_search_paths(Path::new("/Users/me"))
    }

    #[test]
    fn the_search_paths_are_the_cask_and_the_users_applications() {
        assert_eq!(
            search(),
            [
                PathBuf::from("/Applications/TestAnyware.app"),
                PathBuf::from("/Users/me/Applications/TestAnyware.app"),
            ]
        );
    }

    #[test]
    fn a_silent_default_socket_and_no_bundle_is_not_installed() {
        let socket = Path::new("/h/s.sock");
        let error = name_missing_install(
            unreachable("not_running"),
            socket,
            socket,
            &search(),
            |_| false,
        );
        assert_eq!(error.code, "APP_NOT_INSTALLED");
        assert_eq!(error.details["socket"], "/h/s.sock");
        assert_eq!(
            error.details["searched"][0],
            "/Applications/TestAnyware.app"
        );
        let remedy = error.remediation.expect("the setup");
        for step in [
            "brew install --cask linkuistics/taps/testanyware-app",
            "/Applications",
            "Start at Login",
            "Local Network",
        ] {
            assert!(remedy.contains(step), "{step} missing from {remedy}");
        }
        assert!(remedy.contains("release provides the cask"), "{remedy}");
    }

    #[test]
    fn a_bundle_in_either_place_keeps_the_error_unreachable() {
        let socket = Path::new("/h/s.sock");
        for present in search() {
            let error = name_missing_install(
                unreachable("not_running"),
                socket,
                socket,
                &search(),
                |path| path == present,
            );
            assert_eq!(error.code, "APP_UNREACHABLE", "{}", present.display());
        }
    }

    #[test]
    fn a_named_socket_is_never_judged_by_the_install_places() {
        let error = name_missing_install(
            unreachable("not_running"),
            Path::new("/tmp/dev/s.sock"),
            Path::new("/h/s.sock"),
            &search(),
            |_| false,
        );
        assert_eq!(error.code, "APP_UNREACHABLE");
    }

    #[test]
    fn only_a_silent_socket_can_mean_a_missing_install() {
        let socket = Path::new("/h/s.sock");
        for cause in [
            "unsafe_socket_directory",
            "quitting",
            "socket_path_too_long",
            "socket_error",
        ] {
            let error =
                name_missing_install(unreachable(cause), socket, socket, &search(), |_| false);
            assert_eq!(error.code, "APP_UNREACHABLE", "{cause}");
        }
        let other = AppError::new("VM_NOT_FOUND", "m", None, json!({}));
        let error = name_missing_install(other, socket, socket, &search(), |_| false);
        assert_eq!(error.code, "VM_NOT_FOUND");
    }

    #[test]
    fn the_hello_states_the_range_the_build_and_the_directories() {
        let hello = client_hello();
        assert_eq!(hello.protocol_range, Some(SUPPORTED_RANGE));
        assert_eq!(hello.build.as_deref(), Some(env!("CARGO_PKG_VERSION")));
        let dirs = hello.directories.expect("directories");
        assert!(dirs.state_dir.ends_with("testanyware"));
        assert!(dirs.data_dir.ends_with("testanyware"));
    }

    fn cli(build: &str) -> Peer {
        Peer {
            build: Some(build.into()),
            protocol_range: Some(SUPPORTED_RANGE),
        }
    }

    fn app(build: &str, bundle: Option<&str>) -> ServerHello {
        ServerHello {
            protocol_range: Some(SUPPORTED_RANGE),
            build: Some(build.into()),
            bundle_build: bundle.map(str::to_string),
        }
    }

    #[test]
    fn equal_builds_give_no_notice() {
        assert_eq!(skew_notice(&cli("1.0.0"), &app("1.0.0", Some("2.0.0"))), None);
    }

    #[test]
    fn builds_that_differ_give_a_line_with_both_builds_and_the_remedy() {
        let line = skew_notice(&cli("1.1.0"), &app("1.0.0", Some("1.1.0"))).unwrap();
        assert!(line.contains("1.1.0") && line.contains("1.0.0"), "{line}");
        assert!(line.contains("Quit TestAnyware from its menu"), "{line}");
        assert!(!line.contains('\n'));
        let line = skew_notice(&cli("1.0.0"), &app("1.1.0", None)).unwrap();
        assert!(line.contains("brew upgrade testanyware"), "{line}");
    }

    #[test]
    fn a_response_that_states_nothing_gives_no_notice() {
        assert_eq!(skew_notice(&cli("1.0.0"), &ServerHello::default()), None);
    }

    #[test]
    fn the_line_is_written_once_whatever_the_number_of_responses() {
        let written = AtomicBool::new(false);
        let mut out = Vec::new();
        for _ in 0..3 {
            write_notice_once(&written, "one notice", &mut out);
        }
        assert_eq!(String::from_utf8(out).unwrap(), "one notice\n");
    }
}
