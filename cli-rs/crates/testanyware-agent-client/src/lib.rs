//! HTTP client for the TestAnyware in-VM agent (port 8648).
//!
//! This crate is a stub. Endpoint coverage lands in the per-feature
//! port tasks (see `LLM_STATE/core/` backlog). The connection-config
//! type is defined here so the CLI can construct it before we have
//! actual request methods.

use std::time::Duration;

use thiserror::Error;

#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub host: String,
    pub port: u16,
    /// Per-request timeout. The CLI exposes `--timeout` for long-running
    /// `exec` calls; default keeps short-poll calls responsive.
    pub timeout: Duration,
}

impl AgentConfig {
    pub const DEFAULT_PORT: u16 = 8648;

    pub fn new(host: impl Into<String>, port: u16) -> Self {
        Self {
            host: host.into(),
            port,
            timeout: Duration::from_secs(30),
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn base_url(&self) -> String {
        format!("http://{}:{}", self.host, self.port)
    }
}

#[derive(Debug, Error)]
pub enum AgentError {
    #[error("transport error: {0}")]
    Transport(#[from] reqwest::Error),

    #[error("decode error: {0}")]
    Decode(#[from] serde_json::Error),

    #[error("agent returned {status}: {body}")]
    Status {
        status: reqwest::StatusCode,
        body: String,
    },
}

/// Async HTTP client for the in-VM agent.
///
/// Endpoint methods are added by downstream tasks (`exec`,
/// `upload`/`download`, `agent windows`, etc.). The client itself is set
/// up here so the CLI scaffolding can compile and `--help` works against
/// the real type rather than a placeholder.
pub struct AgentClient {
    config: AgentConfig,
    http: reqwest::Client,
}

impl AgentClient {
    pub fn new(config: AgentConfig) -> reqwest::Result<Self> {
        let http = reqwest::Client::builder().timeout(config.timeout).build()?;
        Ok(Self { config, http })
    }

    pub fn config(&self) -> &AgentConfig {
        &self.config
    }

    pub fn http(&self) -> &reqwest::Client {
        &self.http
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_defaults() {
        let cfg = AgentConfig::new("192.168.64.2", AgentConfig::DEFAULT_PORT);
        assert_eq!(cfg.base_url(), "http://192.168.64.2:8648");
        assert_eq!(cfg.timeout, Duration::from_secs(30));
    }

    #[test]
    fn client_builds() {
        let cfg = AgentConfig::new("localhost", 8648);
        let client = AgentClient::new(cfg).expect("reqwest client should build");
        assert_eq!(client.config().host, "localhost");
    }
}
