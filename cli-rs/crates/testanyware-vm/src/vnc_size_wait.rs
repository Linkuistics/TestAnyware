//! The VNC size wait: the last step of `vm start` for a tart guest (ADR-0018).
//!
//! The first RFB session after a tart boot can get a stale 1280x720 in ServerInit. This happens
//! when its ClientInit comes fast, as the ClientInit of each command does. Each later session
//! gets the real size. So the wait opens short sessions to the guest's VNC server until ServerInit
//! reports the size that `vm start` set. Each session closes after ServerInit, and no session
//! outlives the wait. The measurements are in
//! `docs/research/tart-vnc-stale-first-size-macos-27.md`.

use std::fmt;
use std::future::Future;
use std::time::Duration;

use testanyware_rfb::RfbConnection;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

/// The pause between the end of one session and the start of the next.
pub const SESSION_INTERVAL: Duration = Duration::from_millis(250);

/// The wait stops after this time, also during a session.
pub const WAIT_BOUND: Duration = Duration::from_secs(30);

/// What one session read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reading {
    /// The width and height in ServerInit.
    Size(u32, u32),
    /// The session ended before ServerInit, with this error.
    Failed(String),
}

impl fmt::Display for Reading {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Reading::Size(w, h) => write!(f, "{w}x{h}"),
            Reading::Failed(error) => write!(f, "no size ({error})"),
        }
    }
}

/// How the wait ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SizeWait {
    /// The first session read the target.
    AtOnce,
    /// A later session read the target. `stale` is what the first session read.
    Waited {
        stale: Reading,
        sessions: u32,
        elapsed: Duration,
    },
    /// No session read the target within [`WAIT_BOUND`]. `last` is what the last session read.
    Bound { last: Reading, sessions: u32 },
    /// The start was cancelled during the wait.
    Cancelled,
}

impl SizeWait {
    /// The line for the start output, if the wait has one. A wait that read the target at once
    /// prints nothing, so the start output shows whether a later release removed the fault.
    pub fn line(&self, target: (u32, u32)) -> Option<String> {
        let (tw, th) = target;
        match self {
            SizeWait::AtOnce | SizeWait::Cancelled => None,
            SizeWait::Waited {
                stale,
                sessions,
                elapsed,
            } => Some(format!(
                "VNC size wait: the first session read {stale}. Session {sessions} read \
                 {tw}x{th} after {:.2} s.",
                elapsed.as_secs_f64()
            )),
            SizeWait::Bound { last, sessions } => Some(format!(
                "  warning: VNC size wait: no session read {tw}x{th} within {} s. The last of \
                 {sessions} sessions read {last}. The first command can read that size.",
                WAIT_BOUND.as_secs()
            )),
        }
    }
}

/// Open sessions with `session` until one reads `target`, the bound passes or `cancel` fires.
///
/// `session` opens one RFB session, reads ServerInit, closes the session and returns the size.
/// [`read_server_init`] is the real one, and a test gives a scripted one.
pub async fn wait_for_size<F, Fut>(
    target: (u32, u32),
    cancel: &CancellationToken,
    mut session: F,
) -> SizeWait
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<(u32, u32), String>>,
{
    let started = Instant::now();
    let deadline = started + WAIT_BOUND;
    let wanted = Reading::Size(target.0, target.1);
    let mut first: Option<Reading> = None;
    let mut sessions = 0u32;
    loop {
        sessions += 1;
        // A cancel drops the session future, and so it closes the session at once.
        let reading = tokio::select! {
            biased;
            _ = cancel.cancelled() => return SizeWait::Cancelled,
            read = tokio::time::timeout_at(deadline, session()) => match read {
                Ok(Ok((w, h))) => Reading::Size(w, h),
                Ok(Err(error)) => Reading::Failed(error),
                Err(_) => Reading::Failed("no ServerInit before the bound".to_string()),
            },
        };
        if reading == wanted {
            return match first {
                None => SizeWait::AtOnce,
                Some(stale) => SizeWait::Waited {
                    stale,
                    sessions,
                    elapsed: started.elapsed(),
                },
            };
        }
        let next = Instant::now() + SESSION_INTERVAL;
        if next >= deadline {
            return SizeWait::Bound {
                last: reading,
                sessions,
            };
        }
        first.get_or_insert(reading);
        tokio::select! {
            biased;
            _ = cancel.cancelled() => return SizeWait::Cancelled,
            _ = tokio::time::sleep_until(next) => {}
        }
    }
}

/// One session: connect, complete the handshake, read the physical size in ServerInit, and
/// close. The handshake sends ClientInit as fast as each command does, so the session sees what
/// the first command would see. It asks for no frame.
pub async fn read_server_init(
    host: &str,
    port: u16,
    password: Option<&str>,
) -> Result<(u32, u32), String> {
    let conn = RfbConnection::connect(host, port, password.map(str::as_bytes))
        .await
        .map_err(|e| e.to_string())?;
    Ok(conn.physical_framebuffer_size())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    const TARGET: (u32, u32) = (1920, 1080);
    const STALE: (u32, u32) = (1280, 720);

    /// A session seam that returns each scripted reading in turn, then the last one for ever.
    #[allow(clippy::type_complexity)] // Stable Rust cannot name the `impl` type in an alias.
    fn scripted(
        readings: Vec<Result<(u32, u32), String>>,
    ) -> (
        impl FnMut() -> std::future::Ready<Result<(u32, u32), String>>,
        Arc<Mutex<u32>>,
    ) {
        let calls = Arc::new(Mutex::new(0));
        let counter = Arc::clone(&calls);
        let mut queue: VecDeque<_> = readings.into();
        let session = move || {
            *counter.lock().unwrap() += 1;
            let next = if queue.len() > 1 {
                queue.pop_front()
            } else {
                queue.front().cloned()
            };
            std::future::ready(next.expect("a script"))
        };
        (session, calls)
    }

    #[tokio::test(start_paused = true)]
    async fn the_target_at_the_first_session_gives_one_session_and_no_line() {
        let (session, calls) = scripted(vec![Ok(TARGET)]);
        let outcome = wait_for_size(TARGET, &CancellationToken::new(), session).await;
        assert_eq!(outcome, SizeWait::AtOnce);
        assert_eq!(*calls.lock().unwrap(), 1);
        assert_eq!(outcome.line(TARGET), None);
    }

    #[tokio::test(start_paused = true)]
    async fn two_stale_reads_then_the_target_give_three_sessions_and_a_line() {
        let (session, calls) = scripted(vec![Ok(STALE), Ok(STALE), Ok(TARGET)]);
        let outcome = wait_for_size(TARGET, &CancellationToken::new(), session).await;
        assert_eq!(*calls.lock().unwrap(), 3);
        let SizeWait::Waited {
            stale,
            sessions,
            elapsed,
        } = &outcome
        else {
            panic!("expected a wait, got {outcome:?}");
        };
        assert_eq!(*stale, Reading::Size(1280, 720));
        assert_eq!(*sessions, 3);
        assert_eq!(*elapsed, SESSION_INTERVAL * 2);
        let line = outcome.line(TARGET).expect("a waited start names the wait");
        assert_eq!(
            line,
            "VNC size wait: the first session read 1280x720. Session 3 read 1920x1080 after 0.50 s."
        );
    }

    #[tokio::test(start_paused = true)]
    async fn stale_reads_for_ever_end_at_the_bound_with_a_warning() {
        let (session, calls) = scripted(vec![Ok(STALE)]);
        let started = Instant::now();
        let outcome = wait_for_size(TARGET, &CancellationToken::new(), session).await;
        let waited = started.elapsed();
        // Sessions at 0, 0.25 s, ... The session at 29.75 s is the last, because the next one
        // would start at the bound.
        assert_eq!(waited, WAIT_BOUND - SESSION_INTERVAL);
        let calls = *calls.lock().unwrap();
        assert_eq!(calls, 120);
        assert_eq!(
            outcome,
            SizeWait::Bound {
                last: Reading::Size(1280, 720),
                sessions: calls
            }
        );
        let line = outcome.line(TARGET).expect("the bound warns");
        assert!(
            line.starts_with("  warning: VNC size wait: no session read 1920x1080 within 30 s."),
            "{line}"
        );
        assert!(line.contains("read 1280x720"), "{line}");
    }

    #[tokio::test(start_paused = true)]
    async fn a_session_that_hangs_ends_at_the_bound() {
        let started = Instant::now();
        let outcome = wait_for_size(TARGET, &CancellationToken::new(), || {
            std::future::pending::<Result<(u32, u32), String>>()
        })
        .await;
        assert_eq!(started.elapsed(), WAIT_BOUND);
        assert_eq!(
            outcome,
            SizeWait::Bound {
                last: Reading::Failed("no ServerInit before the bound".into()),
                sessions: 1
            }
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_failed_session_counts_as_a_stale_read() {
        let (session, _) = scripted(vec![Err("connection refused".into()), Ok(TARGET)]);
        let outcome = wait_for_size(TARGET, &CancellationToken::new(), session).await;
        let line = outcome.line(TARGET).unwrap();
        assert!(
            line.contains("the first session read no size (connection refused)."),
            "{line}"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_cancel_during_the_pause_ends_the_wait() {
        let cancel = CancellationToken::new();
        let (session, calls) = scripted(vec![Ok(STALE)]);
        let canceller = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(600)).await;
            canceller.cancel();
        });
        let started = Instant::now();
        let outcome = wait_for_size(TARGET, &cancel, session).await;
        assert_eq!(outcome, SizeWait::Cancelled);
        assert_eq!(started.elapsed(), Duration::from_millis(600));
        assert_eq!(*calls.lock().unwrap(), 3);
        assert_eq!(outcome.line(TARGET), None);
    }

    #[tokio::test(start_paused = true)]
    async fn a_cancel_during_a_session_ends_the_wait() {
        let cancel = CancellationToken::new();
        let canceller = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(2)).await;
            canceller.cancel();
        });
        let started = Instant::now();
        let outcome = wait_for_size(TARGET, &cancel, || {
            std::future::pending::<Result<(u32, u32), String>>()
        })
        .await;
        assert_eq!(outcome, SizeWait::Cancelled);
        assert_eq!(started.elapsed(), Duration::from_secs(2));
    }

    // ---- the real session, against a local RFB listener ----

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    /// A listener that answers each connection's ServerInit with the next scripted size, then
    /// reads until the client closes. It records each size it sent and each close it saw.
    async fn scripted_listener(sizes: Vec<(u16, u16)>) -> (u16, Arc<Mutex<Vec<String>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let log = Arc::new(Mutex::new(Vec::new()));
        let record = Arc::clone(&log);
        tokio::spawn(async move {
            let mut sizes: VecDeque<_> = sizes.into();
            loop {
                let (mut conn, _) = listener.accept().await.unwrap();
                let (w, h) = if sizes.len() > 1 {
                    sizes.pop_front().unwrap()
                } else {
                    sizes[0]
                };
                let record = Arc::clone(&record);
                tokio::spawn(async move {
                    conn.write_all(b"RFB 003.008\n").await.unwrap();
                    let mut version = [0u8; 12];
                    conn.read_exact(&mut version).await.unwrap();
                    conn.write_all(&[1, 1]).await.unwrap(); // one security type: None
                    let mut byte = [0u8; 1];
                    conn.read_exact(&mut byte).await.unwrap();
                    conn.write_all(&0u32.to_be_bytes()).await.unwrap();
                    conn.read_exact(&mut byte).await.unwrap(); // ClientInit
                    let mut init = Vec::new();
                    init.extend_from_slice(&w.to_be_bytes());
                    init.extend_from_slice(&h.to_be_bytes());
                    init.extend_from_slice(&testanyware_rfb::PixelFormat::rgba32_le().encode());
                    init.extend_from_slice(&4u32.to_be_bytes());
                    init.extend_from_slice(b"tart");
                    conn.write_all(&init).await.unwrap();
                    record.lock().unwrap().push(format!("sent {w}x{h}"));
                    // Read SetPixelFormat, SetEncodings and anything else until the close.
                    let mut sink = [0u8; 256];
                    while matches!(conn.read(&mut sink).await, Ok(n) if n > 0) {}
                    record.lock().unwrap().push(format!("closed {w}x{h}"));
                });
            }
        });
        (port, log)
    }

    #[tokio::test]
    async fn the_real_session_reads_server_init_and_closes() {
        let (port, log) = scripted_listener(vec![(1280, 720), (1920, 1080)]).await;
        let outcome = wait_for_size(TARGET, &CancellationToken::new(), || {
            read_server_init("127.0.0.1", port, None)
        })
        .await;
        let SizeWait::Waited {
            stale, sessions, ..
        } = &outcome
        else {
            panic!("expected a wait, got {outcome:?}");
        };
        assert_eq!((stale, *sessions), (&Reading::Size(1280, 720), 2));
        // Each session closed: the listener read the end of each connection.
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while log.lock().unwrap().len() < 4 && std::time::Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let mut log = log.lock().unwrap().clone();
        log.sort();
        assert_eq!(
            log,
            [
                "closed 1280x720",
                "closed 1920x1080",
                "sent 1280x720",
                "sent 1920x1080"
            ]
        );
    }

    #[tokio::test]
    async fn the_real_session_reports_a_refused_connection() {
        // Bind and drop a listener, so its port refuses.
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let error = read_server_init("127.0.0.1", port, None).await.unwrap_err();
        assert!(!error.is_empty());
    }
}
