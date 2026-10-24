//! Guest streams: find the endpoint, dial it, report a failed dial, and relay the bytes.
//!
//! The app opens a guest stream only to a TCP endpoint: the one a VM's spec in the state
//! directory names, or the one the request names. It opens no host file and no host socket for
//! a client (spec: "The socket and access").

use std::future::Future;
use std::net::SocketAddr;
use std::os::fd::{AsFd, OwnedFd};
use std::time::SystemTime;

use hyper::StatusCode;
use serde_json::{json, Map, Value};
use testanyware_app_protocol::{
    AppError, Directories, StreamKind, StreamTarget, LOCAL_NETWORK_REMEDY,
};
use testanyware_vm::paths::VmPaths;
use testanyware_vm::spec::VmSpec;
use tokio::io::unix::AsyncFd;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, Interest, Ready};
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

/// Copy bytes both ways until the stream ends. A half-close passes through: the end of one
/// side's bytes shuts down the write half of the other, and the other direction goes on.
/// The stream ends when both directions have ended, or when the client has gone. The client has
/// gone when `client_gone` resolves after the client's end, because a client that half-closed
/// still reads. A guest such as tart's VNC server never ends its direction after the client's
/// end, so without this the relay would keep both connections for ever.
///
/// The client's bytes reach the guest first. `client_gone` ends nothing before the client's end
/// has been copied, so a client that sends a last message and closes still delivers it.
/// The copy holds `RELAY_BUFFER_BYTES` for each direction and no more. The caller drops both
/// sides when this returns, which closes the guest connection.
pub async fn relay<C, G, F>(
    client: &mut C,
    guest: &mut G,
    client_gone: F,
) -> std::io::Result<Relayed>
where
    C: AsyncRead + AsyncWrite + Unpin + ?Sized,
    G: AsyncRead + AsyncWrite + Unpin + ?Sized,
    F: Future<Output = ()>,
{
    let (mut client_in, mut client_out) = tokio::io::split(client);
    let (mut guest_in, mut guest_out) = tokio::io::split(guest);
    let (mut up, mut down) = (0u64, 0u64);
    let mut gone = false;
    {
        let up_pump = pump(&mut client_in, &mut guest_out, &mut up);
        let down_pump = pump(&mut guest_in, &mut client_out, &mut down);
        tokio::pin!(up_pump, down_pump, client_gone);
        let (mut up_ended, mut down_ended) = (false, false);
        while !(up_ended && down_ended) {
            tokio::select! {
                result = &mut up_pump, if !up_ended => {
                    result?;
                    up_ended = true;
                }
                result = &mut down_pump, if !down_ended => {
                    result?;
                    down_ended = true;
                }
                () = &mut client_gone, if up_ended => {
                    gone = true;
                    break;
                }
            }
        }
    }
    Ok(Relayed {
        up,
        down,
        client_gone: gone,
    })
}

/// How a relay ended without an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Relayed {
    /// Bytes from the client to the guest.
    pub up: u64,
    /// Bytes from the guest to the client.
    pub down: u64,
    /// The relay ended because the client had gone, while the guest still kept its direction.
    pub client_gone: bool,
}

/// Close the guest connection of a relay that has ended. A stream that both sides ended has
/// nothing left, and the close is the ordinary one. A client that has gone, or an error, leaves
/// a guest that has sent no end of its own, and the close is a reset. tart's VNC server keeps
/// the connection of an ordinary close in `CLOSE_WAIT` for about two minutes, and 250 commands
/// in that time reach its limit of descriptors. A reset frees it at once.
/// https://docs.rs/tokio/1.52.1/tokio/net/struct.TcpStream.html#method.set_zero_linger
pub fn close_guest(guest: TcpStream, outcome: &std::io::Result<Relayed>) {
    let ended_by_both = matches!(
        outcome,
        Ok(Relayed {
            client_gone: false,
            ..
        })
    );
    if !ended_by_both {
        // A failure to set the option leaves the ordinary close, which is the fallback.
        let _ = guest.set_zero_linger();
    }
    drop(guest);
}

/// Copy `from` to `to` through one fixed buffer, count the bytes, and shut down the write half
/// of `to` at the end of `from`.
async fn pump<R, W>(from: &mut R, to: &mut W, copied: &mut u64) -> std::io::Result<()>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut buffer = vec![0u8; RELAY_BUFFER_BYTES];
    loop {
        let n = from.read(&mut buffer).await?;
        if n == 0 {
            return to.shutdown().await;
        }
        to.write_all(&buffer[..n]).await?;
        *copied += n as u64;
    }
}

/// A watch on a client's Unix socket that resolves when the client has closed it altogether.
///
/// On a Unix socket the client's half-close and its close both end a read, so a read cannot
/// tell them apart. The kernel can. It flags a socket whose peer has closed it as unable to
/// send, and a socket whose peer only shut down its write half as unable to receive. tokio
/// reports the first as `Ready::is_write_closed`, which mio takes from `EPOLLHUP` on Linux and
/// from `EV_EOF` of the write filter on macOS.
/// https://docs.rs/tokio/1.52.1/tokio/io/struct.Ready.html#method.is_write_closed
///
/// The watch holds a duplicate of the descriptor in its own registration, because the relay
/// reads and writes the socket through another one. Clearing the writable readiness of a shared
/// registration would stall the relay's writes.
pub struct ClientWatch {
    fd: AsyncFd<OwnedFd>,
}

impl ClientWatch {
    pub fn new(client: &impl AsFd) -> std::io::Result<Self> {
        let duplicate = client.as_fd().try_clone_to_owned()?;
        // Write interest alone: the watch never reads.
        Ok(Self {
            fd: AsyncFd::with_interest(duplicate, Interest::WRITABLE)?,
        })
    }

    /// Resolve when the client has closed the socket. When the registration fails, wait for ever:
    /// the relay then ends as it did before this watch, when both sides end.
    pub async fn gone(&self) {
        loop {
            let Ok(mut guard) = self.fd.ready(Interest::WRITABLE).await else {
                return std::future::pending().await;
            };
            if guard.ready().is_write_closed() {
                return;
            }
            // A socket that can take bytes is not news. Wait for the next change.
            guard.clear_ready_matching(Ready::WRITABLE);
        }
    }
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
        let task = tokio::spawn(async move {
            relay(&mut relay_client, &mut relay_guest, std::future::pending()).await
        });

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
        let relayed = task.await.unwrap().unwrap();
        assert_eq!((relayed.up, relayed.down), (7, 8));
        assert!(!relayed.client_gone);
    }

    #[tokio::test]
    async fn a_relay_moves_much_more_than_its_buffers_hold() {
        // The pipes hold 1 KiB and the relay buffers hold 64 KiB, so 8 MiB cannot sit in memory.
        let payload: Vec<u8> = (0..8 * 1024 * 1024u32).map(|i| (i % 251) as u8).collect();
        let (mut client_end, mut relay_client) = tokio::io::duplex(1024);
        let (mut relay_guest, mut guest_end) = tokio::io::duplex(1024);
        let task = tokio::spawn(async move {
            relay(&mut relay_client, &mut relay_guest, std::future::pending()).await
        });

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
        let task = tokio::spawn(async move {
            relay(&mut relay_client, &mut relay_guest, std::future::pending()).await
        });
        // Both ends drop: the relay sees the end on both sides and returns.
        drop(guest_end);
        drop(client_end);
        let outcome = tokio::time::timeout(std::time::Duration::from_secs(5), task)
            .await
            .expect("the relay must end");
        let _ = outcome.unwrap();
    }

    use std::time::Duration;

    /// A guest that never ends its direction after the client's end, as tart's VNC server does.
    /// The relay must not wait for it once the client has gone.
    #[tokio::test]
    async fn a_relay_ends_when_the_client_has_gone_and_the_guest_never_ends() {
        let (client_end, mut relay_client) = tokio::io::duplex(1024);
        let (mut relay_guest, mut guest_end) = tokio::io::duplex(1024);
        let (gone, has_gone) = tokio::sync::oneshot::channel::<()>();
        let task = tokio::spawn(async move {
            relay(&mut relay_client, &mut relay_guest, async {
                let _ = has_gone.await;
            })
            .await
        });
        // The client ends its half and then closes. The guest reads the end and stays silent.
        let mut client_end = client_end;
        client_end.write_all(b"last").await.unwrap();
        client_end.shutdown().await.unwrap();
        let mut seen = Vec::new();
        guest_end.read_to_end(&mut seen).await.unwrap();
        assert_eq!(seen, b"last");
        // Until the client has gone the relay waits, because the client may still read.
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(!task.is_finished(), "a half-close must keep the relay");
        gone.send(()).unwrap();
        let outcome = tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .expect("the relay must end when the client has gone");
        let relayed = outcome.unwrap().unwrap();
        assert_eq!((relayed.up, relayed.down), (4, 0));
        assert!(relayed.client_gone);
        drop(client_end);
        drop(guest_end);
    }

    #[tokio::test]
    async fn a_client_that_sends_a_last_message_and_closes_delivers_it() {
        // `client_gone` is ready from the start, and the message is still in the pipe.
        let (mut client_end, mut relay_client) = tokio::io::duplex(1024);
        let (mut relay_guest, mut guest_end) = tokio::io::duplex(1024);
        let task = tokio::spawn(async move {
            relay(&mut relay_client, &mut relay_guest, std::future::ready(())).await
        });
        client_end.write_all(b"key up").await.unwrap();
        drop(client_end);
        let mut seen = Vec::new();
        guest_end.read_to_end(&mut seen).await.unwrap();
        assert_eq!(
            seen, b"key up",
            "the last message of a closing client was lost"
        );
        let outcome = tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .expect("the relay must end");
        assert_eq!(outcome.unwrap().unwrap().up, 6);
    }

    #[tokio::test]
    async fn a_client_that_has_not_ended_its_half_keeps_the_relay_even_if_it_is_gone() {
        // `client_gone` counts only after the client's end, so a quiet client with a gone
        // signal does not end the relay: its bytes may still come.
        let (mut client_end, mut relay_client) = tokio::io::duplex(1024);
        let (mut relay_guest, mut guest_end) = tokio::io::duplex(1024);
        let task = tokio::spawn(async move {
            relay(&mut relay_client, &mut relay_guest, std::future::ready(())).await
        });
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(!task.is_finished());
        client_end.write_all(b"late").await.unwrap();
        let mut seen = [0u8; 4];
        guest_end.read_exact(&mut seen).await.unwrap();
        assert_eq!(&seen, b"late");
        drop(client_end);
        let _ = tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .expect("the relay must end");
    }

    #[tokio::test]
    async fn a_watch_tells_a_close_from_a_half_close_on_a_unix_socket() {
        use tokio::net::UnixStream;
        let (ours, mut theirs) = UnixStream::pair().unwrap();
        let watch = ClientWatch::new(&ours).unwrap();
        let gone = watch.gone();
        tokio::pin!(gone);

        // The peer is there and quiet.
        assert!(
            tokio::time::timeout(Duration::from_millis(200), &mut gone)
                .await
                .is_err(),
            "a quiet peer is not gone"
        );
        // The peer ends its half and still reads: our read gets the end, and the peer is here.
        theirs.shutdown().await.unwrap();
        let mut ours = ours;
        assert_eq!(ours.read(&mut [0u8; 1]).await.unwrap(), 0);
        assert!(
            tokio::time::timeout(Duration::from_millis(200), &mut gone)
                .await
                .is_err(),
            "a half-close is not a close"
        );
        // The peer still reads what we send.
        ours.write_all(b"answer").await.unwrap();
        let mut answer = [0u8; 6];
        theirs.read_exact(&mut answer).await.unwrap();
        assert_eq!(&answer, b"answer");
        // The peer closes.
        drop(theirs);
        tokio::time::timeout(Duration::from_secs(5), &mut gone)
            .await
            .expect("a closed peer is gone");
    }

    #[tokio::test]
    async fn a_watch_started_after_the_close_still_sees_it() {
        use tokio::net::UnixStream;
        let (ours, theirs) = UnixStream::pair().unwrap();
        drop(theirs);
        let watch = ClientWatch::new(&ours).unwrap();
        tokio::time::timeout(Duration::from_secs(5), watch.gone())
            .await
            .expect("a peer that closed before the watch is gone");
    }

    #[tokio::test]
    async fn a_relay_on_real_sockets_ends_with_a_client_that_closes_and_a_silent_guest() {
        use tokio::net::UnixStream;
        let (mut relay_client, mut client_end) = UnixStream::pair().unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mut relay_guest = TcpStream::connect(listener.local_addr().unwrap())
            .await
            .unwrap();
        let (mut guest_end, _) = listener.accept().await.unwrap();

        let watch = ClientWatch::new(&relay_client).unwrap();
        // The task owns both sides, so the guest connection closes when the relay returns.
        let task =
            tokio::spawn(
                async move { relay(&mut relay_client, &mut relay_guest, watch.gone()).await },
            );
        client_end.write_all(b"request").await.unwrap();
        drop(client_end);
        let mut seen = Vec::new();
        guest_end.read_to_end(&mut seen).await.unwrap();
        assert_eq!(seen, b"request");
        let outcome = tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .expect("the relay must end although the guest stays silent");
        assert_eq!(outcome.unwrap().unwrap().up, 7);
    }

    /// The two ends of a TCP connection: the app's side and the guest's.
    async fn tcp_pair() -> (TcpStream, TcpStream) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let app_side = TcpStream::connect(listener.local_addr().unwrap())
            .await
            .unwrap();
        let (guest_side, _) = listener.accept().await.unwrap();
        (app_side, guest_side)
    }

    #[tokio::test]
    async fn a_client_that_has_gone_resets_the_guest_connection() {
        // The relay passed the client's end on. The guest read it and went on holding the
        // connection, as tart's VNC server does.
        let (mut app_side, mut guest_side) = tcp_pair().await;
        app_side.shutdown().await.unwrap();
        assert_eq!(guest_side.read(&mut [0u8; 1]).await.unwrap(), 0);

        close_guest(
            app_side,
            &Ok(Relayed {
                up: 0,
                down: 0,
                client_gone: true,
            }),
        );
        // The guest learns of the close at once, as a reset. An ordinary close leaves no sign.
        let err = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                // The first write may succeed, because the reset is not back yet.
                if let Err(e) = guest_side.write_all(&[0]).await {
                    return e;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("the guest must see the close");
        assert!(matches!(
            err.kind(),
            std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::BrokenPipe
        ));
    }

    #[tokio::test]
    async fn a_stream_that_both_sides_ended_closes_in_the_ordinary_way() {
        let (mut app_side, mut guest_side) = tcp_pair().await;
        app_side.shutdown().await.unwrap();
        guest_side.shutdown().await.unwrap();

        close_guest(
            app_side,
            &Ok(Relayed {
                up: 0,
                down: 0,
                client_gone: false,
            }),
        );
        // An ordinary close ends with the end of file and no reset.
        assert_eq!(guest_side.read(&mut [0u8; 1]).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn a_relay_that_failed_resets_the_guest_connection() {
        let (app_side, mut guest_side) = tcp_pair().await;
        close_guest(app_side, &Err(std::io::ErrorKind::BrokenPipe.into()));
        let mut buffer = [0u8; 1];
        let read = tokio::time::timeout(Duration::from_secs(5), guest_side.read(&mut buffer))
            .await
            .expect("the guest must see the close");
        assert!(
            matches!(&read, Err(e) if e.kind() == std::io::ErrorKind::ConnectionReset),
            "{read:?}"
        );
    }
}
