//! HMP (Human Monitor Protocol) client for a QEMU monitor unix socket.
//!
//! Port of `QEMUMonitorClient.swift`. Unlike the Swift version, this
//! talks to the socket directly via `tokio::net::UnixStream` — the
//! `nc -U` subprocess was a Foundation-`Process` workaround that does
//! not apply to Rust.
//!
//! **Windows host (`#[cfg(not(unix))]`):** `tokio::net::UnixStream` does
//! not exist on Windows, so the connecting `send` body is `#[cfg(unix)]`
//! and the Windows arm returns `ErrorKind::Unsupported`. This is
//! unreachable in practice — `vm start` is gated off the Windows host one
//! layer up by `preflight::check_host_supports_local_qemu` (the
//! local-QEMU VM-host is build-verified only on Windows; ADR-0009
//! no-silent-caps) — but the module must still *compile* for the Windows
//! cross-build, and the honest error keeps the gap loud if it is ever
//! reached. The pure HMP parsers below are platform-independent.

use std::path::{Path, PathBuf};
use std::time::Duration;

#[cfg(unix)]
use tokio::io::{AsyncReadExt, AsyncWriteExt};
#[cfg(unix)]
use tokio::net::UnixStream;

/// HMP client bound to one monitor socket path.
pub struct QemuMonitorClient {
    socket_path: PathBuf,
}

impl QemuMonitorClient {
    pub fn new(socket_path: impl Into<PathBuf>) -> Self {
        Self { socket_path: socket_path.into() }
    }

    /// Send `command` over the monitor socket and return whatever the
    /// monitor writes within `drain`. HMP is line-oriented; the monitor
    /// keeps the connection open, so we read until `drain` elapses.
    #[cfg(unix)]
    pub async fn send(&self, command: &str, drain: Duration) -> std::io::Result<String> {
        let mut stream = UnixStream::connect(&self.socket_path).await?;
        stream.write_all(command.as_bytes()).await?;
        stream.write_all(b"\n").await?;
        stream.flush().await?;

        let mut buf = Vec::new();
        let mut chunk = [0u8; 4096];
        // Best-effort read: a closed peer or an elapsed deadline both end
        // the loop; the parsers tolerate the HMP banner noise either way.
        let _ = tokio::time::timeout(drain, async {
            loop {
                match stream.read(&mut chunk).await {
                    Ok(0) => break,
                    Ok(n) => buf.extend_from_slice(&chunk[..n]),
                    Err(_) => break,
                }
            }
        })
        .await;
        Ok(String::from_utf8_lossy(&buf).into_owned())
    }

    /// Windows arm: the AF_UNIX QEMU monitor has no Windows equivalent and
    /// the local-QEMU VM-host is build-verified only here, so this returns
    /// an honest `Unsupported` error rather than silently appearing to
    /// work. Unreachable in practice — `vm start` is gated off the Windows
    /// host upstream (`preflight::check_host_supports_local_qemu`) — but
    /// keeps the gap loud if a future caller bypasses that gate.
    #[cfg(not(unix))]
    pub async fn send(&self, _command: &str, _drain: Duration) -> std::io::Result<String> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "QEMU monitor over AF_UNIX is unavailable on this host; the local-QEMU \
             VM-host is not supported on Windows (build-verified only, ADR-0009)",
        ))
    }

    /// Poll `info usernet` until the guest→host forward port appears.
    pub async fn agent_port(&self, attempts: u32, interval: Duration) -> Option<u16> {
        for attempt in 0..attempts {
            if let Ok(resp) = self.send("info usernet", Duration::from_millis(500)).await {
                if let Some(port) = parse_agent_port(&resp) {
                    return Some(port);
                }
            }
            if attempt + 1 < attempts {
                tokio::time::sleep(interval).await;
            }
        }
        None
    }

    /// Poll `info vnc` until the listening VNC port appears.
    pub async fn vnc_port(&self, attempts: u32, interval: Duration) -> Option<u16> {
        for attempt in 0..attempts {
            if let Ok(resp) = self.send("info vnc", Duration::from_millis(500)).await {
                if let Some(port) = parse_vnc_port(&resp) {
                    return Some(port);
                }
            }
            if attempt + 1 < attempts {
                tokio::time::sleep(interval).await;
            }
        }
        None
    }

    /// Best-effort `set_password vnc <password>`. The monitor may not
    /// accept connections immediately after launch — retry and swallow
    /// errors. Ports `QEMUMonitorClient.setVNCPassword`.
    pub async fn set_vnc_password(&self, password: &str, attempts: u32) {
        let sanitised: String = password.chars().filter(|c| *c != '\n' && *c != '\r').collect();
        for attempt in 0..attempts {
            let _ = self
                .send(&format!("set_password vnc {sanitised}"), Duration::from_millis(300))
                .await;
            if attempt + 1 < attempts {
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }
    }

    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }
}

/// Parse the host-forward port from `info usernet`. The first
/// `HOST_FORWARD` row wins. `str::lines()` strips trailing `\r`, so CRLF
/// monitor responses parse correctly (decision log 2026-04-20).
pub fn parse_agent_port(info_usernet: &str) -> Option<u16> {
    for line in info_usernet.lines() {
        if !line.contains("HOST_FORWARD") {
            continue;
        }
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() >= 4 {
            if let Ok(port) = fields[3].parse::<u16>() {
                return Some(port);
            }
        }
    }
    None
}

/// Parse the listening VNC port from `info vnc` — the digits after the
/// first `127.0.0.1:` marker.
pub fn parse_vnc_port(info_vnc: &str) -> Option<u16> {
    const MARKER: &str = "127.0.0.1:";
    let idx = info_vnc.find(MARKER)?;
    let digits: String = info_vnc[idx + MARKER.len()..]
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_agent_port_reads_host_forward_row() {
        // QEMU `info usernet` row layout: TCP[HOST_FORWARD] <fd> * <hostport> <guest> ...
        let resp = "Hub -1 (net0):\r\n  Protocol[State]    FD  Source Address  Port   Dest. Address  Port\r\n  TCP[HOST_FORWARD]  10   *               51234        10.0.2.15     8648\r\n";
        assert_eq!(parse_agent_port(resp), Some(51234));
    }

    #[test]
    fn parse_agent_port_handles_crlf_collapsed_response() {
        // QEMU monitor responses use CRLF line endings. Rust's
        // str::lines() splits on \r\n correctly (stripping both), so the
        // HOST_FORWARD row parses cleanly. This is the Rust analogue of
        // the 2026-04-20 decision-log regression: there, Swift's
        // grapheme-cluster split treated \r\n as a single Character and
        // collapsed the whole response into one unparseable line.
        let resp = "header\r\nTCP[HOST_FORWARD]  10   *  49999  10.0.2.15  8648\r\n";
        assert_eq!(parse_agent_port(resp), Some(49999));
    }

    #[test]
    fn parse_agent_port_returns_none_without_a_forward_row() {
        assert_eq!(parse_agent_port("Hub -1 (net0):\r\nno forwards here\r\n"), None);
    }

    #[test]
    fn parse_vnc_port_reads_server_address() {
        let resp = "Server:\r\n     address: 127.0.0.1:5901\r\n  auth: vnc\r\n";
        assert_eq!(parse_vnc_port(resp), Some(5901));
    }

    #[test]
    fn parse_vnc_port_returns_none_when_absent() {
        assert_eq!(parse_vnc_port("Server:\r\n  none\r\n"), None);
    }
}
