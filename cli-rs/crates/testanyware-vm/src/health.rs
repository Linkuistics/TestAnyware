//! Poll the in-VM agent's `/health` endpoint until it is reachable.
//!
//! Port of `AgentHealthWaiter.swift`. `VMLifecycle` uses this to decide
//! whether to populate the `agent` field on the spec file.

use std::time::Duration;

use testanyware_agent_client::{AgentClient, AgentConfig};

/// Poll `http://<host>:<port>/health` up to `attempts` times, `interval`
/// apart. Returns `true` on the first healthy response. Connection
/// failures and errors are treated uniformly as "not ready yet".
pub async fn wait_for_agent(host: &str, port: u16, attempts: u32, interval: Duration) -> bool {
    // A short per-request timeout keeps each poll snappy; the loop, not
    // the socket, owns the overall budget.
    let config = AgentConfig::new(host, port).with_timeout(Duration::from_secs(2));
    let Ok(client) = AgentClient::new(config) else {
        return false;
    };
    for attempt in 0..attempts {
        if client.health().await.is_ok() {
            return true;
        }
        if attempt + 1 < attempts {
            tokio::time::sleep(interval).await;
        }
    }
    false
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

    #[tokio::test]
    async fn returns_true_when_health_responds_ok() {
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
        let ready = wait_for_agent(&host, port, 3, Duration::from_millis(50)).await;
        assert!(ready, "a 200 /health must resolve the waiter to true");
    }

    #[tokio::test]
    async fn returns_false_when_budget_is_exhausted() {
        // Nothing listening on this port — every attempt fails to connect.
        let ready = wait_for_agent("127.0.0.1", 1, 2, Duration::from_millis(20)).await;
        assert!(!ready, "no agent => waiter exhausts its budget and returns false");
    }
}
