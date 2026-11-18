//! SIGINT and SIGTERM during `file exec` and `file download`.
//!
//! Source of truth: `docs/specs/guest-command-lifecycle.md`, sections "CLI
//! interruption" and "Host CLI download", and ADR-0025. The command does its
//! cleanup when one of the two signals arrives. Then it restores the default
//! action and raises the same signal again, so the parent sees the same
//! signal death as before the handler existed.
//!
//! The handler exists only while one of the two commands runs. A signal that
//! the parent set to be ignored stays ignored, because a background job of a
//! shell inherits an ignored SIGINT and must keep it. On a host that is not
//! Unix, nothing is installed and [`Interrupts::recv`] never completes.

/// The signals that a command listens for, from [`Interrupts::install`] to
/// the end of the command.
pub(crate) struct Interrupts {
    #[cfg(unix)]
    listeners: Vec<(i32, tokio::signal::unix::Signal)>,
}

impl Interrupts {
    /// Listen for SIGINT and SIGTERM, except a signal that the parent set to
    /// be ignored.
    pub(crate) fn install() -> Self {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            let mut listeners = Vec::new();
            for (number, kind) in [
                (libc::SIGINT, SignalKind::interrupt()),
                (libc::SIGTERM, SignalKind::terminate()),
            ] {
                if is_ignored(number) {
                    continue;
                }
                // A failure leaves the default action in place, as before.
                if let Ok(listener) = signal(kind) {
                    listeners.push((number, listener));
                }
            }
            Interrupts { listeners }
        }
        #[cfg(not(unix))]
        {
            Interrupts {}
        }
    }

    /// The number of the first signal that arrives. Never completes when no
    /// listener is installed.
    pub(crate) async fn recv(&mut self) -> i32 {
        #[cfg(unix)]
        {
            if self.listeners.is_empty() {
                return std::future::pending().await;
            }
            let arrivals = self.listeners.iter_mut().map(|(number, listener)| {
                let number = *number;
                Box::pin(async move {
                    listener.recv().await;
                    number
                })
            });
            futures_util::future::select_all(arrivals).await.0
        }
        #[cfg(not(unix))]
        {
            std::future::pending().await
        }
    }
}

/// Whether the current action of `number` is to ignore it.
#[cfg(unix)]
fn is_ignored(number: i32) -> bool {
    // SAFETY: a null new action makes sigaction(2) only read the current one
    // into `current`, which is a valid, zeroed struct.
    unsafe {
        let mut current: libc::sigaction = std::mem::zeroed();
        libc::sigaction(number, std::ptr::null(), &mut current) == 0
            && current.sa_sigaction == libc::SIG_IGN
    }
}

/// Restore the default action of `number` and raise it again, so the
/// process dies of the same signal. The exit with 128 plus the number is a
/// last resort, for a signal that the default action did not end.
pub(crate) fn raise_again(number: i32) -> ! {
    use std::io::Write;
    let _ = std::io::stdout().flush();
    let _ = std::io::stderr().flush();
    #[cfg(unix)]
    // SAFETY: signal(2) with SIG_DFL and raise(3) take only the number of a
    // valid signal, and this function never returns into Rust code that the
    // new action could affect.
    unsafe {
        libc::signal(number, libc::SIG_DFL);
        libc::raise(number);
    }
    std::process::exit(128 + number)
}
