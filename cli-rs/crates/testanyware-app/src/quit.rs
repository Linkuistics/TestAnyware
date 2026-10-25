//! The quit of the app (spec "admission-queue": "A quit, a crash and a restart", the paragraph
//! "A quit").
//!
//! A person who quits the app, and Homebrew that quits it for an upgrade, start the same
//! procedure. The app stops admitting, it cancels each start and each hold that has no result yet
//! as a closed connection does, it waits up to [`QUIT_WAIT`] for their cleanup, and then it exits.
//!
//! Each start and each hold registers before it begins, and it holds the guard until its thread
//! ends, so the wait covers the cleanup. A start that the limit leaves unfinished never reaches
//! the removal of its start record, and the next start of the app decides for the record.

use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use tokio_util::sync::CancellationToken;

/// How long a quit waits for the cleanup of the starts that it cancelled.
pub const QUIT_WAIT: Duration = Duration::from_secs(10);

#[derive(Debug, Default)]
struct State {
    /// The registered starts and holds whose threads have not ended.
    running: usize,
    /// When the quit began. Set once.
    begun: Option<Instant>,
}

/// What a quit left behind.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Every start and hold ended.
    Clean,
    /// This many starts and holds were still cleaning up at the limit.
    Unfinished(usize),
}

/// The quit of one run of the app.
#[derive(Debug)]
pub struct Quit {
    /// Cancelled when the quit begins. Each start and each hold has a child of it.
    token: CancellationToken,
    state: Mutex<State>,
    ended: Condvar,
    limit: Duration,
}

/// The right of one start or one hold to run. Dropping it, when the thread ends, counts the start
/// as ended.
#[derive(Debug)]
pub struct QuitGuard {
    quit: Arc<Quit>,
}

impl Quit {
    pub fn new(limit: Duration) -> Arc<Self> {
        Arc::new(Self {
            token: CancellationToken::new(),
            state: Mutex::new(State::default()),
            ended: Condvar::new(),
            limit,
        })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Whether a quit has begun. Then the app admits nothing.
    pub fn is_quitting(&self) -> bool {
        self.lock().begun.is_some()
    }

    /// Register a start or a hold, and give it the token that the client's connection and the quit
    /// both cancel. Returns `None` once the quit has begun: the app admits nothing then.
    pub fn enter(self: &Arc<Self>) -> Option<(QuitGuard, CancellationToken)> {
        let mut state = self.lock();
        if state.begun.is_some() {
            return None;
        }
        state.running += 1;
        Some((
            QuitGuard {
                quit: Arc::clone(self),
            },
            self.token.child_token(),
        ))
    }

    /// Begin the quit, cancel every start and hold, and wait for them to end. The wait ends at the
    /// limit, counted from the first call, so a second call (the signal that follows the menu, for
    /// example) does not wait longer.
    pub fn finish(&self) -> Outcome {
        let deadline = {
            let mut state = self.lock();
            let begun = *state.begun.get_or_insert_with(Instant::now);
            begun + self.limit
        };
        self.token.cancel();
        let mut state = self.lock();
        loop {
            if state.running == 0 {
                return Outcome::Clean;
            }
            let Some(left) = deadline.checked_duration_since(Instant::now()) else {
                return Outcome::Unfinished(state.running);
            };
            state = self
                .ended
                .wait_timeout(state, left)
                .unwrap_or_else(|p| p.into_inner())
                .0;
        }
    }
}

impl Quit {
    /// [`finish`](Self::finish), and a line on stderr for the starts that the limit left
    /// unfinished.
    pub fn finish_and_say(&self) {
        if let Outcome::Unfinished(count) = self.finish() {
            eprintln!(
                "testanyware-app: {count} start(s) did not finish cleaning up in time. Each keeps \
                 its start record, and the next start of the app decides for it."
            );
        }
    }
}

impl Drop for QuitGuard {
    fn drop(&mut self) {
        let mut state = self.quit.lock();
        state.running -= 1;
        self.quit.ended.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_quit_with_nothing_running_is_clean_at_once() {
        let quit = Quit::new(Duration::from_secs(5));
        let began = Instant::now();
        assert_eq!(quit.finish(), Outcome::Clean);
        assert!(began.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn a_quit_cancels_every_token_and_waits_for_the_guards() {
        let quit = Quit::new(Duration::from_secs(5));
        let (first, first_token) = quit.enter().unwrap();
        let (second, second_token) = quit.enter().unwrap();
        let waiting = {
            let quit = Arc::clone(&quit);
            std::thread::spawn(move || {
                let began = Instant::now();
                (quit.finish(), began.elapsed())
            })
        };
        // The cancel reaches both tokens, and the quit still waits for the cleanup.
        while !(first_token.is_cancelled() && second_token.is_cancelled()) {
            std::thread::sleep(Duration::from_millis(10));
        }
        std::thread::sleep(Duration::from_millis(200));
        assert!(!waiting.is_finished(), "the quit must wait for the cleanup");
        drop(first);
        std::thread::sleep(Duration::from_millis(100));
        assert!(!waiting.is_finished(), "one start still cleans up");
        drop(second);
        let (outcome, waited) = waiting.join().unwrap();
        assert_eq!(outcome, Outcome::Clean);
        assert!(waited >= Duration::from_millis(300), "{waited:?}");
    }

    #[test]
    fn a_cleanup_that_outlasts_the_limit_leaves_the_quit_at_the_limit() {
        let quit = Quit::new(Duration::from_millis(300));
        let (_guard, token) = quit.enter().unwrap();
        let began = Instant::now();
        assert_eq!(quit.finish(), Outcome::Unfinished(1));
        let waited = began.elapsed();
        assert!(token.is_cancelled());
        assert!(
            waited >= Duration::from_millis(300) && waited < Duration::from_secs(3),
            "{waited:?}"
        );
    }

    #[test]
    fn nothing_enters_once_the_quit_has_begun() {
        let quit = Quit::new(Duration::from_secs(5));
        assert!(!quit.is_quitting());
        assert_eq!(quit.finish(), Outcome::Clean);
        assert!(quit.is_quitting());
        assert!(quit.enter().is_none());
    }

    #[test]
    fn a_second_call_counts_the_limit_from_the_first() {
        let quit = Quit::new(Duration::from_millis(400));
        let (_guard, _token) = quit.enter().unwrap();
        assert_eq!(quit.finish(), Outcome::Unfinished(1));
        let again = Instant::now();
        assert_eq!(quit.finish(), Outcome::Unfinished(1));
        assert!(
            again.elapsed() < Duration::from_millis(100),
            "the limit has run out: {:?}",
            again.elapsed()
        );
    }

    #[test]
    fn the_cancel_of_one_client_cancels_no_other_start() {
        let quit = Quit::new(Duration::from_secs(5));
        let (_a, a) = quit.enter().unwrap();
        let (_b, b) = quit.enter().unwrap();
        a.cancel();
        assert!(a.is_cancelled() && !b.is_cancelled());
    }
}
