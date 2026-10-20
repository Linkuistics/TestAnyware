//! One start per VM id at a time.
//!
//! A start reclaims a leftover of its id, and a cancelled start removes the VM of its id. A client
//! that gives up and tries again with the same `--id` would start while the first start still
//! cleans up, and the cleanup would remove the new clone. So a start waits until the earlier
//! start of its id has ended, its cleanup included. A start that is cancelled while it waits
//! never begins.

use std::collections::HashSet;
use std::sync::{Condvar, Mutex};
use std::time::Duration;

use tokio_util::sync::CancellationToken;

/// The ids that are being started now.
#[derive(Debug, Default)]
pub struct StartGate {
    busy: Mutex<HashSet<String>>,
    released: Condvar,
}

/// The right to start one id. Dropping it lets the next start of the id begin.
#[derive(Debug)]
pub struct StartPermit<'a> {
    gate: &'a StartGate,
    id: String,
}

impl StartGate {
    /// Wait until no start of `id` runs, and take the id. Returns `None` if `cancel` fires
    /// first. The thread of a start calls this, so it blocks that thread and no other.
    pub fn acquire(&self, id: &str, cancel: &CancellationToken) -> Option<StartPermit<'_>> {
        let mut busy = self.busy.lock().unwrap_or_else(|p| p.into_inner());
        loop {
            if cancel.is_cancelled() {
                return None;
            }
            if busy.insert(id.to_string()) {
                return Some(StartPermit { gate: self, id: id.to_string() });
            }
            // A bounded wait: the cancel token has no way to wake a condition variable.
            busy = self
                .released
                .wait_timeout(busy, Duration::from_millis(100))
                .unwrap_or_else(|p| p.into_inner())
                .0;
        }
    }
}

impl Drop for StartPermit<'_> {
    fn drop(&mut self) {
        self.gate
            .busy
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(&self.id);
        self.gate.released.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Instant;

    #[test]
    fn two_ids_start_together() {
        let gate = StartGate::default();
        let cancel = CancellationToken::new();
        let a = gate.acquire("a", &cancel);
        let b = gate.acquire("b", &cancel);
        assert!(a.is_some() && b.is_some());
    }

    #[test]
    fn a_second_start_of_an_id_waits_for_the_first_to_end() {
        let gate = Arc::new(StartGate::default());
        let cancel = CancellationToken::new();
        let first = gate.acquire("vm", &cancel).expect("the first start begins");

        let waiting = {
            let gate = Arc::clone(&gate);
            let cancel = cancel.clone();
            std::thread::spawn(move || {
                let began = Instant::now();
                let permit = gate.acquire("vm", &cancel);
                (permit.is_some(), began.elapsed())
            })
        };
        std::thread::sleep(Duration::from_millis(300));
        assert!(!waiting.is_finished(), "the second start must wait");
        drop(first);
        let (began, waited) = waiting.join().unwrap();
        assert!(began, "the second start begins once the first has ended");
        assert!(waited >= Duration::from_millis(300), "{waited:?}");
    }

    #[test]
    fn a_start_that_is_cancelled_while_it_waits_never_begins() {
        let gate = Arc::new(StartGate::default());
        let first = gate.acquire("vm", &CancellationToken::new()).unwrap();
        let cancel = CancellationToken::new();
        let waiting = {
            let gate = Arc::clone(&gate);
            let cancel = cancel.clone();
            std::thread::spawn(move || gate.acquire("vm", &cancel).is_some())
        };
        std::thread::sleep(Duration::from_millis(200));
        cancel.cancel();
        assert!(!waiting.join().unwrap(), "a cancelled wait yields no permit");
        // The cancelled wait took nothing: once the first start ends, the id is free.
        drop(first);
        assert!(gate.acquire("vm", &CancellationToken::new()).is_some());
    }
}
