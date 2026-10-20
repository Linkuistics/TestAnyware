//! The VNC keeper (ADR-0018): one silent TCP connection to a tart guest's VNC
//! port, held for as long as tart runs.
//!
//! On a macOS 27 host, Apple's `_VZVNCServer` aborts tart when a VNC client
//! connects 60 s or more after the last client left. One open connection
//! prevents that, and a bare connection that sends nothing is enough. The
//! keeper is the `testanyware` binary run as the hidden command
//! `_vnc-keeper`. `vm start` spawns it detached, and it ends itself when the
//! port refuses a new connection. `vm stop` signals it only as a backstop.
//!
//! The loop ([`hold`]) takes the host and port as values so a test can run it
//! against a local listener. The argument list and the identity check
//! ([`is_keeper_command_line`]) are pure.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;

use crate::error::VmError;

/// The hidden CLI command that runs the keeper. The contract hides each `_`
/// command from `--help` (`docs/architecture/cli-design-contract.md`, §1).
pub const KEEPER_COMMAND: &str = "_vnc-keeper";

/// The argument list that starts a keeper for `id` on `host:port`.
pub fn keeper_args(id: &str, host: &str, port: u16) -> Vec<String> {
    vec![
        KEEPER_COMMAND.to_string(),
        "--id".to_string(),
        id.to_string(),
        "--host".to_string(),
        host.to_string(),
        "--port".to_string(),
        port.to_string(),
    ]
}

/// True when `command_line` (as `ps` prints it) is the keeper of `id`.
///
/// `vm stop` signals a recorded pid only when this holds, so a pid that the
/// system recycled for another process never gets the signal. The match is on
/// whole tokens: the keeper of `testanyware-ab` does not match
/// `testanyware-abc`.
pub fn is_keeper_command_line(command_line: &str, id: &str) -> bool {
    let tokens: Vec<&str> = command_line.split_whitespace().collect();
    tokens
        .windows(3)
        .any(|w| w[0] == KEEPER_COMMAND && w[1] == "--id" && w[2] == id)
}

/// `<log_dir>/<id>.keeper.log`, with any prior run's log of the same id
/// cleared first, as `tart::fresh_log_path` does for the tart log.
pub fn fresh_keeper_log_path(log_dir: &Path, id: &str) -> PathBuf {
    let log_path = log_dir.join(format!("{id}.keeper.log"));
    // Best-effort: a missing log is the normal fresh-id case.
    let _ = std::fs::remove_file(&log_path);
    log_path
}

/// Spawn `program` as the keeper of `id`, detached. `program` is a parameter
/// so a test can replace the binary. Returns the pid.
pub fn spawn_keeper(
    program: &str,
    id: &str,
    host: &str,
    port: u16,
    log_dir: &Path,
) -> Result<i32, VmError> {
    let log_path = fresh_keeper_log_path(log_dir, id);
    crate::detached::spawn_detached(program, &keeper_args(id, host, port), &log_path)
}

/// Start the keeper of `id` from this binary, which is the Host CLI or, in the TestAnyware
/// app, the app's own executable: a keeper always matches the build that starts it. A failure
/// warns and returns `None`: the guest then runs as it does without a keeper (ADR-0018).
pub fn start_keeper(
    id: &str,
    host: &str,
    port: u16,
    log_dir: &Path,
    progress: &crate::start_context::Progress,
) -> Option<i32> {
    let spawned = std::env::current_exe()
        .map_err(|e| VmError::Io(format!("locate the testanyware binary: {e}")))
        .and_then(|exe| spawn_keeper(&exe.display().to_string(), id, host, port, log_dir));
    match spawned {
        Ok(pid) => Some(pid),
        Err(e) => {
            progress.line(&format!(
                "  warning: could not start the VNC keeper ({e}); a command after a pause of \
                 60 s or more can crash the guest on a macOS 27 host"
            ));
            None
        }
    }
}

/// Signal the recorded keeper of `id`, but only if that pid is still the
/// keeper of `id`. Best-effort: the keeper normally ended itself when tart
/// went away.
pub fn stop_keeper(id: &str, pid: Option<i32>) {
    let Some(pid) = pid else { return };
    let is_keeper = crate::process::command_line(pid)
        .is_some_and(|line| is_keeper_command_line(&line, id));
    if is_keeper {
        crate::process::terminate(pid, Duration::from_millis(100), 10);
    }
}

/// The body of the hidden `_vnc-keeper` command, for the Host CLI and for the TestAnyware app,
/// whose executable also answers that command. It holds one silent connection to `host:port`
/// until the port goes away, and logs to stderr, which `vm start` points at `<id>.keeper.log`.
/// Returns the exit code: 0 when it held a connection and the port then went away, 1 otherwise.
pub async fn run(id: &str, host: &str, port: u16) -> i32 {
    let started = Instant::now();
    let mut log = |line: &str| eprintln!("[{id} +{}s] {line}", started.elapsed().as_secs());
    let report = hold(host, port, KeeperPolicy::DEFAULT, &mut log).await;
    log(&format!(
        "exiting after {} connection(s), {} byte(s) discarded: {:?}",
        report.connections, report.bytes_discarded, report.exit
    ));
    if report.ended_normally() { 0 } else { 1 }
}

/// The timings of the keeper loop.
#[derive(Debug, Clone, Copy)]
pub struct KeeperPolicy {
    /// The wait after a connection ends, before the next connect.
    pub reconnect_delay: Duration,
    /// A connection that ends sooner than this counts as short.
    pub short_connection: Duration,
    /// The keeper exits after this many short connections in a row.
    pub max_short_in_a_row: u32,
    /// The longest a connect may take.
    pub connect_timeout: Duration,
}

impl KeeperPolicy {
    /// The values ADR-0018 describes: reconnect after about one second, and
    /// give up when connections keep ending within seconds of opening.
    pub const DEFAULT: KeeperPolicy = KeeperPolicy {
        reconnect_delay: Duration::from_secs(1),
        short_connection: Duration::from_secs(5),
        max_short_in_a_row: 5,
        connect_timeout: Duration::from_secs(5),
    };
}

/// Why the keeper loop ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeeperExit {
    /// A connect failed. After a first connection this is the normal end:
    /// the port refused because tart is gone.
    ConnectFailed(String),
    /// Connections kept ending within seconds of opening.
    ConnectionsKeepEnding,
}

/// What the keeper loop did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeeperReport {
    /// Connections that opened.
    pub connections: u32,
    /// Bytes read and thrown away (the 12-byte protocol version, per connection).
    pub bytes_discarded: u64,
    pub exit: KeeperExit,
}

impl KeeperReport {
    /// True when the keeper held a connection and ended because the port went
    /// away, which is how it ends when tart stops.
    pub fn ended_normally(&self) -> bool {
        self.connections > 0 && matches!(self.exit, KeeperExit::ConnectFailed(_))
    }
}

/// Hold one silent connection to `host:port` until the port refuses.
///
/// The keeper never writes. It reads and discards what the server sends, so
/// its receive buffer is empty when it closes, and the close is a FIN and not
/// the RST that crashes tart (ADR-0018). When a connection ends, it waits
/// `policy.reconnect_delay` and connects again. `log` receives one line per event.
pub async fn hold(
    host: &str,
    port: u16,
    policy: KeeperPolicy,
    log: &mut (dyn FnMut(&str) + Send),
) -> KeeperReport {
    let mut connections = 0u32;
    let mut bytes_discarded = 0u64;
    let mut short_in_a_row = 0u32;
    let mut buf = [0u8; 512];
    loop {
        let connect = TcpStream::connect((host, port));
        let mut stream = match tokio::time::timeout(policy.connect_timeout, connect).await {
            Ok(Ok(stream)) => stream,
            Ok(Err(e)) => {
                log(&format!("connect to {host}:{port} failed: {e}"));
                return KeeperReport { connections, bytes_discarded, exit: KeeperExit::ConnectFailed(e.to_string()) };
            }
            Err(_) => {
                log(&format!("connect to {host}:{port} timed out"));
                return KeeperReport {
                    connections,
                    bytes_discarded,
                    exit: KeeperExit::ConnectFailed("timed out".to_string()),
                };
            }
        };
        connections += 1;
        log(&format!("holding a connection to {host}:{port}"));
        let opened = Instant::now();
        loop {
            match stream.read(&mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(n) => bytes_discarded += n as u64,
            }
        }
        drop(stream);
        let held = opened.elapsed();
        log(&format!("the connection ended after {:.1} s", held.as_secs_f64()));
        if held < policy.short_connection {
            short_in_a_row += 1;
            if short_in_a_row >= policy.max_short_in_a_row {
                log(&format!("{short_in_a_row} short connections in a row; giving up"));
                return KeeperReport { connections, bytes_discarded, exit: KeeperExit::ConnectionsKeepEnding };
            }
        } else {
            short_in_a_row = 0;
        }
        tokio::time::sleep(policy.reconnect_delay).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;
    use tokio::net::TcpListener;

    const FAST: KeeperPolicy = KeeperPolicy {
        reconnect_delay: Duration::from_millis(20),
        short_connection: Duration::from_millis(300),
        max_short_in_a_row: 3,
        connect_timeout: Duration::from_secs(2),
    };

    /// Tests that bind a port take this lock for their whole run. The system
    /// reuses a freed ephemeral port at once, so a parallel test could
    /// otherwise bind the port that `exits_when_the_listener_is_gone` just freed.
    static PORT_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    async fn local_listener() -> (tokio::sync::MutexGuard<'static, ()>, TcpListener, u16) {
        let guard = PORT_LOCK.lock().await;
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let port = listener.local_addr().unwrap().port();
        (guard, listener, port)
    }

    fn quiet() -> impl FnMut(&str) + Send {
        |_: &str| {}
    }

    #[tokio::test]
    async fn holds_the_connection_and_writes_nothing() {
        let (_guard, listener, port) = local_listener().await;
        let keeper = tokio::spawn(async move { hold("127.0.0.1", port, FAST, &mut quiet()).await });

        let (mut server_side, _) = listener.accept().await.unwrap();
        server_side.write_all(b"RFB 003.008\n").await.unwrap();
        // Far longer than the short-connection limit: the keeper holds on.
        let mut byte = [0u8; 1];
        let read = tokio::time::timeout(Duration::from_millis(700), server_side.read(&mut byte)).await;
        assert!(read.is_err(), "the keeper must hold the connection and write nothing: {read:?}");
        assert!(!keeper.is_finished(), "the keeper must still be running");

        // The server ends the connection and the port: the keeper ends too.
        drop(server_side);
        drop(listener);
        let report = keeper.await.unwrap();
        assert_eq!(report.connections, 1);
        assert!(report.ended_normally(), "{report:?}");
    }

    #[tokio::test]
    async fn reads_and_discards_what_the_server_sends() {
        let (_guard, listener, port) = local_listener().await;
        let keeper = tokio::spawn(async move { hold("127.0.0.1", port, FAST, &mut quiet()).await });

        let (mut server_side, _) = listener.accept().await.unwrap();
        server_side.write_all(b"RFB 003.008\n").await.unwrap();
        // The keeper closes only after the server does, and the server reads
        // to the end: anything the keeper wrote would arrive here.
        tokio::time::sleep(Duration::from_millis(400)).await;
        server_side.shutdown().await.unwrap();
        let mut from_keeper = Vec::new();
        server_side.read_to_end(&mut from_keeper).await.ok();
        assert!(from_keeper.is_empty(), "the keeper wrote {from_keeper:?}");
        drop(server_side);
        drop(listener);

        let report = keeper.await.unwrap();
        assert_eq!(report.bytes_discarded, 12);
    }

    #[tokio::test]
    async fn connects_again_after_the_server_closes_the_connection() {
        let (_guard, listener, port) = local_listener().await;
        let keeper = tokio::spawn(async move { hold("127.0.0.1", port, FAST, &mut quiet()).await });

        // First connection lasts longer than a short one, so it does not count against the keeper.
        let (first, _) = listener.accept().await.unwrap();
        tokio::time::sleep(Duration::from_millis(400)).await;
        drop(first);
        let (second, _) = tokio::time::timeout(Duration::from_secs(2), listener.accept())
            .await
            .expect("the keeper must connect again")
            .unwrap();
        drop(second);
        drop(listener);

        let report = keeper.await.unwrap();
        assert_eq!(report.connections, 2);
        assert!(report.ended_normally(), "{report:?}");
    }

    #[tokio::test]
    async fn exits_when_the_listener_is_gone() {
        let (_guard, listener, port) = local_listener().await;
        drop(listener);
        let report = hold("127.0.0.1", port, FAST, &mut quiet()).await;
        assert_eq!(report.connections, 0);
        assert!(matches!(report.exit, KeeperExit::ConnectFailed(_)), "{report:?}");
        assert!(!report.ended_normally(), "no connection was ever held");
    }

    #[tokio::test]
    async fn exits_and_does_not_loop_when_connections_keep_ending_at_once() {
        let (_guard, listener, port) = local_listener().await;
        let server = tokio::spawn(async move {
            let mut accepted = 0u32;
            while let Ok((stream, _)) = listener.accept().await {
                accepted += 1;
                drop(stream);
            }
            accepted
        });
        let report = hold("127.0.0.1", port, FAST, &mut quiet()).await;
        assert_eq!(report.exit, KeeperExit::ConnectionsKeepEnding);
        assert_eq!(report.connections, FAST.max_short_in_a_row);
        server.abort();
    }

    #[tokio::test]
    async fn a_long_connection_resets_the_short_count() {
        let (_guard, listener, port) = local_listener().await;
        let keeper = tokio::spawn(async move { hold("127.0.0.1", port, FAST, &mut quiet()).await });
        // Two short connections, then a long one, then two short ones: no
        // streak reaches three, so the keeper is still connecting.
        for hold_for in [0u64, 0, 400, 0, 0] {
            let (stream, _) = listener.accept().await.unwrap();
            tokio::time::sleep(Duration::from_millis(hold_for)).await;
            drop(stream);
        }
        assert!(!keeper.is_finished(), "a long connection must reset the streak");
        keeper.abort();
    }

    #[test]
    fn the_argument_list_names_the_command_id_host_and_port() {
        assert_eq!(
            keeper_args("testanyware-ab12cd34", "127.0.0.1", 58372),
            ["_vnc-keeper", "--id", "testanyware-ab12cd34", "--host", "127.0.0.1", "--port", "58372"]
        );
    }

    #[test]
    fn the_identity_check_accepts_only_the_keeper_of_this_id() {
        let line = "/opt/homebrew/bin/testanyware _vnc-keeper --id testanyware-ab12cd34 \
                    --host 127.0.0.1 --port 58372";
        assert!(is_keeper_command_line(line, "testanyware-ab12cd34"));
        // Another id, including one that is a prefix of this id.
        assert!(!is_keeper_command_line(line, "testanyware-ab12cd3"));
        assert!(!is_keeper_command_line(line, "testanyware-00000000"));
        // A recycled pid: some other process.
        assert!(!is_keeper_command_line("/usr/bin/ssh host", "testanyware-ab12cd34"));
        assert!(!is_keeper_command_line("tart run testanyware-ab12cd34 --no-graphics", "testanyware-ab12cd34"));
        // An id with no `_vnc-keeper` token before it.
        assert!(!is_keeper_command_line("testanyware vm stop --id testanyware-ab12cd34", "testanyware-ab12cd34"));
        assert!(!is_keeper_command_line("", "testanyware-ab12cd34"));
    }

    #[test]
    fn the_identity_check_accepts_what_keeper_args_builds() {
        let mut line = String::from("/some path/testanyware");
        for arg in keeper_args("testanyware-ab12cd34", "127.0.0.1", 1) {
            line.push(' ');
            line.push_str(&arg);
        }
        assert!(is_keeper_command_line(&line, "testanyware-ab12cd34"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn spawn_passes_the_arguments_and_clears_an_old_log() {
        let dir = tempfile::tempdir().unwrap();
        let id = "testanyware-ab12cd34";
        let log = dir.path().join(format!("{id}.keeper.log"));
        std::fs::write(&log, "an earlier run\n").unwrap();

        // `echo` stands in for the testanyware binary and prints its arguments.
        let pid = spawn_keeper("/bin/echo", id, "127.0.0.1", 58372, dir.path()).expect("spawn");
        assert!(pid > 0);
        for _ in 0..50 {
            if std::fs::read_to_string(&log).is_ok_and(|s| s.contains("58372")) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        let logged = std::fs::read_to_string(&log).unwrap();
        assert_eq!(logged.trim(), "_vnc-keeper --id testanyware-ab12cd34 --host 127.0.0.1 --port 58372");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn stop_leaves_a_process_that_is_not_the_keeper_alone() {
        // A `sleep` stands in for a pid that the system recycled.
        // `terminate` reaps the child below, as in process.rs's tests.
        #[allow(clippy::zombie_processes)]
        let child = std::process::Command::new("sleep").arg("30").spawn().unwrap();
        let pid = child.id() as i32;
        stop_keeper("testanyware-ab12cd34", Some(pid));
        assert!(crate::process::process_alive(pid), "a process that is not the keeper must not be signalled");
        crate::process::terminate(pid, Duration::from_millis(100), 10);
        stop_keeper("testanyware-ab12cd34", None);
    }
}
