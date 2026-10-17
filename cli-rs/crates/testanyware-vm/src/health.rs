//! Poll the in-VM agent's `/health` endpoint until it is a Healthy agent.
//!
//! Port of `AgentHealthWaiter.swift`, extended: the wait returns an
//! [`AgentWait`] outcome rather than a `bool`, so a failed `vm start` can name
//! *why* the agent never came up — the last probe's error kind and OS errno —
//! and can stop early when the VM itself is gone.

use std::fmt;
use std::time::{Duration, Instant};

use testanyware_agent_client::{AgentClient, AgentConfig, AgentError};

/// What a failed `/health` probe was, coarse enough to tell the hypotheses
/// about a never-healthy agent apart (a refused connect means the guest is up
/// and nothing listens; "no route to host" is the macOS Local Network privacy
/// signature; a timeout means packets vanish).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeKind {
    /// TCP connect refused: the host answered, nothing is listening.
    Refused,
    /// No route / host unreachable / network unreachable.
    Unreachable,
    /// The request or connect timed out.
    Timeout,
    /// Some other connect-phase failure; the errno says which.
    Connect,
    /// The connection dropped mid-request.
    Dropped,
    /// The agent answered with a non-2xx status.
    HttpStatus(u16),
    /// The agent answered 2xx with a body that is not a health response.
    Decode,
    /// Anything else (client construction, local I/O).
    Other,
}

impl ProbeKind {
    /// Stable token for `--json` details.
    pub fn as_str(self) -> &'static str {
        match self {
            ProbeKind::Refused => "connection_refused",
            ProbeKind::Unreachable => "host_unreachable",
            ProbeKind::Timeout => "timeout",
            ProbeKind::Connect => "connect_failed",
            ProbeKind::Dropped => "connection_dropped",
            ProbeKind::HttpStatus(_) => "http_status",
            ProbeKind::Decode => "decode",
            ProbeKind::Other => "other",
        }
    }
}

impl fmt::Display for ProbeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProbeKind::Refused => f.write_str("connection refused"),
            ProbeKind::Unreachable => f.write_str("host unreachable"),
            ProbeKind::Timeout => f.write_str("timed out"),
            ProbeKind::Connect => f.write_str("connect failed"),
            ProbeKind::Dropped => f.write_str("connection dropped"),
            ProbeKind::HttpStatus(status) => write!(f, "HTTP {status}"),
            ProbeKind::Decode => f.write_str("undecodable response"),
            ProbeKind::Other => f.write_str("error"),
        }
    }
}

/// The last failed probe of an agent wait.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeError {
    pub kind: ProbeKind,
    /// The raw OS errno, when the failure was a connect-phase I/O error.
    pub os_errno: Option<i32>,
    /// The error's full text, `source()` chain included.
    pub detail: String,
}

impl ProbeError {
    /// Classify an agent-client error. The errno is mapped to a kind through
    /// `io::Error::from_raw_os_error(..).kind()`, which is the same on every
    /// host OS, rather than through per-OS errno constants.
    pub fn from_agent_error(err: &AgentError) -> Self {
        // `detail` is the variant's own text without `AgentError`'s Display
        // prefix, which would only repeat the kind printed beside it.
        let (kind, os_errno, detail) = match err {
            AgentError::ConnectionRefused { message, os_errno } => {
                (kind_from_errno(*os_errno), *os_errno, message.clone())
            }
            AgentError::Timeout(m) => (ProbeKind::Timeout, None, m.clone()),
            AgentError::ConnectionDropped(m) => (ProbeKind::Dropped, None, m.clone()),
            AgentError::HttpStatus { status, body } => (
                ProbeKind::HttpStatus(status.as_u16()),
                None,
                truncate(body.trim(), 200),
            ),
            AgentError::Decode(m) => (ProbeKind::Decode, None, m.clone()),
            other => (ProbeKind::Other, None, other.to_string()),
        };
        ProbeError {
            kind,
            os_errno,
            detail,
        }
    }
}

/// `text` cut to at most `max` characters, marked when cut.
fn truncate(text: &str, max: usize) -> String {
    match text.char_indices().nth(max) {
        Some((cut, _)) => format!("{}…", &text[..cut]),
        None => text.to_string(),
    }
}

fn kind_from_errno(os_errno: Option<i32>) -> ProbeKind {
    use std::io::ErrorKind;
    let Some(code) = os_errno else {
        return ProbeKind::Connect;
    };
    match std::io::Error::from_raw_os_error(code).kind() {
        ErrorKind::ConnectionRefused => ProbeKind::Refused,
        ErrorKind::HostUnreachable | ErrorKind::NetworkUnreachable => ProbeKind::Unreachable,
        ErrorKind::TimedOut => ProbeKind::Timeout,
        _ => ProbeKind::Connect,
    }
}

impl fmt::Display for ProbeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.kind)?;
        if let Some(code) = self.os_errno {
            write!(f, " (os error {code})")?;
        }
        write!(f, ": {}", self.detail)
    }
}

/// How an agent wait ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentWait {
    /// `/health` answered 2xx with a decodable body.
    Healthy { attempts: u32 },
    /// The probe budget ran out with the VM still alive.
    NotHealthy {
        last_error: Option<ProbeError>,
        attempts: u32,
        elapsed: Duration,
    },
    /// The VM's process exited during the wait; no point probing further.
    VmExited {
        last_error: Option<ProbeError>,
        attempts: u32,
        elapsed: Duration,
    },
}

/// Poll `http://<host>:<port>/health` up to `attempts` times, `interval`
/// apart, until the agent is healthy.
///
/// `vm_alive` is asked before every probe; once it reports the VM gone the
/// wait ends at once with [`AgentWait::VmExited`] rather than spending the
/// rest of the budget on a corpse. It is injected so the wait is testable
/// without a hypervisor.
pub async fn wait_for_agent(
    host: &str,
    port: u16,
    attempts: u32,
    interval: Duration,
    mut vm_alive: impl FnMut() -> bool,
) -> AgentWait {
    // A short per-request timeout keeps each poll snappy; the loop, not
    // the socket, owns the overall budget.
    let config = AgentConfig::new(host, port).with_timeout(Duration::from_secs(2));
    let started = Instant::now();
    let client = match AgentClient::new(config) {
        Ok(client) => client,
        Err(err) => {
            return AgentWait::NotHealthy {
                last_error: Some(ProbeError::from_agent_error(&err)),
                attempts: 0,
                elapsed: started.elapsed(),
            };
        }
    };
    let mut last_error = None;
    let mut made = 0;
    for attempt in 0..attempts {
        if !vm_alive() {
            return AgentWait::VmExited {
                last_error,
                attempts: made,
                elapsed: started.elapsed(),
            };
        }
        made += 1;
        match client.health().await {
            Ok(_) => return AgentWait::Healthy { attempts: made },
            Err(err) => last_error = Some(ProbeError::from_agent_error(&err)),
        }
        if attempt + 1 < attempts {
            tokio::time::sleep(interval).await;
        }
    }
    AgentWait::NotHealthy {
        last_error,
        attempts: made,
        elapsed: started.elapsed(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn host_port(server: &MockServer) -> (String, u16) {
        let uri = server.uri(); // http://127.0.0.1:PORT
        let rest = uri.strip_prefix("http://").unwrap();
        let (h, p) = rest.rsplit_once(':').unwrap();
        (h.to_string(), p.parse().unwrap())
    }

    fn always_alive() -> bool {
        true
    }

    /// A loopback port with nothing listening on it.
    fn dead_port() -> u16 {
        std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port()
    }

    #[tokio::test]
    async fn healthy_when_health_responds_ok() {
        let server = MockServer::start().await;
        // The agent's /health returns a JSON HealthResponse body.
        Mock::given(method("GET"))
            .and(path("/health"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "accessible": true,
                "platform": "linux"
            })))
            .mount(&server)
            .await;
        let (host, port) = host_port(&server);
        let wait = wait_for_agent(&host, port, 3, Duration::from_millis(50), always_alive).await;
        assert_eq!(wait, AgentWait::Healthy { attempts: 1 });
    }

    #[tokio::test]
    async fn not_healthy_when_budget_is_exhausted_reports_the_refusal_and_its_errno() {
        // Nothing listening: every attempt is refused at connect.
        let wait = wait_for_agent(
            "127.0.0.1",
            dead_port(),
            3,
            Duration::from_millis(20),
            always_alive,
        )
        .await;
        let AgentWait::NotHealthy {
            last_error: Some(err),
            attempts,
            ..
        } = wait
        else {
            panic!("expected NotHealthy with a last error, got {wait:?}");
        };
        assert_eq!(
            attempts, 3,
            "every attempt of the budget is spent and counted"
        );
        assert_eq!(err.kind, ProbeKind::Refused);
        let errno = err.os_errno.expect("a refused connect carries its errno");
        assert_eq!(
            std::io::Error::from_raw_os_error(errno).kind(),
            std::io::ErrorKind::ConnectionRefused,
        );
    }

    #[tokio::test]
    async fn a_non_2xx_health_is_an_http_status_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/health"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;
        let (host, port) = host_port(&server);
        let wait = wait_for_agent(&host, port, 2, Duration::from_millis(5), always_alive).await;
        let AgentWait::NotHealthy {
            last_error: Some(err),
            ..
        } = wait
        else {
            panic!("expected NotHealthy, got {wait:?}");
        };
        assert_eq!(err.kind, ProbeKind::HttpStatus(503));
        assert_eq!(err.os_errno, None);
    }

    #[tokio::test]
    async fn a_2xx_health_with_a_non_health_body_is_a_decode_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/health"))
            .respond_with(ResponseTemplate::new(200).set_body_string("<html>not an agent</html>"))
            .mount(&server)
            .await;
        let (host, port) = host_port(&server);
        let wait = wait_for_agent(&host, port, 2, Duration::from_millis(5), always_alive).await;
        let AgentWait::NotHealthy {
            last_error: Some(err),
            ..
        } = wait
        else {
            panic!("expected NotHealthy, got {wait:?}");
        };
        assert_eq!(err.kind, ProbeKind::Decode);
    }

    #[tokio::test]
    async fn a_vm_that_exits_mid_wait_ends_the_wait_at_once_with_the_last_error() {
        // A budget that would take ~20 s if spent; the VM dies after two probes.
        let mut probes_allowed = 2;
        let started = Instant::now();
        let wait = wait_for_agent(
            "127.0.0.1",
            dead_port(),
            1000,
            Duration::from_millis(20),
            || {
                let alive = probes_allowed > 0;
                probes_allowed -= 1;
                alive
            },
        )
        .await;
        let AgentWait::VmExited {
            last_error,
            attempts,
            ..
        } = wait
        else {
            panic!("expected VmExited, got {wait:?}");
        };
        assert_eq!(
            attempts, 2,
            "liveness is asked before each probe, so two probes ran"
        );
        assert_eq!(last_error.map(|e| e.kind), Some(ProbeKind::Refused));
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "must not spend the budget"
        );
    }

    #[tokio::test]
    async fn a_vm_dead_before_the_first_probe_makes_no_probe() {
        let wait = wait_for_agent(
            "127.0.0.1",
            dead_port(),
            10,
            Duration::from_millis(5),
            || false,
        )
        .await;
        let AgentWait::VmExited {
            last_error,
            attempts,
            ..
        } = wait
        else {
            panic!("expected VmExited, got {wait:?}");
        };
        assert_eq!(attempts, 0);
        assert_eq!(last_error, None);
    }

    #[test]
    fn errno_classification_separates_refused_from_no_route() {
        // macOS Local Network privacy refuses with EHOSTUNREACH ("no route to
        // host"); a guest with no listener refuses with ECONNREFUSED. The two
        // are the cheapest discriminator between finding 1's hypotheses.
        #[cfg(unix)]
        {
            let classify = |code: i32| {
                ProbeError::from_agent_error(&AgentError::ConnectionRefused {
                    message: "x".into(),
                    os_errno: Some(code),
                })
                .kind
            };
            assert_eq!(classify(nix::libc::ECONNREFUSED), ProbeKind::Refused);
            assert_eq!(classify(nix::libc::EHOSTUNREACH), ProbeKind::Unreachable);
            assert_eq!(classify(nix::libc::ENETUNREACH), ProbeKind::Unreachable);
            assert_eq!(classify(nix::libc::ETIMEDOUT), ProbeKind::Timeout);
            assert_eq!(classify(nix::libc::EACCES), ProbeKind::Connect);
        }
    }

    #[test]
    fn a_connect_failure_without_an_errno_is_a_generic_connect_failure() {
        let err = ProbeError::from_agent_error(&AgentError::ConnectionRefused {
            message: "x".into(),
            os_errno: None,
        });
        assert_eq!(err.kind, ProbeKind::Connect);
    }

    #[test]
    fn other_agent_errors_map_to_their_kinds() {
        let kind = |e: AgentError| ProbeError::from_agent_error(&e).kind;
        assert_eq!(kind(AgentError::Timeout("t".into())), ProbeKind::Timeout);
        assert_eq!(
            kind(AgentError::ConnectionDropped("d".into())),
            ProbeKind::Dropped
        );
        assert_eq!(kind(AgentError::Internal("i".into())), ProbeKind::Other);
    }

    #[test]
    fn probe_error_display_names_kind_errno_and_detail() {
        let err = ProbeError {
            kind: ProbeKind::Unreachable,
            os_errno: Some(65),
            detail: "tcp connect error".into(),
        };
        assert_eq!(
            err.to_string(),
            "host unreachable (os error 65): tcp connect error"
        );
    }
}
