//! The admission queue of the app (spec "admission-queue": "The order", "The admission
//! decision" and "A cancel").
//!
//! A `vm start` of a macOS guest joins the queue when its clone is ready, and the app serves the
//! waiters in the order of their join while its own count limits the starts. The queue lives in
//! memory only. Each waiter is the thread of a start, which holds the connection of its client,
//! so a quit of the app or a restart of the Mac leaves no waiter behind.
//!
//! One lock guards the queue, the admitted starts and the verdicts. Each change takes it: a join, a
//! leave, an admission, an expiry and a release. Each waiting thread decides in its turn, through
//! [`decision::decide`], and it reads the verdict that a decision left for it. So no thread of the
//! queue exists beside the threads of the starts, and a start that nobody serves leaves no state.
//!
//! This increment counts the own holders only: the macOS guests that have a meta, and the starts
//! that the app admitted. The seen holders of `tart list`, the pause after a refusal and the
//! contention come later, and the retry of today keeps handling a refusal inside the hold.

pub mod count;
pub mod decision;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use testanyware_vm::error::{AdmissionFacts, LimitReached, RefusedBy};
use testanyware_vm::identity::SystemProbe;
use testanyware_vm::slot_gate::{SlotAsk, SlotGate, SlotHold};
use testanyware_vm::{Progress, VmError, VmPaths};
use tokio_util::sync::CancellationToken;

use decision::{admits_without_wait, decide, Input, Waiter};

/// The number of the two slots that the app may hold. `config.rhai` lowers it in a later
/// increment (`queue-slots-config`).
pub const MACOS_SLOTS: usize = 2;

/// The app decides again at least this often while the queue holds a waiter. A guest that ends by
/// itself raises no event, and the tick finds it.
const TICK: Duration = Duration::from_secs(3);

/// A waiting start repeats its line on stderr this often when nothing has changed.
const REPORT_EVERY: Duration = Duration::from_secs(30);

/// How often a waiting thread reads the cancel of its client. The token cannot wake a condition
/// variable, so the wait is bounded.
const CANCEL_POLL: Duration = Duration::from_millis(100);

/// The ids of the macOS guests that hold a slot because they have a meta.
pub type OwnGuests = Box<dyn Fn() -> BTreeSet<String> + Send + Sync>;

/// The ordered queue for the macOS guest slots of the user who runs the app.
pub struct AdmissionQueue {
    state: Mutex<State>,
    changed: Condvar,
    /// The clock of the queue starts here. Every time in the queue is the time since this moment.
    epoch: Instant,
    slots: usize,
    guests: OwnGuests,
    /// The seed of the request ids of this run of the app.
    salt: u32,
}

impl std::fmt::Debug for AdmissionQueue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AdmissionQueue")
            .field("slots", &self.slots)
            .finish_non_exhaustive()
    }
}

#[derive(Default)]
struct State {
    /// The waiters in the order of their join.
    waiting: Vec<Entry>,
    /// The starts that the app admitted and whose hold has not ended, by VM id. A guest that the
    /// meta holder group also names counts once.
    admitted: BTreeMap<String, String>,
    /// What a decision left for a waiter that is not yet back to read it, by request id.
    verdicts: HashMap<String, Verdict>,
    /// The holders at the last decision, for the lines of the waiters.
    holders: Vec<String>,
    next_seq: u64,
    /// The number of ids made so far.
    counter: u32,
    /// Raised by each event, so that a waiting thread knows that it must decide again.
    version: u64,
    last_decision: Duration,
}

struct Entry {
    request_id: String,
    vm_id: String,
    seq: u64,
    /// `None` for a waiter without a bound. A start with `--wait` always has one.
    deadline: Option<Duration>,
}

enum Verdict {
    Admitted,
    /// The place the waiter held when its deadline came.
    Expired {
        place: usize,
    },
}

impl AdmissionQueue {
    pub fn new(slots: usize, guests: OwnGuests) -> Arc<Self> {
        let epoch = Instant::now();
        Arc::new(Self {
            state: Mutex::new(State::default()),
            changed: Condvar::new(),
            epoch,
            slots,
            guests,
            salt: salt(),
        })
    }

    /// The queue of an app that keeps its records in `paths`.
    pub fn for_paths(paths: VmPaths) -> Arc<Self> {
        Self::new(
            MACOS_SLOTS,
            Box::new(move || count::macos_guests_with_a_meta(&paths, &SystemProbe)),
        )
    }

    /// A guest has ended, so a slot may be free. The queue decides again.
    pub fn holder_ended(&self) {
        let mut state = self.lock();
        state.version += 1;
        self.changed.notify_all();
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn now(&self) -> Duration {
        self.epoch.elapsed()
    }

    /// The own holders now: the guests with a meta, and the starts that the app admitted. A guest
    /// that two of them name counts once.
    fn own_holders(&self, state: &State) -> BTreeSet<String> {
        let mut own = (self.guests)();
        own.extend(state.admitted.keys().cloned());
        own
    }

    /// Decide, and leave each verdict for its waiter. The lock is held, so the decision and its
    /// effects form one step.
    fn run_decision(&self, state: &mut State) {
        let now = self.now();
        let own = self.own_holders(state);
        let queue: Vec<Waiter> = state
            .waiting
            .iter()
            .map(|e| Waiter {
                request_id: e.request_id.clone(),
                deadline: e.deadline,
            })
            .collect();
        let decision = decide(Input {
            queue: &queue,
            own_holders: own.len(),
            slots: self.slots,
            now,
        });
        state.holders = own.into_iter().collect();
        state.last_decision = now;
        let acted = !decision.admit.is_empty() || !decision.expire.is_empty();
        for request_id in decision.admit {
            if let Some(at) = state
                .waiting
                .iter()
                .position(|e| e.request_id == request_id)
            {
                let entry = state.waiting.remove(at);
                state.admitted.insert(entry.vm_id, request_id.clone());
                state.verdicts.insert(request_id, Verdict::Admitted);
            }
        }
        for request_id in decision.expire {
            if let Some(at) = state
                .waiting
                .iter()
                .position(|e| e.request_id == request_id)
            {
                state.waiting.remove(at);
                state
                    .verdicts
                    .insert(request_id, Verdict::Expired { place: at + 1 });
            }
        }
        if acted {
            state.version += 1;
            self.changed.notify_all();
        }
    }

    /// Join the queue, and wait for the admission. See [`SlotGate::admit`].
    pub fn admit(
        self: &Arc<Self>,
        ask: &SlotAsk<'_>,
        cancel: &CancellationToken,
        progress: &Progress,
    ) -> Result<Box<dyn SlotHold>, VmError> {
        let wait = ask.wait.filter(|wait| !wait.is_zero());
        let joined = self.now();
        let mut state = self.lock();
        let request_id = new_request_id(&mut state, self.salt);
        let seq = state.next_seq;
        state.next_seq += 1;

        let Some(wait) = wait else {
            return self.admit_without_wait(&mut state, ask, request_id, seq);
        };

        state.waiting.push(Entry {
            request_id: request_id.clone(),
            vm_id: ask.vm_id.to_string(),
            seq,
            deadline: Some(joined + wait),
        });
        // The join is an event, and the decision of it may admit the start at once.
        state.version += 1;
        self.run_decision(&mut state);

        let mut reporter = WaitReporter::default();
        let mut seen = state.version;
        loop {
            if let Some(verdict) = state.verdicts.remove(&request_id) {
                let waited = self.now().saturating_sub(joined);
                return match verdict {
                    Verdict::Admitted => Ok(Box::new(Hold {
                        queue: Arc::clone(self),
                        vm_id: ask.vm_id.to_string(),
                        request_id,
                        seq,
                        waited,
                    })),
                    Verdict::Expired { place } => {
                        Err(self.refusal(&state, request_id, place, waited))
                    }
                };
            }
            // A cancel that comes after the admission found the verdict above, so the start
            // runs its own cleanup and drops its hold.
            if cancel.is_cancelled() {
                if let Some(at) = state
                    .waiting
                    .iter()
                    .position(|e| e.request_id == request_id)
                {
                    state.waiting.remove(at);
                }
                state.version += 1;
                self.changed.notify_all();
                return Err(VmError::StartCancelled {
                    id: ask.vm_id.to_string(),
                });
            }
            if let Some(line) = reporter.note(&state, &request_id, self.now()) {
                progress.line(&line);
            }
            let deadline = state
                .waiting
                .iter()
                .find(|e| e.request_id == request_id)
                .and_then(|e| e.deadline);
            let now = self.now();
            let step = deadline
                .map_or(CANCEL_POLL, |d| d.saturating_sub(now))
                .min(CANCEL_POLL);
            state = self
                .changed
                .wait_timeout(state, step)
                .unwrap_or_else(|p| p.into_inner())
                .0;
            let now = self.now();
            let due = state.version != seen
                || now.saturating_sub(state.last_decision) >= TICK
                || deadline.is_some_and(|d| now >= d);
            if due {
                self.run_decision(&mut state);
                seen = state.version;
            }
        }
    }

    /// The one decision of a start without `--wait`, or with `--wait=0`. It never waits and it
    /// never passes a waiter, and it takes no place in the queue.
    fn admit_without_wait(
        self: &Arc<Self>,
        state: &mut State,
        ask: &SlotAsk<'_>,
        request_id: String,
        seq: u64,
    ) -> Result<Box<dyn SlotHold>, VmError> {
        let own = self.own_holders(state);
        state.holders = own.iter().cloned().collect();
        if admits_without_wait(state.waiting.len(), own.len(), self.slots) {
            state
                .admitted
                .insert(ask.vm_id.to_string(), request_id.clone());
            state.version += 1;
            return Ok(Box::new(Hold {
                queue: Arc::clone(self),
                vm_id: ask.vm_id.to_string(),
                request_id,
                seq,
                waited: Duration::ZERO,
            }));
        }
        // The place that the start would take.
        let place = state.waiting.len() + 1;
        Err(self.refusal(state, request_id, place, Duration::ZERO))
    }

    /// `VM_LIMIT_REACHED`, refused by the app. No `tart run` took part.
    fn refusal(
        &self,
        state: &State,
        request_id: String,
        place: usize,
        waited: Duration,
    ) -> VmError {
        VmError::VmLimitReached(Box::new(LimitReached {
            platform: "macos".into(),
            limit: u32::try_from(self.slots).unwrap_or(u32::MAX),
            holders: state.holders.clone(),
            raw: String::new(),
            admission: AdmissionFacts {
                request_id: Some(request_id),
                place: Some(u32::try_from(place).unwrap_or(u32::MAX)),
                waited_secs: waited.as_secs(),
                refused_by: RefusedBy::App,
            },
        }))
    }

    /// The hold of an admitted start ends. Its slot is free in the count, unless the guest now
    /// has a meta, which holds the slot from then on.
    fn release(&self, vm_id: &str, request_id: &str) {
        let mut state = self.lock();
        if state
            .admitted
            .get(vm_id)
            .is_some_and(|held| held == request_id)
        {
            state.admitted.remove(vm_id);
        }
        state.version += 1;
        self.changed.notify_all();
    }

    fn place_if_requeued(&self, seq: u64) -> u32 {
        let state = self.lock();
        let ahead = state.waiting.iter().filter(|e| e.seq < seq).count();
        u32::try_from(ahead + 1).unwrap_or(u32::MAX)
    }

    /// The waiters now, in order, as the request id and the VM id. A test reads this.
    pub fn waiting(&self) -> Vec<(String, String)> {
        self.lock()
            .waiting
            .iter()
            .map(|e| (e.request_id.clone(), e.vm_id.clone()))
            .collect()
    }

    /// The VM ids of the starts that the app admitted and that still hold. A test reads this.
    pub fn admitted(&self) -> Vec<String> {
        self.lock().admitted.keys().cloned().collect()
    }
}

/// The hold of one admitted start. Dropping it releases the slot.
struct Hold {
    queue: Arc<AdmissionQueue>,
    vm_id: String,
    request_id: String,
    seq: u64,
    waited: Duration,
}

impl SlotHold for Hold {
    fn place_if_requeued(&self) -> u32 {
        self.queue.place_if_requeued(self.seq)
    }

    fn request_id(&self) -> &str {
        &self.request_id
    }

    fn waited(&self) -> Duration {
        self.waited
    }
}

impl Drop for Hold {
    fn drop(&mut self) {
        self.queue.release(&self.vm_id, &self.request_id);
    }
}

/// The gate that a start of the app passes.
#[derive(Debug, Clone)]
pub struct AppGate(pub Arc<AdmissionQueue>);

impl SlotGate for AppGate {
    fn admit(
        &self,
        ask: &SlotAsk<'_>,
        cancel: &CancellationToken,
        progress: &Progress,
    ) -> Result<Box<dyn SlotHold>, VmError> {
        self.0.admit(ask, cancel, progress)
    }
}

/// The seed of the ids of one run of the app: its pid and the time of its start.
fn salt() -> u32 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos() ^ (d.as_secs() as u32));
    nanos.rotate_left(13) ^ std::process::id().wrapping_mul(0x85eb_ca6b)
}

/// The next request id, such as `r-1a2b3c4d`. An odd multiplier maps the counter one to one onto
/// the 32 bits, so the app reuses no id until it has made 2^32 of them.
fn new_request_id(state: &mut State, salt: u32) -> String {
    let n = state.counter;
    state.counter = state.counter.wrapping_add(1);
    format!("r-{:08x}", salt.wrapping_add(n.wrapping_mul(0x9e37_79b1)))
}

/// Decides when a waiting start speaks on stderr: when it joins, when its place or the holders
/// change, and every 30 s otherwise.
#[derive(Default)]
struct WaitReporter {
    last: Option<(usize, Vec<String>, Duration)>,
}

impl WaitReporter {
    fn note(&mut self, state: &State, request_id: &str, now: Duration) -> Option<String> {
        let place = state
            .waiting
            .iter()
            .position(|e| e.request_id == request_id)?
            + 1;
        let due = match &self.last {
            None => true,
            Some((last_place, last_holders, at)) => {
                *last_place != place
                    || *last_holders != state.holders
                    || now.saturating_sub(*at) >= REPORT_EVERY
            }
        };
        if !due {
            return None;
        }
        self.last = Some((place, state.holders.clone(), now));
        let held_by = if state.holders.is_empty() {
            "no VM".to_string()
        } else {
            state.holders.join(", ")
        };
        let waiters = state.waiting.len();
        Some(format!(
            "waiting for a macOS guest slot: request {request_id}, place {place}, {waiters} {}, slots held by {held_by}",
            if waiters == 1 { "waiter" } else { "waiters" }
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// A queue whose meta holders the test sets.
    fn queue(slots: usize) -> (Arc<AdmissionQueue>, Arc<Mutex<BTreeSet<String>>>) {
        let guests = Arc::new(Mutex::new(BTreeSet::new()));
        let read = Arc::clone(&guests);
        let queue = AdmissionQueue::new(slots, Box::new(move || read.lock().unwrap().clone()));
        (queue, guests)
    }

    fn quiet() -> Progress {
        Progress::to(|_| {})
    }

    fn ask(vm_id: &str, wait_secs: Option<u64>) -> SlotAsk<'_> {
        SlotAsk {
            vm_id,
            wait: wait_secs.map(Duration::from_secs),
        }
    }

    fn limit_of(error: VmError) -> LimitReached {
        match error {
            VmError::VmLimitReached(limit) => *limit,
            other => panic!("not a limit refusal: {other}"),
        }
    }

    fn wait_until(what: &str, mut ready: impl FnMut() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !ready() {
            assert!(Instant::now() < deadline, "timed out waiting for {what}");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn request_ids_have_the_shape_and_are_not_reused() {
        let mut state = State::default();
        let mut seen = std::collections::HashSet::new();
        for _ in 0..20_000 {
            let id = new_request_id(&mut state, 0xdead_beef);
            assert!(id.starts_with("r-") && id.len() == 10, "{id}");
            assert!(id[2..].chars().all(|c| c.is_ascii_hexdigit()), "{id}");
            assert!(seen.insert(id.clone()), "{id} came twice");
        }
    }

    #[test]
    fn a_free_slot_admits_a_start_at_once_and_the_hold_counts() {
        let (queue, _) = queue(2);
        let cancel = CancellationToken::new();
        let a = queue.admit(&ask("a", Some(5)), &cancel, &quiet()).unwrap();
        let b = queue.admit(&ask("b", Some(5)), &cancel, &quiet()).unwrap();
        assert_ne!(a.request_id(), b.request_id());
        assert_eq!(queue.admitted(), ["a", "b"]);
        drop(a);
        assert_eq!(queue.admitted(), ["b"]);
    }

    #[test]
    fn the_third_start_waits_and_gets_the_slot_after_a_release() {
        let (queue, _) = queue(2);
        let cancel = CancellationToken::new();
        let a = queue.admit(&ask("a", Some(5)), &cancel, &quiet()).unwrap();
        let _b = queue.admit(&ask("b", Some(5)), &cancel, &quiet()).unwrap();

        let lines = Arc::new(Mutex::new(Vec::<String>::new()));
        let third = {
            let queue = Arc::clone(&queue);
            let lines = Arc::clone(&lines);
            std::thread::spawn(move || {
                let progress =
                    Progress::to(move |line| lines.lock().unwrap().push(line.to_string()));
                queue.admit(&ask("c", Some(30)), &CancellationToken::new(), &progress)
            })
        };
        wait_until("the third start to join", || queue.waiting().len() == 1);
        assert!(!third.is_finished());
        drop(a);
        // The test keeps the hold, as a start does while its guest boots.
        let hold = third
            .join()
            .unwrap()
            .expect("the release admits the waiter");
        assert!(hold.request_id().starts_with("r-"));
        assert_eq!(queue.admitted(), ["b", "c"]);
        let lines = lines.lock().unwrap();
        assert!(
            !lines.is_empty(),
            "a waiting start writes a line when it joins"
        );
        assert!(lines[0].contains("place 1"), "{lines:?}");
        assert!(lines[0].contains("1 waiter"), "{lines:?}");
        assert!(lines[0].contains("a, b"), "{lines:?}");
    }

    #[test]
    fn waiters_are_admitted_in_the_order_of_their_join() {
        let (queue, _) = queue(1);
        let first = queue
            .admit(&ask("holder", Some(5)), &CancellationToken::new(), &quiet())
            .unwrap();
        let order = Arc::new(Mutex::new(Vec::<String>::new()));
        let mut threads = Vec::new();
        for name in ["w1", "w2", "w3"] {
            let queue2 = Arc::clone(&queue);
            let order = Arc::clone(&order);
            let want = threads.len() + 1;
            threads.push(std::thread::spawn(move || {
                let hold = queue2
                    .admit(&ask(name, Some(60)), &CancellationToken::new(), &quiet())
                    .unwrap();
                order.lock().unwrap().push(name.to_string());
                // Hold the slot a moment, then release it for the next waiter.
                std::thread::sleep(Duration::from_millis(150));
                drop(hold);
            }));
            // The joins are in order because each thread waits for the previous join.
            wait_until("a join", || queue.waiting().len() == want);
        }
        drop(first);
        for thread in threads {
            thread.join().unwrap();
        }
        assert_eq!(*order.lock().unwrap(), ["w1", "w2", "w3"]);
    }

    #[test]
    fn a_meta_holder_counts_and_its_end_admits_the_waiter() {
        let (queue, guests) = queue(2);
        guests
            .lock()
            .unwrap()
            .extend(["g1".to_string(), "g2".to_string()]);
        let waiter = {
            let queue = Arc::clone(&queue);
            std::thread::spawn(move || {
                queue
                    .admit(&ask("c", Some(30)), &CancellationToken::new(), &quiet())
                    .is_ok()
            })
        };
        wait_until("the start to join", || queue.waiting().len() == 1);
        std::thread::sleep(Duration::from_millis(300));
        assert!(!waiter.is_finished(), "two guests hold both slots");
        // The stop of a guest raises an event, as `vm stop` does.
        guests.lock().unwrap().remove("g1");
        queue.holder_ended();
        assert!(waiter.join().unwrap());
    }

    #[test]
    fn a_guest_that_ends_by_itself_is_found_by_the_tick() {
        let (queue, guests) = queue(1);
        guests.lock().unwrap().insert("g1".to_string());
        let waiter = {
            let queue = Arc::clone(&queue);
            std::thread::spawn(move || {
                queue
                    .admit(&ask("c", Some(30)), &CancellationToken::new(), &quiet())
                    .is_ok()
            })
        };
        wait_until("the start to join", || queue.waiting().len() == 1);
        // No event: the guest just goes away.
        guests.lock().unwrap().clear();
        assert!(waiter.join().unwrap(), "the tick of 3 s admits the waiter");
    }

    #[test]
    fn a_guest_that_the_start_and_the_meta_both_name_counts_once() {
        let (queue, guests) = queue(2);
        let cancel = CancellationToken::new();
        let hold = queue.admit(&ask("a", Some(5)), &cancel, &quiet()).unwrap();
        // The start wrote its meta and has not dropped its hold yet.
        guests.lock().unwrap().insert("a".to_string());
        let b = queue.admit(&ask("b", Some(5)), &cancel, &quiet());
        assert!(
            b.is_ok(),
            "one guest holds one slot, however many holders name it"
        );
        drop(hold);
        // The meta holds the slot from now on.
        let refused = queue.admit(&ask("c", None), &cancel, &quiet());
        assert!(refused.is_err());
    }

    #[test]
    fn a_wait_that_runs_out_fails_with_the_facts_of_the_wait() {
        let (queue, guests) = queue(2);
        guests
            .lock()
            .unwrap()
            .extend(["g1".to_string(), "g2".to_string()]);
        let began = Instant::now();
        let error = queue
            .admit(&ask("late", Some(1)), &CancellationToken::new(), &quiet())
            .err()
            .expect("the wait runs out");
        assert!(began.elapsed() >= Duration::from_secs(1));
        let limit = limit_of(error);
        assert_eq!(limit.admission.refused_by, RefusedBy::App);
        assert_eq!(limit.admission.place, Some(1));
        assert!(limit.admission.waited_secs >= 1);
        assert!(limit.admission.request_id.unwrap().starts_with("r-"));
        assert_eq!(limit.limit, 2);
        assert_eq!(limit.holders, ["g1", "g2"]);
        assert_eq!(limit.raw, "", "no tart run took part");
        assert!(
            queue.waiting().is_empty(),
            "the failed start left the queue"
        );
        assert!(queue.admitted().is_empty(), "and holds no slot");
    }

    #[test]
    fn the_last_try_at_the_deadline_admits_a_start_whose_slot_freed_in_the_last_moment() {
        let (queue, guests) = queue(1);
        guests.lock().unwrap().insert("g1".to_string());
        let waiter = {
            let queue = Arc::clone(&queue);
            std::thread::spawn(move || {
                queue
                    .admit(&ask("c", Some(1)), &CancellationToken::new(), &quiet())
                    .is_ok()
            })
        };
        wait_until("the start to join", || queue.waiting().len() == 1);
        // The guest goes at once. No event is raised, and the tick is 3 s away. Only the
        // decision at the deadline can find the free slot.
        guests.lock().unwrap().clear();
        assert!(
            waiter.join().unwrap(),
            "the decision at the deadline admits before it expires"
        );
    }

    #[test]
    fn a_start_without_a_wait_fails_at_once_when_a_waiter_stands_ahead() {
        let (queue, guests) = queue(2);
        guests
            .lock()
            .unwrap()
            .extend(["g1".to_string(), "g2".to_string()]);
        let cancel = CancellationToken::new();
        let waiter = {
            let queue = Arc::clone(&queue);
            let cancel = cancel.clone();
            std::thread::spawn(move || queue.admit(&ask("w", Some(30)), &cancel, &quiet()).is_ok())
        };
        wait_until("the waiter to join", || queue.waiting().len() == 1);
        let began = Instant::now();
        let error = queue
            .admit(&ask("x", None), &CancellationToken::new(), &quiet())
            .err()
            .expect("refused at once");
        assert!(began.elapsed() < Duration::from_millis(500));
        let limit = limit_of(error);
        assert_eq!(
            limit.admission.place,
            Some(2),
            "the place that the start would take"
        );
        assert_eq!(limit.admission.refused_by, RefusedBy::App);
        assert_eq!(queue.waiting().len(), 1, "it took no place in the queue");
        cancel.cancel();
        assert!(!waiter.join().unwrap());
    }

    #[test]
    fn a_start_with_wait_zero_is_decided_like_a_start_without_a_wait() {
        let (queue, guests) = queue(2);
        let cancel = CancellationToken::new();
        // A free slot and no waiter: admitted.
        let hold = queue.admit(&ask("a", Some(0)), &cancel, &quiet());
        assert!(hold.is_ok());
        // Both slots held: refused at once, with the place 1.
        guests.lock().unwrap().insert("g1".to_string());
        let error = queue
            .admit(&ask("b", Some(0)), &cancel, &quiet())
            .err()
            .unwrap();
        assert_eq!(limit_of(error).admission.place, Some(1));
    }

    #[test]
    fn a_cancel_while_the_start_waits_leaves_the_queue_and_holds_no_slot() {
        let (queue, guests) = queue(1);
        guests.lock().unwrap().insert("g1".to_string());
        let cancel = CancellationToken::new();
        let waiter = {
            let queue = Arc::clone(&queue);
            let cancel = cancel.clone();
            std::thread::spawn(move || queue.admit(&ask("w", Some(30)), &cancel, &quiet()))
        };
        wait_until("the start to join", || queue.waiting().len() == 1);
        cancel.cancel();
        let outcome = waiter.join().unwrap();
        assert!(matches!(outcome, Err(VmError::StartCancelled { .. })));
        assert!(queue.waiting().is_empty());
        assert!(queue.admitted().is_empty());
    }

    #[test]
    fn a_cancel_moves_the_places_behind_it_up() {
        let (queue, guests) = queue(1);
        guests.lock().unwrap().insert("g1".to_string());
        let cancel_first = CancellationToken::new();
        let first = {
            let queue = Arc::clone(&queue);
            let cancel = cancel_first.clone();
            std::thread::spawn(move || queue.admit(&ask("w1", Some(30)), &cancel, &quiet()).is_ok())
        };
        wait_until("the first join", || queue.waiting().len() == 1);
        let lines = Arc::new(Mutex::new(Vec::<String>::new()));
        let second = {
            let queue = Arc::clone(&queue);
            let lines = Arc::clone(&lines);
            std::thread::spawn(move || {
                let progress =
                    Progress::to(move |line| lines.lock().unwrap().push(line.to_string()));
                queue
                    .admit(&ask("w2", Some(30)), &CancellationToken::new(), &progress)
                    .is_ok()
            })
        };
        wait_until("the second join", || queue.waiting().len() == 2);
        cancel_first.cancel();
        assert!(!first.join().unwrap());
        guests.lock().unwrap().clear();
        queue.holder_ended();
        assert!(second.join().unwrap());
        let lines = lines.lock().unwrap();
        assert!(lines.iter().any(|l| l.contains("place 2")), "{lines:?}");
        assert!(
            lines.iter().any(|l| l.contains("place 1")),
            "the place moved up: {lines:?}"
        );
    }

    #[test]
    fn a_cancel_after_the_admission_is_the_start_s_to_clean_up_and_the_hold_releases_the_slot() {
        let (queue, _) = queue(1);
        let cancel = CancellationToken::new();
        let hold = queue.admit(&ask("a", Some(5)), &cancel, &quiet()).unwrap();
        cancel.cancel();
        // The admitted start owns the release, and nothing else frees the slot.
        assert_eq!(queue.admitted(), ["a"]);
        drop(hold);
        assert!(queue.admitted().is_empty());
    }

    #[test]
    fn a_cancel_at_the_admission_leaves_one_of_the_two_outcomes_and_never_a_leaked_slot() {
        // The cancel and the end of a holder come together. The lock orders them, so the start
        // either leaves the queue with no hold, or it holds the slot and its own cleanup
        // releases it. No order leaves a waiter in the queue or a slot held without a start.
        for round in 0..40 {
            let (queue, guests) = queue(1);
            guests.lock().unwrap().insert("g1".to_string());
            let cancel = CancellationToken::new();
            let waiter = {
                let queue = Arc::clone(&queue);
                let cancel = cancel.clone();
                std::thread::spawn(move || queue.admit(&ask("w", Some(30)), &cancel, &quiet()))
            };
            wait_until("the start to join", || queue.waiting().len() == 1);
            let freeing = {
                let queue = Arc::clone(&queue);
                let guests = Arc::clone(&guests);
                std::thread::spawn(move || {
                    guests.lock().unwrap().clear();
                    queue.holder_ended();
                })
            };
            if round % 2 == 0 {
                std::thread::sleep(Duration::from_micros(200));
            }
            cancel.cancel();
            freeing.join().unwrap();
            match waiter.join().unwrap() {
                Ok(hold) => {
                    assert_eq!(queue.admitted(), ["w"], "round {round}");
                    drop(hold);
                }
                Err(VmError::StartCancelled { .. }) => {}
                Err(other) => panic!("round {round}: {other}"),
            }
            assert!(queue.waiting().is_empty(), "round {round}");
            assert!(queue.admitted().is_empty(), "round {round}");
        }
    }

    #[test]
    fn a_hold_says_where_the_start_would_stand_if_it_queued_again() {
        let (queue, guests) = queue(1);
        let hold = queue
            .admit(&ask("a", Some(5)), &CancellationToken::new(), &quiet())
            .unwrap();
        assert_eq!(hold.place_if_requeued(), 1, "nobody joined before it");
        guests.lock().unwrap().insert("g1".to_string());
        let count = Arc::new(AtomicUsize::new(0));
        let waiter = {
            let queue = Arc::clone(&queue);
            let count = Arc::clone(&count);
            std::thread::spawn(move || {
                let ok = queue
                    .admit(&ask("w", Some(30)), &CancellationToken::new(), &quiet())
                    .is_ok();
                count.fetch_add(1, Ordering::SeqCst);
                ok
            })
        };
        wait_until("the waiter to join", || queue.waiting().len() == 1);
        // The waiter joined after the hold, so the hold would go back ahead of it.
        assert_eq!(hold.place_if_requeued(), 1);
        drop(hold);
        guests.lock().unwrap().clear();
        queue.holder_ended();
        assert!(waiter.join().unwrap());
    }

    #[test]
    fn a_line_names_the_request_the_place_the_waiters_and_the_holders() {
        let mut state = State {
            holders: vec!["vm-a".into(), "vm-b".into()],
            ..State::default()
        };
        for (n, id) in ["r-1", "r-2"].iter().enumerate() {
            state.waiting.push(Entry {
                request_id: id.to_string(),
                vm_id: format!("w{n}"),
                seq: n as u64,
                deadline: None,
            });
        }
        let mut reporter = WaitReporter::default();
        let line = reporter.note(&state, "r-2", Duration::ZERO).unwrap();
        assert_eq!(
            line,
            "waiting for a macOS guest slot: request r-2, place 2, 2 waiters, slots held by vm-a, vm-b"
        );
        // Nothing changed, so nothing is said until 30 s have passed.
        assert!(reporter
            .note(&state, "r-2", Duration::from_secs(29))
            .is_none());
        assert!(reporter
            .note(&state, "r-2", Duration::from_secs(30))
            .is_some());
        // A change of the holders is said at once.
        state.holders = vec!["vm-a".into()];
        assert!(reporter
            .note(&state, "r-2", Duration::from_secs(31))
            .is_some());
        // So is a change of the place.
        state.waiting.remove(0);
        let line = reporter
            .note(&state, "r-2", Duration::from_secs(32))
            .unwrap();
        assert!(
            line.contains("place 1") && line.contains("1 waiter,"),
            "{line}"
        );
    }
}
