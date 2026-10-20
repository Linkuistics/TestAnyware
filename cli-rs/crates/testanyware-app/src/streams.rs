//! Guest streams: find the endpoint, dial it, report a failed dial, and relay the bytes.
//!
//! The app opens a guest stream only to a TCP endpoint: the one a VM's spec in the state
//! directory names, or the one the request names. It opens no host file and no host socket for
//! a client (spec: "The socket and access").

use std::net::SocketAddr;
use std::time::SystemTime;

use hyper::StatusCode;
use serde_json::{json, Map, Value};
use testanyware_app_protocol::{
    AppError, Directories, StreamKind, StreamTarget, LOCAL_NETWORK_REMEDY,
};
use testanyware_vm::paths::VmPaths;
use testanyware_vm::spec::VmSpec;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;

use crate::dial::{
    connect_once, dial_with_diagnosis, platform_nc_reaches, Cause, DialError, ATTEMPT_TIMEOUT,
};
use crate::grant::{host_networks, observe, DialOutcome, GrantObservation};

/// Each direction of a relay holds at most this many bytes.
pub const RELAY_BUFFER_BYTES: usize = 64 * 1024;

/// An error object and the HTTP status that carries it.
#[derive(Debug)]
pub struct ServeError {
    pub status: StatusCode,
    pub error: AppError,
}

impl ServeError {
    pub fn new(status: StatusCode, error: AppError) -> Self {
        Self { status, error }
    }
}

/// The TCP endpoint of a guest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuestEndpoint {
    pub host: String,
    pub port: u16,
}

/// The endpoint for `target`. An endpoint that the request names is used as it stands. A VM
/// id reads the VM's spec in the state directory that the request's directories name.
pub fn resolve_endpoint(
    directories: &Directories,
    target: &StreamTarget,
    kind: StreamKind,
) -> Result<GuestEndpoint, ServeError> {
    match target {
        StreamTarget::Endpoint { host, port } => Ok(GuestEndpoint {
            host: host.clone(),
            port: *port,
        }),
        StreamTarget::Vm { id } => {
            // The route grammar admits plain ids only, so the id cannot leave `vms/`.
            let paths = VmPaths::new(
                directories.state_dir.clone(),
                directories.data_dir.clone(),
                std::env::temp_dir(),
            );
            let spec_path = paths.spec_path(id);
            if !spec_path.is_file() {
                return Err(vm_not_found(id, Some(&spec_path)));
            }
            let spec = VmSpec::load(&spec_path).map_err(|e| {
                ServeError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    AppError::new(
                        "IO_ERROR",
                        e.to_string(),
                        None,
                        json!({ "spec_path": spec_path.display().to_string() }),
                    ),
                )
            })?;
            let missing = |section: &str| {
                ServeError::new(
                    StatusCode::CONFLICT,
                    AppError::new(
                        "NO_CONNECTION_SPECIFIED",
                        format!(
                            "Connection spec at {} has no `{section}` section; this command requires it",
                            spec_path.display()
                        ),
                        None,
                        json!({ "spec_path": spec_path.display().to_string() }),
                    ),
                )
            };
            match kind {
                StreamKind::Agent => spec
                    .agent
                    .map(|a| GuestEndpoint {
                        host: a.host,
                        port: a.port,
                    })
                    .ok_or_else(|| missing("agent")),
                StreamKind::Vnc => Ok(GuestEndpoint {
                    host: spec.vnc.host,
                    port: spec.vnc.port,
                }),
            }
        }
    }
}

/// `VM_NOT_FOUND`, in the words of the Host CLI. `spec_path` is `None` for an id that the app
/// refused without looking at the disk.
pub fn vm_not_found(id: &str, spec_path: Option<&std::path::Path>) -> ServeError {
    let (message, details) = match spec_path {
        Some(path) => (
            format!("No spec found for VM id '{id}' at {}", path.display()),
            json!({ "vm_id": id, "spec_path": path.display().to_string() }),
        ),
        None => (
            format!("No spec found for VM id '{id}'"),
            json!({ "vm_id": id }),
        ),
    };
    ServeError::new(
        StatusCode::NOT_FOUND,
        AppError::new("VM_NOT_FOUND", message, None, details),
    )
}

/// `INVALID_ENDPOINT`, in the words of the Host CLI.
pub fn invalid_endpoint(value: &str, reason: &str) -> ServeError {
    ServeError::new(
        StatusCode::BAD_REQUEST,
        AppError::new(
            "INVALID_ENDPOINT",
            format!("Invalid agent endpoint '{value}': {reason}"),
            None,
            json!({ "value": value, "reason": reason }),
        ),
    )
}

/// The error object for a failed dial.
pub fn dial_error(endpoint: &GuestEndpoint, err: &DialError) -> ServeError {
    let mut details = Map::new();
    details.insert("host".into(), json!(endpoint.host));
    details.insert("port".into(), json!(endpoint.port));
    match err {
        DialError::TimedOut { failure } => {
            details.insert("cause".into(), json!("timeout"));
            ServeError::new(
                StatusCode::GATEWAY_TIMEOUT,
                AppError::new(
                    "CONNECTION_TIMEOUT",
                    failure.message.clone(),
                    None,
                    Value::Object(details),
                ),
            )
        }
        DialError::Refused { failure, cause } => {
            details.insert("cause".into(), json!(cause.as_str()));
            if let Some(errno) = failure.errno {
                details.insert("errno".into(), json!(errno));
            }
            let (message, remediation) = match cause {
                Cause::LocalNetworkDenied => (
                    format!(
                        "macOS refused the TestAnyware app the route to {}:{}. The platform \
                         tool /usr/bin/nc connects to it.",
                        endpoint.host, endpoint.port
                    ),
                    Some(LOCAL_NETWORK_REMEDY.to_string()),
                ),
                Cause::HostUnreachable => (
                    failure.message.clone(),
                    Some(format!(
                        "The host has no route to {}:{}, and the platform tool /usr/bin/nc \
                         has none either. Check that the guest runs and has an address.",
                        endpoint.host, endpoint.port
                    )),
                ),
                Cause::ConnectionRefused | Cause::ConnectFailed => (failure.message.clone(), None),
            };
            ServeError::new(
                StatusCode::BAD_GATEWAY,
                AppError::new(
                    "CONNECTION_REFUSED",
                    message,
                    remediation,
                    Value::Object(details),
                ),
            )
        }
    }
}

/// Dial the guest, with the Local Network diagnosis, and record what the dial showed about the
/// grant.
pub async fn open_guest_stream(
    grant: &std::sync::Mutex<GrantObservation>,
    endpoint: &GuestEndpoint,
) -> Result<TcpStream, ServeError> {
    let port = endpoint.port;
    let result = dial_with_diagnosis(
        || connect_once(&endpoint.host, port, ATTEMPT_TIMEOUT),
        |ip| platform_nc_reaches(ip, port),
    )
    .await;
    let seen = match &result {
        Ok(dialed) => Some((dialed.ip, DialOutcome::Connected)),
        Err(DialError::Refused {
            failure,
            cause: Cause::LocalNetworkDenied,
        }) => failure.ip.map(|ip| (ip, DialOutcome::DeniedProved)),
        Err(_) => None,
    };
    if let Some((ip, outcome)) = seen {
        record(grant, SocketAddr::new(ip, port), outcome);
    }
    match result {
        Ok(dialed) => Ok(dialed.stream),
        Err(err) => Err(dial_error(endpoint, &err)),
    }
}

fn record(
    grant: &std::sync::Mutex<GrantObservation>,
    destination: SocketAddr,
    outcome: DialOutcome,
) {
    let networks = host_networks();
    let mut current = grant.lock().unwrap_or_else(|p| p.into_inner());
    *current = observe(&current, destination, outcome, &networks, SystemTime::now());
}

/// Copy bytes both ways until both directions end. A half-close passes through: the end of one
/// side's bytes shuts down the write half of the other, and the other direction goes on. The
/// copy holds `RELAY_BUFFER_BYTES` for each direction and no more. The caller drops both sides
/// when this returns, which closes the guest connection.
pub async fn relay<C, G>(client: &mut C, guest: &mut G) -> std::io::Result<(u64, u64)>
where
    C: AsyncRead + AsyncWrite + Unpin + ?Sized,
    G: AsyncRead + AsyncWrite + Unpin + ?Sized,
{
    // Each direction has a fixed buffer, and the end of one side shuts down the write half of the
    // other. https://docs.rs/tokio/1.52.1/tokio/io/fn.copy_bidirectional_with_sizes.html
    tokio::io::copy_bidirectional_with_sizes(client, guest, RELAY_BUFFER_BYTES, RELAY_BUFFER_BYTES)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use testanyware_vm::spec::{AgentEndpoint, VncEndpoint};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    fn dirs(root: &std::path::Path) -> Directories {
        Directories {
            state_dir: root.join("state"),
            data_dir: root.join("data"),
        }
    }

    fn write_spec(directories: &Directories, id: &str, agent: Option<AgentEndpoint>) {
        let spec = VmSpec {
            vnc: VncEndpoint {
                host: "192.168.64.7".into(),
                port: 5901,
                password: None,
            },
            agent,
            platform: "macos".into(),
            logical: None,
        };
        let vms = directories.state_dir.join("vms");
        std::fs::create_dir_all(&vms).unwrap();
        spec.write_atomic(&vms.join(format!("{id}.json"))).unwrap();
    }

    #[test]
    fn an_endpoint_that_the_request_names_is_used_as_it_stands() {
        let tmp = tempfile::tempdir().unwrap();
        let target = StreamTarget::Endpoint {
            host: "10.1.2.3".into(),
            port: 80,
        };
        let endpoint = resolve_endpoint(&dirs(tmp.path()), &target, StreamKind::Vnc).unwrap();
        assert_eq!(
            endpoint,
            GuestEndpoint {
                host: "10.1.2.3".into(),
                port: 80
            }
        );
    }

    #[test]
    fn a_vm_resolves_to_the_endpoint_of_its_spec() {
        let tmp = tempfile::tempdir().unwrap();
        let directories = dirs(tmp.path());
        write_spec(
            &directories,
            "testanyware-aabbccdd",
            Some(AgentEndpoint {
                host: "192.168.64.7".into(),
                port: 8648,
            }),
        );
        let target = StreamTarget::Vm {
            id: "testanyware-aabbccdd".into(),
        };
        assert_eq!(
            resolve_endpoint(&directories, &target, StreamKind::Agent).unwrap(),
            GuestEndpoint {
                host: "192.168.64.7".into(),
                port: 8648
            }
        );
        assert_eq!(
            resolve_endpoint(&directories, &target, StreamKind::Vnc).unwrap(),
            GuestEndpoint {
                host: "192.168.64.7".into(),
                port: 5901
            }
        );
    }

    #[test]
    fn a_missing_spec_is_vm_not_found_with_the_path() {
        let tmp = tempfile::tempdir().unwrap();
        let target = StreamTarget::Vm {
            id: "testanyware-00000000".into(),
        };
        let err = resolve_endpoint(&dirs(tmp.path()), &target, StreamKind::Agent).unwrap_err();
        assert_eq!(err.status, StatusCode::NOT_FOUND);
        assert_eq!(err.error.code, "VM_NOT_FOUND");
        assert!(err.error.details["spec_path"]
            .as_str()
            .unwrap()
            .ends_with("state/vms/testanyware-00000000.json"));
    }

    #[test]
    fn a_spec_without_an_agent_section_cannot_serve_an_agent_stream() {
        let tmp = tempfile::tempdir().unwrap();
        let directories = dirs(tmp.path());
        write_spec(&directories, "vm-1", None);
        let target = StreamTarget::Vm { id: "vm-1".into() };
        let err = resolve_endpoint(&directories, &target, StreamKind::Agent).unwrap_err();
        assert_eq!(err.error.code, "NO_CONNECTION_SPECIFIED");
        assert!(resolve_endpoint(&directories, &target, StreamKind::Vnc).is_ok());
    }

    #[test]
    fn a_spec_that_does_not_parse_is_an_io_error() {
        let tmp = tempfile::tempdir().unwrap();
        let directories = dirs(tmp.path());
        let vms = directories.state_dir.join("vms");
        std::fs::create_dir_all(&vms).unwrap();
        std::fs::write(vms.join("vm-1.json"), "{ not json").unwrap();
        let target = StreamTarget::Vm { id: "vm-1".into() };
        let err = resolve_endpoint(&directories, &target, StreamKind::Agent).unwrap_err();
        assert_eq!(err.status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(err.error.code, "IO_ERROR");
    }

    fn failure(kind: crate::dial::ProbeKind, errno: Option<i32>) -> crate::dial::DialFailure {
        crate::dial::DialFailure {
            kind,
            errno,
            message: "failed to connect to h:1: nope".into(),
            ip: None,
        }
    }

    fn endpoint() -> GuestEndpoint {
        GuestEndpoint {
            host: "192.168.64.7".into(),
            port: 8648,
        }
    }

    #[test]
    fn a_refused_dial_is_connection_refused_with_its_errno() {
        let err = dial_error(
            &endpoint(),
            &DialError::Refused {
                failure: failure(crate::dial::ProbeKind::Refused, Some(61)),
                cause: Cause::ConnectionRefused,
            },
        );
        assert_eq!(err.status, StatusCode::BAD_GATEWAY);
        assert_eq!(err.error.code, "CONNECTION_REFUSED");
        assert_eq!(err.error.details["errno"], 61);
        assert_eq!(err.error.details["cause"], "connection_refused");
        assert_eq!(err.error.remediation, None);
    }

    #[test]
    fn a_dial_that_timed_out_is_connection_timeout() {
        let err = dial_error(
            &endpoint(),
            &DialError::TimedOut {
                failure: failure(crate::dial::ProbeKind::Timeout, None),
            },
        );
        assert_eq!(err.status, StatusCode::GATEWAY_TIMEOUT);
        assert_eq!(err.error.code, "CONNECTION_TIMEOUT");
        assert_eq!(err.error.details["cause"], "timeout");
        assert!(
            err.error.details.get("errno").is_none(),
            "no errno, no field"
        );
    }

    #[test]
    fn a_denial_names_the_app_and_the_pane_of_system_settings() {
        let err = dial_error(
            &endpoint(),
            &DialError::Refused {
                failure: failure(crate::dial::ProbeKind::Unreachable, Some(65)),
                cause: Cause::LocalNetworkDenied,
            },
        );
        assert_eq!(err.error.code, "CONNECTION_REFUSED");
        assert_eq!(err.error.details["cause"], "local_network_denied");
        assert_eq!(err.error.details["errno"], 65);
        let remedy = err.error.remediation.unwrap();
        for needle in [
            "TestAnyware",
            "System Settings",
            "Privacy & Security",
            "Local Network",
        ] {
            assert!(remedy.contains(needle), "{needle} missing from {remedy}");
        }
    }

    #[test]
    fn an_unreachable_host_is_not_called_a_denial() {
        let err = dial_error(
            &endpoint(),
            &DialError::Refused {
                failure: failure(crate::dial::ProbeKind::Unreachable, Some(65)),
                cause: Cause::HostUnreachable,
            },
        );
        assert_eq!(err.error.details["cause"], "host_unreachable");
        assert!(!err.error.remediation.unwrap().contains("System Settings"));
    }

    #[tokio::test]
    async fn a_relay_passes_a_half_close_through_in_both_directions() {
        // client_end <-> relay <-> guest_end, with in-memory pipes.
        let (mut client_end, mut relay_client) = tokio::io::duplex(1024);
        let (mut relay_guest, mut guest_end) = tokio::io::duplex(1024);
        let task = tokio::spawn(async move { relay(&mut relay_client, &mut relay_guest).await });

        // The client sends its request and shuts down its write half.
        client_end.write_all(b"request").await.unwrap();
        client_end.shutdown().await.unwrap();
        // The guest sees the bytes, then the end of the stream, and still answers.
        let mut seen = Vec::new();
        guest_end.read_to_end(&mut seen).await.unwrap();
        assert_eq!(seen, b"request");
        guest_end.write_all(b"response").await.unwrap();
        guest_end.shutdown().await.unwrap();
        // The client still reads the answer after its own half-close.
        let mut answer = Vec::new();
        client_end.read_to_end(&mut answer).await.unwrap();
        assert_eq!(answer, b"response");
        assert_eq!(task.await.unwrap().unwrap(), (7, 8));
    }

    #[tokio::test]
    async fn a_relay_moves_much_more_than_its_buffers_hold() {
        // The pipes hold 1 KiB and the relay buffers hold 64 KiB, so 8 MiB cannot sit in memory.
        let payload: Vec<u8> = (0..8 * 1024 * 1024u32).map(|i| (i % 251) as u8).collect();
        let (mut client_end, mut relay_client) = tokio::io::duplex(1024);
        let (mut relay_guest, mut guest_end) = tokio::io::duplex(1024);
        let task = tokio::spawn(async move { relay(&mut relay_client, &mut relay_guest).await });

        let sent = payload.clone();
        let writer = tokio::spawn(async move {
            client_end.write_all(&sent).await.unwrap();
            client_end.shutdown().await.unwrap();
            client_end
        });
        let mut received = Vec::new();
        guest_end.read_to_end(&mut received).await.unwrap();
        guest_end.shutdown().await.unwrap();
        assert!(received == payload, "the bytes changed in the relay");
        let _client_end = writer.await.unwrap();
        task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn a_relay_ends_when_the_guest_resets() {
        let (client_end, mut relay_client) = tokio::io::duplex(1024);
        let (mut relay_guest, guest_end) = tokio::io::duplex(1024);
        let task = tokio::spawn(async move { relay(&mut relay_client, &mut relay_guest).await });
        // Both ends drop: the relay sees the end on both sides and returns.
        drop(guest_end);
        drop(client_end);
        let outcome = tokio::time::timeout(std::time::Duration::from_secs(5), task)
            .await
            .expect("the relay must end");
        let _ = outcome.unwrap();
    }
}
