//! The reach oracle for a tart guest's agent (ADR-0019).
//!
//! macOS Local Network privacy can refuse this process a route to the guest
//! subnet (`192.168.64.0/24`) while the guest's agent is healthy. The refusal
//! looks like `No route to host` (errno 65), and so does a guest that has not
//! answered ARP yet. A platform-signed tool such as `/usr/bin/nc` is exempt
//! from the check, so its success where this process fails proves the
//! refusal is about this process. The oracle only diagnoses: no agent call is
//! ever routed through the tool.

use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;

/// The platform-signed TCP tool that macOS ships. A copy of it elsewhere, or
/// a re-signed copy, loses the exemption, so only this path counts.
const PLATFORM_NC: &str = "/usr/bin/nc";

/// How long `nc` may take to connect or fail, and a hard bound on the wait.
const CONNECT_SECS: u64 = 3;
const HARD_LIMIT: Duration = Duration::from_secs(CONNECT_SECS + 3);

/// Does the platform `nc` connect to `host:port`? Any failure to ask, such as a
/// missing tool or a timeout, answers `false`: the oracle proves, it never
/// guesses.
pub async fn platform_tool_reaches(host: &str, port: u16) -> bool {
    let mut command = Command::new(PLATFORM_NC);
    command
        .args([
            "-z",
            "-G",
            &CONNECT_SECS.to_string(),
            host,
            &port.to_string(),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let Ok(mut child) = command.spawn() else {
        return false;
    };
    matches!(
        tokio::time::timeout(HARD_LIMIT, child.wait()).await,
        Ok(Ok(status)) if status.success()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn reaches_a_listening_port() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        assert!(platform_tool_reaches("127.0.0.1", port).await);
    }

    #[tokio::test]
    async fn does_not_reach_a_closed_port() {
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        assert!(!platform_tool_reaches("127.0.0.1", port).await);
    }
}
