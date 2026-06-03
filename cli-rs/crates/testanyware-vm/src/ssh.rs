//! `russh`-backed SSH/SFTP provisioning helper (ADR-0007).
//!
//! Golden-image creation drives a throwaway setup VM almost entirely over
//! SSH: dozens of `exec` calls plus a handful of SFTP `upload`s (pubkey,
//! wallpaper helper+png, agent binary, LaunchAgent plist). This module is
//! the **reusable provisioning seam** the `110-vm-create-golden-macos`
//! boot leaves (`020`, `030`) sit on, and which the Tier-2 linux/win
//! goldens reuse unchanged — so it carries **no macOS/tart assumptions**.
//! Backend-specific concerns (IP discovery, recovery-mode RFB automation)
//! live in the boot leaves, not here.
//!
//! `russh` is pure Rust and async (ADR-0007): native password *and* pubkey
//! auth in-process — no `SSH_ASKPASS` dance ([[vm-ssh-from-harness]]) — and
//! no C dependency to fight the `zig cc` cross-build
//! ([[linux-crosscheck-zig]]). The cost is that `russh` is lower-level than
//! `ssh2`: we implement a `client::Handler` and drive `channel.exec()` /
//! collect `ChannelMsg` ourselves, and ride `russh-sftp` over a subsystem
//! channel for `upload`.
//!
//! The auth shape matches the script: the vanilla Cirrus Labs image only
//! permits **password** auth (`admin/admin`) on first contact, so
//! `connect_password` is used **once** to install the host pubkey, and
//! `connect_key` (pubkey auth) for every subsequent connection.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use russh::client::{self, Handle};
use russh::keys::{load_secret_key, PrivateKeyWithHashAlg};
use russh::ChannelMsg;
use russh_sftp::client::SftpSession;
use russh_sftp::protocol::OpenFlags;
use tokio::io::AsyncWriteExt;

use crate::error::VmError;

/// Result of one `SshSession::exec` — the `vm_ssh` equivalent.
///
/// A non-zero `exit_code` is **not** an error: it is the remote command's
/// status, returned for the caller to assert on (mirroring how the script
/// branches on `vm_ssh "..."` exit). Only a transport/auth failure surfaces
/// as `Err(VmError)`. `exit_code` is `-1` when the channel closed without an
/// explicit `ExitStatus` (e.g. the remote was killed by a signal).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

/// An authenticated SSH connection to the setup VM. Wraps a `russh`
/// client handle; clone-free, dropped to disconnect.
pub struct SshSession {
    handle: Handle<ClientHandler>,
}

/// `russh` client event handler. Host-key policy is **accept-any**: the
/// setup VM is a disposable throwaway, matching the script's
/// `StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null`.
struct ClientHandler;

impl ClientHandler {
    /// The host-key decision, factored out so the accept-any policy is
    /// unit-testable without a live handshake. Returns `true`
    /// unconditionally — see the type comment for why that is safe here.
    fn accept_server_key() -> bool {
        true
    }
}

impl client::Handler for ClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &russh::keys::ssh_key::PublicKey,
    ) -> Result<bool, Self::Error> {
        Ok(Self::accept_server_key())
    }
}

/// Assemble an [`ExecOutput`] from collected channel bytes. Pure (no I/O)
/// so the stdout/stderr/exit decoding is unit-tested without a channel.
/// A missing `ExitStatus` maps to `-1` (signalled / abnormal close).
fn assemble_exec_output(stdout: Vec<u8>, stderr: Vec<u8>, exit_status: Option<u32>) -> ExecOutput {
    ExecOutput {
        stdout: String::from_utf8_lossy(&stdout).into_owned(),
        stderr: String::from_utf8_lossy(&stderr).into_owned(),
        exit_code: exit_status.map(|c| c as i32).unwrap_or(-1),
    }
}

/// Default `russh` client config. A keepalive keeps long provisioning
/// commands (Homebrew install, Xcode CLT) from idling the connection out.
fn client_config() -> Arc<client::Config> {
    Arc::new(client::Config {
        keepalive_interval: Some(Duration::from_secs(30)),
        ..Default::default()
    })
}

fn ssh_err(detail: impl std::fmt::Display) -> VmError {
    VmError::SshConnectFailed { detail: detail.to_string() }
}

impl SshSession {
    /// Open a session authenticating with a **password** — the one initial
    /// connection to the vanilla `admin/admin` image. Ports the script's
    /// `SSH_ASKPASS` password step into a single in-process call.
    pub async fn connect_password(
        host: &str,
        port: u16,
        user: &str,
        password: &str,
    ) -> Result<Self, VmError> {
        let mut handle = client::connect(client_config(), (host, port), ClientHandler)
            .await
            .map_err(|e| ssh_err(format!("connect {host}:{port}: {e}")))?;
        let auth = handle
            .authenticate_password(user, password)
            .await
            .map_err(|e| ssh_err(format!("password auth as {user}: {e}")))?;
        if !auth.success() {
            return Err(ssh_err(format!("password auth rejected for {user}@{host}")));
        }
        Ok(Self { handle })
    }

    /// Open a session authenticating with a **private key** — every
    /// connection after the pubkey is installed. `key_path` is the private
    /// key (`russh` signs with it); the matching public key must already be
    /// in the remote's `authorized_keys`.
    pub async fn connect_key(
        host: &str,
        port: u16,
        user: &str,
        key_path: &Path,
    ) -> Result<Self, VmError> {
        let key = load_secret_key(key_path, None)
            .map_err(|e| ssh_err(format!("load private key {}: {e}", key_path.display())))?;
        let mut handle = client::connect(client_config(), (host, port), ClientHandler)
            .await
            .map_err(|e| ssh_err(format!("connect {host}:{port}: {e}")))?;
        let auth = handle
            .authenticate_publickey(user, PrivateKeyWithHashAlg::new(Arc::new(key), None))
            .await
            .map_err(|e| ssh_err(format!("pubkey auth as {user}: {e}")))?;
        if !auth.success() {
            return Err(ssh_err(format!("pubkey auth rejected for {user}@{host}")));
        }
        Ok(Self { handle })
    }

    /// Poll `connect_password` until the SSH service answers or the deadline
    /// passes — the in-process replacement for the script's
    /// `Waiting for SSH...` loop. Kept **tart-agnostic**: IP discovery
    /// (`tart list` state-gated, [[tart-ip-lies]]) is the boot leaf's job;
    /// this only retries the connect on an already-resolved `host`.
    pub async fn wait_for_password(
        host: &str,
        port: u16,
        user: &str,
        password: &str,
        attempts: u32,
        interval: Duration,
    ) -> Result<Self, VmError> {
        let mut last = ssh_err("no connection attempts made");
        for attempt in 0..attempts {
            match Self::connect_password(host, port, user, password).await {
                Ok(session) => return Ok(session),
                Err(e) => last = e,
            }
            if attempt + 1 < attempts {
                tokio::time::sleep(interval).await;
            }
        }
        Err(last)
    }

    /// Run `command` to completion, capturing stdout, stderr, and exit code
    /// — the `vm_ssh` workhorse. A non-zero exit is returned in the receipt,
    /// not raised; only a transport failure errors.
    pub async fn exec(&self, command: &str) -> Result<ExecOutput, VmError> {
        let mut channel = self
            .handle
            .channel_open_session()
            .await
            .map_err(|e| ssh_err(format!("open exec channel: {e}")))?;
        channel
            .exec(true, command)
            .await
            .map_err(|e| ssh_err(format!("exec `{command}`: {e}")))?;

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let mut exit_status = None;
        while let Some(msg) = channel.wait().await {
            match msg {
                ChannelMsg::Data { ref data } => stdout.extend_from_slice(data),
                // ext == 1 is the SSH stderr stream; other extended-data
                // codes are not used by a shell command.
                ChannelMsg::ExtendedData { ref data, ext } if ext == 1 => {
                    stderr.extend_from_slice(data)
                }
                ChannelMsg::ExitStatus { exit_status: code } => exit_status = Some(code),
                _ => {}
            }
        }
        Ok(assemble_exec_output(stdout, stderr, exit_status))
    }

    /// Upload `local_path` to `remote_path` over SFTP — the `vm_scp`
    /// equivalent. Rides `russh-sftp` over a `sftp` subsystem channel,
    /// truncating any existing remote file. Used for the pubkey, wallpaper
    /// helper+png, agent binary, and LaunchAgent plist.
    pub async fn upload(&self, local_path: &Path, remote_path: &str) -> Result<(), VmError> {
        let contents = tokio::fs::read(local_path)
            .await
            .map_err(|e| VmError::Io(format!("read {}: {e}", local_path.display())))?;

        let channel = self
            .handle
            .channel_open_session()
            .await
            .map_err(|e| ssh_err(format!("open sftp channel: {e}")))?;
        channel
            .request_subsystem(true, "sftp")
            .await
            .map_err(|e| ssh_err(format!("request sftp subsystem: {e}")))?;
        let sftp = SftpSession::new(channel.into_stream())
            .await
            .map_err(|e| ssh_err(format!("start sftp session: {e}")))?;

        let mut file = sftp
            .open_with_flags(
                remote_path,
                OpenFlags::CREATE | OpenFlags::TRUNCATE | OpenFlags::WRITE,
            )
            .await
            .map_err(|e| ssh_err(format!("open remote {remote_path}: {e}")))?;
        file.write_all(&contents)
            .await
            .map_err(|e| ssh_err(format!("write remote {remote_path}: {e}")))?;
        file.flush()
            .await
            .map_err(|e| ssh_err(format!("flush remote {remote_path}: {e}")))?;
        file.shutdown()
            .await
            .map_err(|e| ssh_err(format!("close remote {remote_path}: {e}")))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_key_policy_accepts_any_key() {
        // ADR-0007 / script parity: the setup VM is a throwaway, so the
        // host-key check is unconditional accept. Guards against an
        // accidental tightening that would break first-contact.
        assert!(ClientHandler::accept_server_key());
    }

    #[test]
    fn assemble_exec_output_carries_streams_and_exit() {
        let out = assemble_exec_output(b"hello\n".to_vec(), b"warn\n".to_vec(), Some(0));
        assert_eq!(out.stdout, "hello\n");
        assert_eq!(out.stderr, "warn\n");
        assert_eq!(out.exit_code, 0);
    }

    #[test]
    fn assemble_exec_output_nonzero_exit_is_not_lost() {
        let out = assemble_exec_output(Vec::new(), b"boom".to_vec(), Some(127));
        assert_eq!(out.exit_code, 127);
        assert_eq!(out.stderr, "boom");
    }

    #[test]
    fn assemble_exec_output_missing_exit_status_is_minus_one() {
        // No ExitStatus message (signalled / abnormal close) → -1, never a
        // false success.
        let out = assemble_exec_output(b"partial".to_vec(), Vec::new(), None);
        assert_eq!(out.exit_code, -1);
    }

    #[test]
    fn assemble_exec_output_is_utf8_lossy() {
        let out = assemble_exec_output(vec![0xff, 0xfe], Vec::new(), Some(0));
        // Invalid UTF-8 is replaced, not a panic — matches the script's
        // tolerance of arbitrary command output.
        assert!(out.stdout.contains('\u{fffd}'));
    }
}
