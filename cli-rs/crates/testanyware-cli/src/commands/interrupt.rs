//! SIGINT and SIGTERM during `file exec` and `file download`.
//!
//! Source of truth: `docs/specs/guest-command-lifecycle.md`, sections "CLI
//! interruption" and "Host CLI download", and ADR-0025. The command does its
//! cleanup when one of the two signals arrives. Then it restores the default
//! action and raises the same signal again, so the parent sees the same
//! signal death as before the handler existed.
//!
//! The handler exists only while the request of one of the two commands runs.
//! [`Interrupts::finish`] restores the default actions when the request ends,
//! so a signal during the output kills the process, as before the handler
//! existed. Tokio never removes its own handler, so without that step a later
//! signal would only wake a listener that nothing polls.
//!
//! A signal that the parent set to be ignored stays ignored. A background job
//! of a shell inherits an ignored SIGINT and must keep it. On a
//! host that is not Unix, nothing is installed and [`Interrupts::recv`] never
//! completes.

#[cfg(unix)]
use std::sync::atomic::{AtomicI32, Ordering};

/// The first signal that the handler caught, or 0. The handler stores it
/// beside tokio's own notification, so [`Interrupts::finish`] can see a
/// signal that tokio has not yet delivered to a listener.
#[cfg(unix)]
static CAUGHT: AtomicI32 = AtomicI32::new(0);

/// The signals that a command listens for, from [`Interrupts::install`] to
/// [`Interrupts::finish`] or the end of the command.
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
                    // SAFETY: the action only stores into an atomic, which is
                    // async-signal-safe. A failure leaves tokio's listener,
                    // which still sees the signal during the request.
                    let _ = unsafe {
                        signal_hook_registry::register(number, move || {
                            let _ = CAUGHT.compare_exchange(
                                0,
                                number,
                                Ordering::SeqCst,
                                Ordering::SeqCst,
                            );
                        })
                    };
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

    /// End the listening after the request has ended. Restore the default
    /// action of each signal that this listened for, so a later signal kills
    /// the process at once. Return a signal that the handler caught before
    /// the restore.
    ///
    /// Limit: a handler that runs on another thread at the moment of the
    /// restore, and has not yet stored its signal, is missed. That window
    /// lasts from the delivery to the store, a few instructions.
    pub(crate) fn finish(self) -> Option<i32> {
        #[cfg(unix)]
        {
            for (number, _) in &self.listeners {
                restore_default(*number);
            }
            match CAUGHT.load(Ordering::SeqCst) {
                0 => None,
                number => Some(number),
            }
        }
        #[cfg(not(unix))]
        {
            None
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

/// Restore the default action of `number`.
#[cfg(unix)]
fn restore_default(number: i32) {
    // SAFETY: signal(2) with SIG_DFL takes only the number of a valid signal.
    // The handler of the registry is then no longer called for it.
    unsafe {
        libc::signal(number, libc::SIG_DFL);
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
    {
        restore_default(number);
        // SAFETY: raise(3) takes only the number of a valid signal, and this
        // function never returns into Rust code that the new action could
        // affect.
        unsafe {
            libc::raise(number);
        }
    }
    std::process::exit(128 + number)
}
