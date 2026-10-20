//! The dial of a guest stream, and the Local Network diagnosis of a failed dial (ADR-0019).
//!
//! The rules are generic over the connect and the oracle, so a test drives each one with
//! scripted results and paused time instead of a network.

use std::future::Future;
use std::net::IpAddr;
use std::time::Duration;

use tokio::time::Instant;

pub use testanyware_vm::health::ProbeKind;

/// The app retries a `host_unreachable` dial for this long, counted from the first try. A new
/// executable UUID gave a refusal of about 2 s in the measurements, and the retry finds the
/// grant that a person has just given.
pub const RETRY_WINDOW: Duration = Duration::from_secs(5);

/// The pause between two tries. A try that fails with `host_unreachable` takes about 2 s, so
/// this pause only matters for a dial that fails at once.
pub const RETRY_PAUSE: Duration = Duration::from_millis(250);

/// One try may take this long, from the name lookup to the connected socket.
pub const ATTEMPT_TIMEOUT: Duration = Duration::from_secs(10);

/// Why one try failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DialFailure {
    pub kind: ProbeKind,
    /// The raw OS errno, when the failure was a connect error.
    pub errno: Option<i32>,
    /// The error's text, with the OS cause.
    pub message: String,
    /// The address that the try used, when the name resolved.
    pub ip: Option<IpAddr>,
}

/// A connected socket and the address that it reached.
#[derive(Debug)]
pub struct Dialed<S> {
    pub stream: S,
    pub ip: IpAddr,
}

/// What the diagnosis concluded about a failed dial.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cause {
    /// The host answered and nothing listens.
    ConnectionRefused,
    /// Another connect error. The errno says which.
    ConnectFailed,
    /// No route for the whole retry window, and the platform `nc` has no route either.
    HostUnreachable,
    /// No route for the whole retry window, and the platform `nc` connects.
    LocalNetworkDenied,
}

impl Cause {
    /// The stable token of `details.cause`.
    pub fn as_str(self) -> &'static str {
        match self {
            Cause::ConnectionRefused => "connection_refused",
            Cause::ConnectFailed => "connect_failed",
            Cause::HostUnreachable => "host_unreachable",
            Cause::LocalNetworkDenied => "local_network_denied",
        }
    }
}

/// How a dial failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialError {
    /// `CONNECTION_REFUSED`. The failure is the last try.
    Refused { failure: DialFailure, cause: Cause },
    /// `CONNECTION_TIMEOUT`.
    TimedOut { failure: DialFailure },
}

/// Dial until it connects or fails for good.
///
/// - A refusal, a timeout or another connect error ends the dial at once.
/// - A `host_unreachable` try repeats until `RETRY_WINDOW` has passed since the first try.
/// - After the window, the oracle decides between `local_network_denied` and
///   `host_unreachable`. It runs once, for the address of the last try.
pub async fn dial_with_diagnosis<S, Connect, ConnectFut, Oracle, OracleFut>(
    mut connect: Connect,
    oracle: Oracle,
) -> Result<Dialed<S>, DialError>
where
    Connect: FnMut() -> ConnectFut,
    ConnectFut: Future<Output = Result<Dialed<S>, DialFailure>>,
    Oracle: FnOnce(Option<IpAddr>) -> OracleFut,
    OracleFut: Future<Output = bool>,
{
    let started = Instant::now();
    loop {
        let failure = match connect().await {
            Ok(dialed) => return Ok(dialed),
            Err(failure) => failure,
        };
        match failure.kind {
            ProbeKind::Unreachable => {}
            ProbeKind::Timeout => return Err(DialError::TimedOut { failure }),
            ProbeKind::Refused => {
                return Err(DialError::Refused {
                    failure,
                    cause: Cause::ConnectionRefused,
                })
            }
            _ => {
                return Err(DialError::Refused {
                    failure,
                    cause: Cause::ConnectFailed,
                })
            }
        }
        let remaining = RETRY_WINDOW.saturating_sub(started.elapsed());
        if !remaining.is_zero() {
            tokio::time::sleep(RETRY_PAUSE.min(remaining)).await;
            continue;
        }
        let cause = if oracle(failure.ip).await {
            Cause::LocalNetworkDenied
        } else {
            Cause::HostUnreachable
        };
        return Err(DialError::Refused { failure, cause });
    }
}

/// One real try: resolve the name, then connect to each address in turn. The whole try is
/// bounded by `ATTEMPT_TIMEOUT`.
pub async fn connect_once(
    host: &str,
    port: u16,
    attempt_timeout: Duration,
) -> Result<Dialed<tokio::net::TcpStream>, DialFailure> {
    match tokio::time::timeout(attempt_timeout, connect_any(host, port)).await {
        Ok(outcome) => outcome,
        Err(_) => Err(DialFailure {
            kind: ProbeKind::Timeout,
            errno: None,
            message: format!(
                "connecting to {host}:{port} did not finish within {attempt_timeout:?}"
            ),
            ip: None,
        }),
    }
}

async fn connect_any(host: &str, port: u16) -> Result<Dialed<tokio::net::TcpStream>, DialFailure> {
    let addrs = tokio::net::lookup_host((host, port))
        .await
        .map_err(|e| DialFailure {
            kind: ProbeKind::Connect,
            errno: e.raw_os_error(),
            message: format!("failed to resolve {host}: {e}"),
            ip: None,
        })?;
    let mut last = None;
    for addr in addrs {
        match tokio::net::TcpStream::connect(addr).await {
            Ok(stream) => {
                // Best effort, as the agent client does: small requests should not wait for Nagle.
                let _ = stream.set_nodelay(true);
                return Ok(Dialed {
                    stream,
                    ip: addr.ip(),
                });
            }
            Err(e) => {
                last = Some(DialFailure {
                    kind: testanyware_vm::health::kind_from_errno(e.raw_os_error()),
                    errno: e.raw_os_error(),
                    message: format!("failed to connect to {host}:{port}: {e}"),
                    ip: Some(addr.ip()),
                })
            }
        }
    }
    Err(last.unwrap_or_else(|| DialFailure {
        kind: ProbeKind::Connect,
        errno: None,
        message: format!("{host} resolved to no address"),
        ip: None,
    }))
}

/// Does the platform `nc` reach `ip:port`? Only macOS has the exempt tool. Any other host
/// answers `false`, so a denial is never claimed without proof.
pub async fn platform_nc_reaches(ip: Option<IpAddr>, port: u16) -> bool {
    let Some(ip) = ip else { return false };
    #[cfg(target_os = "macos")]
    {
        testanyware_vm::local_network::platform_tool_reaches(&ip.to_string(), port).await
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (ip, port);
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};
    use std::net::Ipv4Addr;

    const GUEST: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 168, 64, 5));

    fn failure(kind: ProbeKind, errno: i32) -> DialFailure {
        DialFailure {
            kind,
            errno: Some(errno),
            message: format!("os error {errno}"),
            ip: Some(GUEST),
        }
    }

    fn unreachable() -> DialFailure {
        failure(ProbeKind::Unreachable, 65)
    }

    /// A connect that answers from a script, then repeats its last answer. It counts its calls.
    fn scripted(
        script: Vec<Result<(), DialFailure>>,
        calls: &Cell<u32>,
    ) -> impl FnMut() -> std::future::Ready<Result<Dialed<()>, DialFailure>> + '_ {
        let script = RefCell::new(script);
        move || {
            calls.set(calls.get() + 1);
            let mut script = script.borrow_mut();
            let next = if script.len() > 1 {
                script.remove(0)
            } else {
                script[0].clone()
            };
            std::future::ready(next.map(|()| Dialed {
                stream: (),
                ip: GUEST,
            }))
        }
    }

    async fn never_asked(_: Option<IpAddr>) -> bool {
        panic!("the oracle must not run");
    }

    #[tokio::test(start_paused = true)]
    async fn a_connected_dial_asks_nothing_and_waits_for_nothing() {
        let calls = Cell::new(0);
        let started = Instant::now();
        let dialed = dial_with_diagnosis(scripted(vec![Ok(())], &calls), never_asked)
            .await
            .expect("connects");
        assert_eq!(dialed.ip, GUEST);
        assert_eq!(calls.get(), 1);
        assert_eq!(started.elapsed(), Duration::ZERO);
    }

    #[tokio::test(start_paused = true)]
    async fn a_refusal_ends_the_dial_at_once_with_its_errno() {
        let calls = Cell::new(0);
        let refused = failure(ProbeKind::Refused, 61);
        let err = dial_with_diagnosis(scripted(vec![Err(refused.clone())], &calls), never_asked)
            .await
            .unwrap_err();
        assert_eq!(
            err,
            DialError::Refused {
                failure: refused,
                cause: Cause::ConnectionRefused
            }
        );
        assert_eq!(calls.get(), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn a_timeout_ends_the_dial_at_once() {
        let calls = Cell::new(0);
        let timeout = DialFailure {
            kind: ProbeKind::Timeout,
            errno: None,
            message: "slow".into(),
            ip: None,
        };
        let err = dial_with_diagnosis(scripted(vec![Err(timeout.clone())], &calls), never_asked)
            .await
            .unwrap_err();
        assert_eq!(err, DialError::TimedOut { failure: timeout });
        assert_eq!(calls.get(), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn another_connect_error_ends_the_dial_with_its_own_cause() {
        let calls = Cell::new(0);
        let other = failure(ProbeKind::Connect, 13);
        let err = dial_with_diagnosis(scripted(vec![Err(other.clone())], &calls), never_asked)
            .await
            .unwrap_err();
        assert_eq!(
            err,
            DialError::Refused {
                failure: other,
                cause: Cause::ConnectFailed
            }
        );
        assert_eq!(calls.get(), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn an_unreachable_dial_that_connects_inside_the_window_is_not_a_failure() {
        let calls = Cell::new(0);
        let started = Instant::now();
        let script = vec![Err(unreachable()), Err(unreachable()), Ok(())];
        dial_with_diagnosis(scripted(script, &calls), never_asked)
            .await
            .expect("connects on the third try");
        assert_eq!(calls.get(), 3);
        assert_eq!(started.elapsed(), RETRY_PAUSE * 2);
    }

    #[tokio::test(start_paused = true)]
    async fn an_unreachable_dial_asks_the_oracle_once_after_the_window() {
        let calls = Cell::new(0);
        let asked = Cell::new(0);
        let seen = Cell::new(None);
        let started = Instant::now();
        let err = dial_with_diagnosis(scripted(vec![Err(unreachable())], &calls), |ip| {
            asked.set(asked.get() + 1);
            seen.set(ip);
            async { true }
        })
        .await
        .unwrap_err();
        assert_eq!(
            err,
            DialError::Refused {
                failure: unreachable(),
                cause: Cause::LocalNetworkDenied
            }
        );
        assert_eq!(asked.get(), 1, "the oracle runs once");
        assert_eq!(
            seen.get(),
            Some(GUEST),
            "it is asked about the address of the last try"
        );
        assert_eq!(
            started.elapsed(),
            RETRY_WINDOW,
            "the window is 5 s, counted from the first try"
        );
        assert_eq!(
            calls.get(),
            21,
            "one try at 0 s, then one after each 250 ms pause up to 5 s"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn an_oracle_that_cannot_connect_leaves_host_unreachable() {
        let calls = Cell::new(0);
        let err = dial_with_diagnosis(scripted(vec![Err(unreachable())], &calls), |_| async {
            false
        })
        .await
        .unwrap_err();
        assert_eq!(
            err,
            DialError::Refused {
                failure: unreachable(),
                cause: Cause::HostUnreachable
            }
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_slow_try_counts_against_the_window() {
        // Each try takes 2 s, as the measurements show. Three tries pass 5 s.
        let calls = Cell::new(0);
        let err = dial_with_diagnosis(
            || {
                calls.set(calls.get() + 1);
                async {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    Err::<Dialed<()>, _>(unreachable())
                }
            },
            |_| async { true },
        )
        .await
        .unwrap_err();
        assert!(matches!(
            err,
            DialError::Refused {
                cause: Cause::LocalNetworkDenied,
                ..
            }
        ));
        assert_eq!(
            calls.get(),
            3,
            "tries at 0 s, 2.25 s and 4.5 s; the last ends at 6.5 s"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_refusal_after_unreachable_tries_ends_the_dial_without_the_oracle() {
        let calls = Cell::new(0);
        let refused = failure(ProbeKind::Refused, 61);
        let script = vec![Err(unreachable()), Err(refused.clone())];
        let err = dial_with_diagnosis(scripted(script, &calls), never_asked)
            .await
            .unwrap_err();
        assert_eq!(
            err,
            DialError::Refused {
                failure: refused,
                cause: Cause::ConnectionRefused
            }
        );
        assert_eq!(calls.get(), 2);
    }

    #[tokio::test]
    async fn a_real_dial_to_a_closed_loopback_port_is_refused_with_errno() {
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let failure = connect_once("127.0.0.1", port, ATTEMPT_TIMEOUT)
            .await
            .unwrap_err();
        assert_eq!(failure.kind, ProbeKind::Refused);
        assert_eq!(failure.errno, Some(nix::libc::ECONNREFUSED));
        assert_eq!(failure.ip, Some(IpAddr::V4(Ipv4Addr::LOCALHOST)));
    }

    #[tokio::test]
    async fn a_real_dial_to_a_listener_connects() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let dialed = connect_once("127.0.0.1", port, ATTEMPT_TIMEOUT)
            .await
            .expect("connects");
        assert_eq!(dialed.ip, IpAddr::V4(Ipv4Addr::LOCALHOST));
    }

    #[tokio::test]
    async fn a_name_that_does_not_resolve_is_a_connect_failure_with_no_address() {
        let failure = connect_once("no-such-host.invalid", 80, ATTEMPT_TIMEOUT)
            .await
            .unwrap_err();
        assert_eq!(failure.kind, ProbeKind::Connect);
        assert_eq!(failure.ip, None);
    }
}
