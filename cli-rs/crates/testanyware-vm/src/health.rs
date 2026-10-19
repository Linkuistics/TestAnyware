//! Poll the in-VM agent's `/health` endpoint until it is a Healthy agent.
//!
//! Port of `AgentHealthWaiter.swift`, extended: the wait returns an
//! [`AgentWait`] outcome rather than a `bool`, so a failed `vm start` can name
//! *why* the agent never came up — the last probe's error kind and OS errno —
//! and can stop early when the VM itself is gone.

use std::fmt;
use std::future::Future;
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
    /// The probes kept failing with "host unreachable" while a platform tool
    /// connected to the same endpoint. This process is denied the route
    /// (macOS Local Network privacy), so more probes cannot help.
    LocalNetworkDenied {
        last_error: ProbeError,
        attempts: u32,
        elapsed: Duration,
    },
}

/// The longest the wait sleeps without asking whether the VM is still alive.
const LIVENESS_SLICE: Duration = Duration::from_millis(250);

/// Sleep for `interval`, cutting it short once the VM is gone, so a death
/// between probes ends the wait within a slice rather than a whole interval
/// later (QEMU's is 5 s). An interval of one slice or less is a single sleep:
/// the loop head asks again before the next probe anyway.
async fn sleep_while_alive(interval: Duration, vm_alive: &mut impl FnMut() -> bool) {
    let deadline = Instant::now() + interval;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return;
        }
        tokio::time::sleep(left.min(LIVENESS_SLICE)).await;
        if Instant::now() < deadline && !vm_alive() {
            return;
        }
    }
}

/// Consecutive "host unreachable" probes after which the wait asks whether
/// another process can reach the endpoint. A booting guest that has not
/// answered ARP yet gives the same errno, so a few misses prove nothing.
const DENIAL_STREAK: u32 = 5;

/// Poll `http://<host>:<port>/health` up to `attempts` times, `interval`
/// apart, until the agent is healthy.
///
/// `vm_alive` is asked before every probe and during the sleep between probes;
/// once it reports the VM gone the wait ends at once with [`AgentWait::VmExited`] rather than spending the
/// rest of the budget on a corpse. It is injected so the wait is testable
/// without a hypervisor.
///
/// For an endpoint on loopback (QEMU's port forward), where no privacy layer
/// sits between this process and the guest. A guest on a host-only subnet uses
/// [`wait_for_agent_with_oracle`].
pub async fn wait_for_agent(
    host: &str,
    port: u16,
    attempts: u32,
    interval: Duration,
    vm_alive: impl FnMut() -> bool,
) -> AgentWait {
    wait_for_agent_with_oracle(host, port, attempts, interval, vm_alive, || async { false }).await
}

/// [`wait_for_agent`] for a guest on a host-only subnet, where macOS Local
/// Network privacy can deny this process the route while the agent is healthy.
///
/// After [`DENIAL_STREAK`] unreachable probes in a row, and again after every
/// further streak of that length, the wait calls `reach_oracle`. It answers
/// whether a process that the privacy layer exempts (a platform tool such as
/// `/usr/bin/nc`) reaches the endpoint. A `true` ends the wait with
/// [`AgentWait::LocalNetworkDenied`]. A `false` changes nothing.
pub async fn wait_for_agent_with_oracle<Reach>(
    host: &str,
    port: u16,
    attempts: u32,
    interval: Duration,
    vm_alive: impl FnMut() -> bool,
    reach_oracle: impl FnMut() -> Reach,
) -> AgentWait
where
    Reach: Future<Output = bool>,
{
    // A short per-request timeout keeps each poll snappy; the loop, not
    // the socket, owns the overall budget.
    let config = AgentConfig::new(host, port).with_timeout(Duration::from_secs(2));
    let client = match AgentClient::new(config) {
        Ok(client) => client,
        Err(err) => {
            return AgentWait::NotHealthy {
                last_error: Some(ProbeError::from_agent_error(&err)),
                attempts: 0,
                elapsed: Duration::ZERO,
            };
        }
    };
    let client = &client;
    wait_with_probe(
        attempts,
        interval,
        vm_alive,
        move || async move {
            client
                .health()
                .await
                .map(|_| ())
                .map_err(|err| ProbeError::from_agent_error(&err))
        },
        reach_oracle,
    )
    .await
}

/// The wait loop over an injected probe, so its rules are testable with
/// scripted results instead of a network.
async fn wait_with_probe<Probe, Reach>(
    attempts: u32,
    interval: Duration,
    mut vm_alive: impl FnMut() -> bool,
    mut probe: impl FnMut() -> Probe,
    mut reach_oracle: impl FnMut() -> Reach,
) -> AgentWait
where
    Probe: Future<Output = Result<(), ProbeError>>,
    Reach: Future<Output = bool>,
{
    let started = Instant::now();
    let mut last_error = None;
    let mut made = 0;
    let mut unreachable_streak = 0;
    for attempt in 0..attempts {
        if !vm_alive() {
            return AgentWait::VmExited {
                last_error,
                attempts: made,
                elapsed: started.elapsed(),
            };
        }
        made += 1;
        match probe().await {
            Ok(()) => return AgentWait::Healthy { attempts: made },
            Err(err) => {
                if err.kind == ProbeKind::Unreachable {
                    unreachable_streak += 1;
                } else {
                    unreachable_streak = 0;
                }
                let ask = unreachable_streak > 0 && unreachable_streak % DENIAL_STREAK == 0;
                if ask && reach_oracle().await {
                    return AgentWait::LocalNetworkDenied {
                        last_error: err,
                        attempts: made,
                        elapsed: started.elapsed(),
                    };
                }
                last_error = Some(err);
            }
        }
        if attempt + 1 < attempts {
            sleep_while_alive(interval, &mut vm_alive).await;
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
    async fn a_vm_that_dies_between_probes_ends_the_wait_within_the_interval() {
        // Alive for the first probe, dead from the next question on; the
        // interval (20 s) is what a dead VM must not be waited out for.
        let mut asked = 0;
        let started = Instant::now();
        let wait = wait_for_agent(
            "127.0.0.1",
            dead_port(),
            100,
            Duration::from_secs(20),
            || {
                asked += 1;
                asked == 1
            },
        )
        .await;
        let AgentWait::VmExited { attempts, .. } = wait else {
            panic!("expected VmExited, got {wait:?}");
        };
        assert_eq!(attempts, 1);
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "took {:?}: the death was noticed only after the interval",
            started.elapsed()
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

    fn unreachable_probe() -> Result<(), ProbeError> {
        Err(ProbeError {
            kind: ProbeKind::Unreachable,
            os_errno: Some(65),
            detail: "tcp connect error".into(),
        })
    }

    fn refused_probe() -> Result<(), ProbeError> {
        Err(ProbeError {
            kind: ProbeKind::Refused,
            os_errno: Some(61),
            detail: "tcp connect error".into(),
        })
    }

    /// Run the wait over scripted probe results (the last repeats forever),
    /// counting how often the reach oracle was asked.
    async fn scripted_wait(
        script: Vec<Result<(), ProbeError>>,
        attempts: u32,
        oracle_says: bool,
        asked: &std::cell::Cell<u32>,
    ) -> AgentWait {
        let mut next = 0;
        wait_with_probe(
            attempts,
            Duration::from_millis(1),
            always_alive,
            move || {
                let result = script[next.min(script.len() - 1)].clone();
                next += 1;
                async move { result }
            },
            || {
                asked.set(asked.get() + 1);
                async move { oracle_says }
            },
        )
        .await
    }

    #[tokio::test]
    async fn five_unreachable_probes_in_a_row_ask_the_oracle_and_a_yes_is_a_denial() {
        let asked = std::cell::Cell::new(0);
        let wait = scripted_wait(vec![unreachable_probe()], 60, true, &asked).await;
        let AgentWait::LocalNetworkDenied {
            last_error,
            attempts,
            ..
        } = wait
        else {
            panic!("expected LocalNetworkDenied, got {wait:?}");
        };
        assert_eq!(attempts, 5, "the fifth refusal triggers the question");
        assert_eq!(last_error.kind, ProbeKind::Unreachable);
        assert_eq!(asked.get(), 1);
    }

    #[tokio::test]
    async fn a_no_from_the_oracle_keeps_waiting_and_asks_again_after_another_five() {
        let asked = std::cell::Cell::new(0);
        let wait = scripted_wait(vec![unreachable_probe()], 12, false, &asked).await;
        let AgentWait::NotHealthy { attempts, .. } = wait else {
            panic!("expected NotHealthy, got {wait:?}");
        };
        assert_eq!(attempts, 12, "an unproven denial spends the whole budget");
        assert_eq!(asked.get(), 2, "asked at the 5th and the 10th refusal");
    }

    #[tokio::test]
    async fn a_probe_that_is_not_unreachable_resets_the_streak() {
        // Four unreachable, one refused, four unreachable: never five in a row.
        let mut script = vec![unreachable_probe(); 4];
        script.push(refused_probe());
        script.extend(vec![unreachable_probe(); 4]);
        script.push(refused_probe());
        let asked = std::cell::Cell::new(0);
        let wait = scripted_wait(script, 10, true, &asked).await;
        assert!(matches!(wait, AgentWait::NotHealthy { .. }), "got {wait:?}");
        assert_eq!(asked.get(), 0, "the oracle is never asked");
    }

    #[tokio::test]
    async fn refused_probes_never_ask_the_oracle() {
        let asked = std::cell::Cell::new(0);
        let wait = scripted_wait(vec![refused_probe()], 20, true, &asked).await;
        assert!(matches!(wait, AgentWait::NotHealthy { .. }), "got {wait:?}");
        assert_eq!(asked.get(), 0);
    }

    #[tokio::test]
    async fn a_healthy_probe_after_a_streak_wins_over_the_oracle() {
        let mut script = vec![unreachable_probe(); 3];
        script.push(Ok(()));
        let asked = std::cell::Cell::new(0);
        let wait = scripted_wait(script, 60, true, &asked).await;
        assert_eq!(wait, AgentWait::Healthy { attempts: 4 });
        assert_eq!(asked.get(), 0);
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
