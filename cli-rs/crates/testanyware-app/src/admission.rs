//! The admission queue of the app (spec "admission-queue": "The order", "The admission
//! decision", "A refusal by the framework" and "A cancel").
//!
//! A `vm start` of a macOS guest joins the queue when its clone is ready, and the app serves the
//! waiters in the order of their join while its own count limits the starts. The queue lives in
//! memory only. Each waiter is the thread of a start, which holds the connection of its client,
//! so a quit of the app or a restart of the Mac leaves no waiter behind.
//!
//! One lock guards the queue, the admitted starts and the verdicts. Each change takes it: a join, a
//! leave, an admission, a refusal, an acceptance, an expiry and a release. Each waiting thread
//! decides in its turn, through [`decision::decide`], and it reads the verdict that a decision left
//! for it. So no thread of the queue exists beside the threads of the starts, and a start that
//! nobody serves leaves no state.
//!
//! This increment counts the own holders only: the macOS guests that have a meta, and the starts
//! that the app admitted. A refusal by the framework sends the start back to the place of its
//! join, starts a pause of 3 s and starts contention. The seen holders of `tart list` come later.

pub mod count;
pub mod decision;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use testanyware_vm::error::{AdmissionFacts, LimitReached, RefusedBy};
use testanyware_vm::identity::SystemProbe;
use testanyware_vm::slot_gate::{Readmission, SlotAsk, SlotGate, SlotHold};
use testanyware_vm::{Progress, VmError, VmPaths};
use tokio_util::sync::CancellationToken;

use decision::{
    admits_without_wait, contention_after_acceptance, contention_after_settle, decide, Input,
    Waiter, PAUSE,
};

/// The slots that Apple allows for macOS guests on one Mac. The `limit` of a refusal names it
/// when the app's own share is not what ran out.
const APPLE_LIMIT: usize = 2;

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
    /// How long the queue admits nothing after a refusal by the framework.
    pause: Duration,
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
    admitted: BTreeMap<String, Admitted>,
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
    /// Whether the app is in contention (spec "A refusal by the framework").
    contention: bool,
    /// The time and the line of the last refusal by the framework.
    last_refusal: Option<Refusal>,
}

impl State {
    /// The admitted starts that the framework has neither accepted nor refused.
    fn unproven(&self) -> usize {
        self.admitted.values().filter(|a| !a.proven).count()
    }

    /// Contention ends when the queue holds no waiter and no admitted start is unproven.
    fn settle_contention(&mut self) {
        self.contention =
            contention_after_settle(self.contention, self.waiting.len(), self.unproven());
    }
}

struct Entry {
    request_id: String,
    vm_id: String,
    seq: u64,
    /// `None` for a waiter without a bound. A start with `--wait` always has one.
    deadline: Option<Duration>,
    /// The line of the refusal by the framework that sent this start back, when one did.
    refusal: Option<String>,
}

/// An admitted start whose hold has not ended.
struct Admitted {
    request_id: String,
    /// The framework accepted the start or refused it. An unproven start holds contention.
    proven: bool,
    /// The app admitted the start while it was in contention. Its acceptance ends contention.
    in_contention: bool,
}

struct Refusal {
    at: Duration,
    line: String,
}

enum Verdict {
    Admitted,
    Expired {
        /// The place the waiter held when its deadline came.
        place: usize,
        /// The line of the refusal that sent the waiter back, when one did.
        refusal: Option<String>,
    },
}

/// A request that waits for its admission.
struct Request {
    request_id: String,
    vm_id: String,
    joined: Duration,
}

impl AdmissionQueue {
    pub fn new(slots: usize, guests: OwnGuests) -> Arc<Self> {
        Self::with_pause(slots, guests, PAUSE)
    }

    /// [`new`](Self::new) with another pause after a refusal. A test shortens it.
    pub fn with_pause(slots: usize, guests: OwnGuests, pause: Duration) -> Arc<Self> {
        let epoch = Instant::now();
        Arc::new(Self {
            state: Mutex::new(State::default()),
            changed: Condvar::new(),
            epoch,
            slots,
            pause,
            guests,
            salt: salt(),
        })
    }

    /// The queue of an app that keeps its records in `paths` and holds `slots` of the two slots
    /// (`macos_slots` of `config.rhai`).
    pub fn for_paths(paths: VmPaths, slots: usize) -> Arc<Self> {
        Self::new(
            slots,
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
        state.settle_contention();
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
            unproven: state.unproven(),
            slots: self.slots,
            contention: state.contention,
            last_refusal: state.last_refusal.as_ref().map(|r| r.at),
            pause: self.pause,
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
                let in_contention = state.contention;
                state.admitted.insert(
                    entry.vm_id,
                    Admitted {
                        request_id: request_id.clone(),
                        proven: false,
                        in_contention,
                    },
                );
                state.verdicts.insert(request_id, Verdict::Admitted);
            }
        }
        for request_id in decision.expire {
            if let Some(at) = state
                .waiting
                .iter()
                .position(|e| e.request_id == request_id)
            {
                let entry = state.waiting.remove(at);
                state.verdicts.insert(
                    request_id,
                    Verdict::Expired {
                        place: at + 1,
                        refusal: entry.refusal,
                    },
                );
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
        // With no slot to hold, no slot can free while the app runs, because it reads the file
        // only when it starts. So a start never waits: it takes the one decision of a start
        // without `--wait`, which refuses it.
        let wait = ask
            .wait
            .filter(|wait| !wait.is_zero())
            .filter(|_| self.slots > 0);
        let joined = self.now();
        let mut state = self.lock();
        let request_id = new_request_id(&mut state, self.salt);
        let seq = state.next_seq;
        state.next_seq += 1;

        let Some(wait) = wait else {
            return self.admit_without_wait(&mut state, ask, request_id, seq);
        };

        let deadline = joined + wait;
        state.waiting.push(Entry {
            request_id: request_id.clone(),
            vm_id: ask.vm_id.to_string(),
            seq,
            deadline: Some(deadline),
            refusal: None,
        });
        // The join is an event, and the decision of it may admit the start at once.
        state.version += 1;
        self.run_decision(&mut state);

        let request = Request {
            request_id,
            vm_id: ask.vm_id.to_string(),
            joined,
        };
        let mut reporter = WaitReporter::default();
        self.wait_admitted(state, &request, &mut reporter, cancel, progress)?;
        Ok(Box::new(Hold {
            queue: Arc::clone(self),
            vm_id: request.vm_id,
            request_id: request.request_id,
            seq,
            joined,
            deadline: Some(deadline),
            waited: self.now().saturating_sub(joined),
            reporter,
            refusals: RefusalReporter::default(),
        }))
    }

    /// Wait until a decision admits the request or expires it, or until the client leaves. The
    /// request is in the queue when this starts. It is out of the queue when this returns.
    fn wait_admitted(
        &self,
        mut state: MutexGuard<'_, State>,
        request: &Request,
        reporter: &mut WaitReporter,
        cancel: &CancellationToken,
        progress: &Progress,
    ) -> Result<(), VmError> {
        let mut seen = state.version;
        loop {
            if let Some(verdict) = state.verdicts.remove(&request.request_id) {
                let waited = self.now().saturating_sub(request.joined);
                return match verdict {
                    Verdict::Admitted => Ok(()),
                    Verdict::Expired { place, refusal } => {
                        // A refusal of this start, or contention, makes a guest that the app
                        // cannot see a possible cause.
                        let unseen = refusal.is_some() || state.contention;
                        Err(self.refusal(
                            &state,
                            request.request_id.clone(),
                            place,
                            waited,
                            refusal.unwrap_or_default(),
                            unseen,
                        ))
                    }
                };
            }
            // A cancel that comes after the admission found the verdict above, so the start
            // runs its own cleanup and drops its hold.
            if cancel.is_cancelled() {
                if let Some(at) = state
                    .waiting
                    .iter()
                    .position(|e| e.request_id == request.request_id)
                {
                    state.waiting.remove(at);
                }
                state.version += 1;
                self.changed.notify_all();
                return Err(VmError::StartCancelled {
                    id: request.vm_id.clone(),
                });
            }
            if let Some(line) = reporter.note(&state, &request.request_id, self.now()) {
                progress.line(&line);
            }
            let deadline = state
                .waiting
                .iter()
                .find(|e| e.request_id == request.request_id)
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
            // The end of the pause is an event too: no other event marks it.
            let pause_ended = state.last_refusal.as_ref().is_some_and(|refusal| {
                let end = refusal.at + self.pause;
                now >= end && state.last_decision < end
            });
            let due = state.version != seen
                || now.saturating_sub(state.last_decision) >= TICK
                || deadline.is_some_and(|d| now >= d)
                || pause_ended;
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
        state.settle_contention();
        let joined = self.now();
        if admits_without_wait(
            state.waiting.len(),
            own.len(),
            self.slots,
            state.contention,
            state.unproven(),
        ) {
            state.admitted.insert(
                ask.vm_id.to_string(),
                Admitted {
                    request_id: request_id.clone(),
                    proven: false,
                    in_contention: state.contention,
                },
            );
            state.version += 1;
            return Ok(Box::new(Hold {
                queue: Arc::clone(self),
                vm_id: ask.vm_id.to_string(),
                request_id,
                seq,
                joined,
                deadline: None,
                waited: Duration::ZERO,
                reporter: WaitReporter::default(),
                refusals: RefusalReporter::default(),
            }));
        }
        // The place that the start would take.
        let place = state.waiting.len() + 1;
        let unseen = state.contention;
        Err(self.refusal(
            state,
            request_id,
            place,
            Duration::ZERO,
            String::new(),
            unseen,
        ))
    }

    /// The limit that refused: the app's share when the own holders fill it, and the limit of
    /// Apple otherwise (spec "The command surface").
    fn limit(&self, state: &State) -> usize {
        if state.holders.len() >= self.slots {
            self.slots
        } else {
            APPLE_LIMIT
        }
    }

    /// `VM_LIMIT_REACHED`, refused by the app. `raw` is the line of the refusal by the framework
    /// that sent the start back, or empty when no `tart run` of this start took part.
    fn refusal(
        &self,
        state: &State,
        request_id: String,
        place: usize,
        waited: Duration,
        raw: String,
        unseen_possible: bool,
    ) -> VmError {
        VmError::VmLimitReached(Box::new(LimitReached {
            platform: "macos".into(),
            limit: u32::try_from(self.limit(state)).unwrap_or(u32::MAX),
            holders: state.holders.clone(),
            raw,
            admission: AdmissionFacts {
                request_id: Some(request_id),
                place: Some(u32::try_from(place).unwrap_or(u32::MAX)),
                waited_secs: waited.as_secs(),
                refused_by: RefusedBy::App,
                unseen_possible,
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
            .is_some_and(|held| held.request_id == request_id)
        {
            state.admitted.remove(vm_id);
        }
        state.version += 1;
        self.changed.notify_all();
    }

    /// The framework accepted an admitted start: its slot is proven. That ends contention when
    /// the app admitted the start in contention.
    fn accepted(&self, vm_id: &str, request_id: &str) {
        let mut state = self.lock();
        let admitted_in_contention = match state.admitted.get_mut(vm_id) {
            Some(held) if held.request_id == request_id => {
                held.proven = true;
                Some(held.in_contention)
            }
            _ => None,
        };
        if let Some(in_contention) = admitted_in_contention {
            state.contention = contention_after_acceptance(state.contention, in_contention);
        }
        state.version += 1;
        self.changed.notify_all();
    }

    /// The framework refused an admitted start (spec "A refusal by the framework"). The hold
    /// ends, contention starts, and the app notes the time and the line of the refusal. When the
    /// wait has time left, the start goes back to the place of its join, and this waits for the
    /// admission again. The queue lists two starts that were refused together in the order of
    /// their join, whichever refusal comes first.
    fn refuse(
        self: &Arc<Self>,
        hold: &mut Hold,
        line: &str,
        cancel: &CancellationToken,
        progress: &Progress,
    ) -> Result<Readmission, VmError> {
        let mut state = self.lock();
        let now = self.now();
        if state
            .admitted
            .get(&hold.vm_id)
            .is_some_and(|held| held.request_id == hold.request_id)
        {
            state.admitted.remove(&hold.vm_id);
        }
        state.contention = true;
        state.last_refusal = Some(Refusal {
            at: now,
            line: line.to_string(),
        });
        state.version += 1;
        // A refusal at or after the deadline is the verdict of the start.
        if !hold.deadline.is_some_and(|deadline| now < deadline) {
            self.changed.notify_all();
            return Ok(Readmission::Stands);
        }
        let at = state
            .waiting
            .iter()
            .position(|e| e.seq > hold.seq)
            .unwrap_or(state.waiting.len());
        state.waiting.insert(
            at,
            Entry {
                request_id: hold.request_id.clone(),
                vm_id: hold.vm_id.clone(),
                seq: hold.seq,
                deadline: hold.deadline,
                refusal: Some(line.to_string()),
            },
        );
        if let Some(note) = hold
            .refusals
            .note(&hold.request_id, at + 1, line, self.pause, now)
        {
            progress.line(&note);
        }
        self.changed.notify_all();
        self.run_decision(&mut state);
        let request = Request {
            request_id: hold.request_id.clone(),
            vm_id: hold.vm_id.clone(),
            joined: hold.joined,
        };
        self.wait_admitted(state, &request, &mut hold.reporter, cancel, progress)?;
        hold.waited = self.now().saturating_sub(hold.joined);
        Ok(Readmission::Admitted)
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

    /// Whether the app is in contention. A test reads this.
    pub fn in_contention(&self) -> bool {
        self.lock().contention
    }

    /// The line of the last refusal by the framework. A test reads this.
    pub fn last_refusal_line(&self) -> Option<String> {
        self.lock().last_refusal.as_ref().map(|r| r.line.clone())
    }
}

/// The hold of one admitted start. Dropping it releases the slot.
struct Hold {
    queue: Arc<AdmissionQueue>,
    vm_id: String,
    request_id: String,
    seq: u64,
    /// The moment of the join, on the clock of the queue.
    joined: Duration,
    /// The end of the wait, on the clock of the queue. `None` for a start that never waits.
    deadline: Option<Duration>,
    waited: Duration,
    /// Keeps the lines of the waits that follow a refusal from repeating themselves.
    reporter: WaitReporter,
    refusals: RefusalReporter,
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

    fn accepted(&self) {
        self.queue.accepted(&self.vm_id, &self.request_id);
    }

    fn refused(
        &mut self,
        line: &str,
        cancel: &CancellationToken,
        progress: &Progress,
    ) -> Result<Readmission, VmError> {
        let queue = Arc::clone(&self.queue);
        queue.refuse(self, line, cancel, progress)
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

/// Decides when a start says that the framework refused it. A start that the framework refuses
/// again and again says so when it first happens, when the place or the line changes, and every
/// 30 s otherwise.
#[derive(Default)]
struct RefusalReporter {
    last: Option<(usize, String, Duration)>,
}

impl RefusalReporter {
    fn note(
        &mut self,
        request_id: &str,
        place: usize,
        line: &str,
        pause: Duration,
        now: Duration,
    ) -> Option<String> {
        let due = match &self.last {
            None => true,
            Some((last_place, last_line, at)) => {
                *last_place != place || last_line != line || now.saturating_sub(*at) >= REPORT_EVERY
            }
        };
        if !due {
            return None;
        }
        self.last = Some((place, line.to_string(), now));
        Some(format!(
            "the framework refused request {request_id}: {line}. The start is back at place {place}. \
             The app admits nothing for a pause of {} s, then tries this start again",
            pause.as_secs()
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
    fn with_one_slot_a_second_start_waits_while_a_first_guest_runs() {
        let (queue, _) = queue(1);
        let cancel = CancellationToken::new();
        let first = queue.admit(&ask("a", Some(5)), &cancel, &quiet()).unwrap();
        let second = {
            let queue = Arc::clone(&queue);
            std::thread::spawn(move || {
                queue
                    .admit(&ask("b", Some(30)), &CancellationToken::new(), &quiet())
                    .is_ok()
            })
        };
        wait_until("the second start to join", || queue.waiting().len() == 1);
        assert!(
            !second.is_finished(),
            "the framework has a free slot, but the share is full"
        );
        drop(first);
        assert!(
            second.join().unwrap(),
            "the release admits the second start"
        );
    }

    #[test]
    fn with_one_slot_the_refusal_of_a_full_share_names_one_as_the_limit() {
        let (queue, guests) = queue(1);
        guests.lock().unwrap().insert("g1".to_string());
        let limit = limit_of(
            queue
                .admit(&ask("late", None), &CancellationToken::new(), &quiet())
                .err()
                .expect("the share is full"),
        );
        assert_eq!(limit.limit, 1);
        assert_eq!(limit.holders, ["g1"]);
    }

    #[test]
    fn with_no_slot_every_start_fails_at_once_with_limit_0_and_waits_for_nothing() {
        let (queue, _) = queue(0);
        for wait in [None, Some(0), Some(600)] {
            let began = Instant::now();
            let error = queue
                .admit(&ask("a", wait), &CancellationToken::new(), &quiet())
                .err()
                .unwrap_or_else(|| panic!("{wait:?}: no slot to admit"));
            assert!(
                began.elapsed() < Duration::from_secs(1),
                "{wait:?}: the start waited"
            );
            let limit = limit_of(error);
            assert_eq!(limit.limit, 0, "{wait:?}");
            assert_eq!(limit.admission.refused_by, RefusedBy::App);
            assert_eq!(limit.admission.waited_secs, 0);
            assert!(queue.waiting().is_empty() && queue.admitted().is_empty());
        }
    }

    #[test]
    fn the_limit_is_the_share_when_the_own_holders_fill_it_and_the_limit_of_apple_otherwise() {
        let (queue, _) = queue(1);
        let mut state = State::default();
        assert_eq!(queue.limit(&state), 2, "a guest that the app cannot see");
        state.holders = vec!["g1".into()];
        assert_eq!(queue.limit(&state), 1);
        let (queue, _) = self::queue(0);
        assert_eq!(queue.limit(&State::default()), 0);
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
        // The second waiter says its new place before the slot frees. Without this wait, a busy
        // machine can admit it before it speaks.
        wait_until("the place to move up", || {
            lines.lock().unwrap().iter().any(|l| l.contains("place 1"))
        });
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
                refusal: None,
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

    // The refusal by the framework (spec "admission-queue": "A refusal by the framework").

    const LINE: &str = "The number of VMs exceeds the system limit (other running VMs: holder-a)";

    /// A queue whose pause after a refusal is short, so that a test does not wait 3 s for it.
    fn quick_queue(slots: usize) -> (Arc<AdmissionQueue>, Arc<Mutex<BTreeSet<String>>>) {
        let guests = Arc::new(Mutex::new(BTreeSet::new()));
        let read = Arc::clone(&guests);
        let queue = AdmissionQueue::with_pause(
            slots,
            Box::new(move || read.lock().unwrap().clone()),
            Duration::from_millis(300),
        );
        (queue, guests)
    }

    type Refused = std::thread::JoinHandle<(Result<Readmission, VmError>, Box<dyn SlotHold>)>;

    /// The framework refuses `hold`. The call blocks while the start waits to be admitted again,
    /// so it runs on a thread, and the thread gives the hold back.
    fn refuse(
        mut hold: Box<dyn SlotHold>,
        cancel: &CancellationToken,
        progress: Progress,
    ) -> Refused {
        let cancel = cancel.clone();
        std::thread::spawn(move || {
            let outcome = hold.refused(LINE, &cancel, &progress);
            (outcome, hold)
        })
    }

    fn join_waiter(
        queue: &Arc<AdmissionQueue>,
        vm_id: &'static str,
        wait_secs: u64,
    ) -> std::thread::JoinHandle<Result<Box<dyn SlotHold>, VmError>> {
        let queue = Arc::clone(queue);
        let joined = queue.waiting().len();
        let handle = {
            let queue = Arc::clone(&queue);
            std::thread::spawn(move || {
                queue.admit(
                    &ask(vm_id, Some(wait_secs)),
                    &CancellationToken::new(),
                    &quiet(),
                )
            })
        };
        wait_until("the waiter to join", || queue.waiting().len() == joined + 1);
        handle
    }

    fn vm_ids(queue: &AdmissionQueue) -> Vec<String> {
        queue.waiting().into_iter().map(|(_, vm)| vm).collect()
    }

    #[test]
    fn a_refused_start_goes_back_ahead_of_the_waiters_that_joined_after_it() {
        let (queue, _) = quick_queue(1);
        let a = queue
            .admit(&ask("a", Some(30)), &CancellationToken::new(), &quiet())
            .unwrap();
        let b = join_waiter(&queue, "b", 30);
        let c = join_waiter(&queue, "c", 30);

        let cancel = CancellationToken::new();
        let refused = refuse(a, &cancel, quiet());
        wait_until("a back in the queue", || vm_ids(&queue) == ["a", "b", "c"]);
        assert!(queue.in_contention(), "a refusal starts contention");
        assert_eq!(queue.last_refusal_line().as_deref(), Some(LINE));
        assert!(queue.admitted().is_empty(), "the refusal ended the hold");

        // The pause ends, and contention admits the head alone: a, never b.
        let (outcome, a) = refused.join().unwrap();
        assert_eq!(outcome.unwrap(), Readmission::Admitted);
        assert_eq!(queue.admitted(), ["a"]);
        assert_eq!(vm_ids(&queue), ["b", "c"]);
        assert!(!b.is_finished() && !c.is_finished());

        // The framework accepts the retry in contention, and contention ends.
        a.accepted();
        assert!(!queue.in_contention());
        drop(a);
        let b = b.join().unwrap().expect("a release admits b before c");
        assert_eq!(vm_ids(&queue), ["c"]);
        drop(b);
        c.join().unwrap().expect("then c");
    }

    #[test]
    fn two_starts_that_the_framework_refused_together_go_back_in_the_order_of_their_join() {
        for refused_first in ["a", "b"] {
            let (queue, guests) = quick_queue(2);
            let cancel = CancellationToken::new();
            let a = queue.admit(&ask("a", Some(30)), &cancel, &quiet()).unwrap();
            let b = queue.admit(&ask("b", Some(30)), &cancel, &quiet()).unwrap();
            let c = join_waiter(&queue, "c", 30);
            // Both slots look taken now, so the retries wait where the test can see them.
            guests
                .lock()
                .unwrap()
                .extend(["g1".to_string(), "g2".to_string()]);
            let (first, second) = if refused_first == "a" { (a, b) } else { (b, a) };
            let first = refuse(first, &cancel, quiet());
            wait_until("the first refusal", || queue.waiting().len() == 2);
            let second = refuse(second, &cancel, quiet());
            wait_until("the second refusal", || queue.waiting().len() == 3);
            assert_eq!(
                vm_ids(&queue),
                ["a", "b", "c"],
                "the refusal of {refused_first} came first"
            );
            cancel.cancel();
            for refused in [first, second] {
                let (outcome, _) = refused.join().unwrap();
                assert!(matches!(outcome, Err(VmError::StartCancelled { .. })));
            }
            let _ = c;
        }
    }

    #[test]
    fn contention_admits_nothing_while_the_other_start_of_a_pair_is_unproven() {
        let (queue, _) = quick_queue(2);
        let cancel = CancellationToken::new();
        let a = queue.admit(&ask("a", Some(30)), &cancel, &quiet()).unwrap();
        let _b = queue.admit(&ask("b", Some(30)), &cancel, &quiet()).unwrap();
        // The framework refuses a. b has neither been accepted nor refused.
        let refused = refuse(a, &cancel, quiet());
        wait_until("a back in the queue", || vm_ids(&queue) == ["a"]);
        std::thread::sleep(Duration::from_millis(600));
        // The pause is over, and the count shows a free slot, but b is unproven.
        assert_eq!(vm_ids(&queue), ["a"]);
        assert!(!refused.is_finished(), "contention holds a back");
        cancel.cancel();
        let _ = refused.join().unwrap();
    }

    #[test]
    fn an_acceptance_of_a_start_admitted_before_the_refusal_does_not_end_contention() {
        let (queue, _) = quick_queue(2);
        let cancel = CancellationToken::new();
        let a = queue.admit(&ask("a", Some(30)), &cancel, &quiet()).unwrap();
        let b = queue.admit(&ask("b", Some(30)), &cancel, &quiet()).unwrap();
        let c = join_waiter(&queue, "c", 30);

        let refused = refuse(a, &cancel, quiet());
        wait_until("a back in the queue", || vm_ids(&queue) == ["a", "c"]);
        // The framework accepts b, which the app admitted before the refusal.
        b.accepted();
        assert!(queue.in_contention(), "that acceptance proves only b");

        // The retry of a is admitted alone, and c stays behind it.
        let (outcome, a) = refused.join().unwrap();
        assert_eq!(outcome.unwrap(), Readmission::Admitted);
        assert_eq!(vm_ids(&queue), ["c"]);
        assert!(queue.in_contention());

        // Its acceptance ends contention. The count is full, so c still waits for a release.
        a.accepted();
        assert!(!queue.in_contention());
        assert!(!c.is_finished());
        drop(b);
        let c = c.join().unwrap().expect("a release admits c");
        drop((a, c));
    }

    #[test]
    fn a_refusal_at_the_deadline_is_the_verdict_and_leaves_no_hold() {
        let (queue, _) = quick_queue(2);
        let hold = queue
            .admit(&ask("a", Some(1)), &CancellationToken::new(), &quiet())
            .unwrap();
        std::thread::sleep(Duration::from_millis(1100));
        let (outcome, hold) = refuse(hold, &CancellationToken::new(), quiet())
            .join()
            .unwrap();
        assert_eq!(outcome.unwrap(), Readmission::Stands);
        assert!(
            queue.waiting().is_empty(),
            "no time left, so it queues no more"
        );
        assert!(queue.admitted().is_empty(), "and holds no slot");
        drop(hold);
    }

    #[test]
    fn a_start_that_never_waits_stands_at_its_refusal() {
        let (queue, _) = quick_queue(2);
        let hold = queue
            .admit(&ask("a", None), &CancellationToken::new(), &quiet())
            .unwrap();
        let (outcome, _hold) = refuse(hold, &CancellationToken::new(), quiet())
            .join()
            .unwrap();
        assert_eq!(outcome.unwrap(), Readmission::Stands);
        assert!(queue.waiting().is_empty() && queue.admitted().is_empty());
    }

    #[test]
    fn a_wait_that_runs_out_after_a_refusal_fails_with_the_app_and_the_line_of_the_framework() {
        let (queue, guests) = quick_queue(1);
        let hold = queue
            .admit(&ask("a", Some(1)), &CancellationToken::new(), &quiet())
            .unwrap();
        // Another guest takes the slot that the refusal gave back.
        guests.lock().unwrap().insert("g1".to_string());
        let (outcome, _hold) = refuse(hold, &CancellationToken::new(), quiet())
            .join()
            .unwrap();
        let limit = limit_of(outcome.expect_err("the wait runs out in the queue"));
        assert_eq!(limit.admission.refused_by, RefusedBy::App);
        assert_eq!(limit.admission.place, Some(1));
        assert_eq!(limit.raw, LINE, "the last refusal of this start");
        assert!(limit.admission.unseen_possible);
        assert!(queue.waiting().is_empty() && queue.admitted().is_empty());
    }

    #[test]
    fn a_cancel_of_a_start_that_the_queue_took_back_removes_it_and_holds_no_slot() {
        let (queue, guests) = quick_queue(1);
        let cancel = CancellationToken::new();
        let hold = queue.admit(&ask("a", Some(30)), &cancel, &quiet()).unwrap();
        guests.lock().unwrap().insert("g1".to_string());
        let refused = refuse(hold, &cancel, quiet());
        wait_until("a back in the queue", || vm_ids(&queue) == ["a"]);
        cancel.cancel();
        let (outcome, _hold) = refused.join().unwrap();
        assert!(matches!(outcome, Err(VmError::StartCancelled { .. })));
        assert!(queue.waiting().is_empty() && queue.admitted().is_empty());
    }

    #[test]
    fn the_refusal_writes_a_line_that_names_the_request_the_place_and_the_pause() {
        let (queue, guests) = queue(1);
        let cancel = CancellationToken::new();
        let hold = queue.admit(&ask("a", Some(30)), &cancel, &quiet()).unwrap();
        let request_id = hold.request_id().to_string();
        guests.lock().unwrap().insert("g1".to_string());
        let lines = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = Arc::clone(&lines);
        let progress = Progress::to(move |line| sink.lock().unwrap().push(line.to_string()));
        let refused = refuse(hold, &cancel, progress);
        wait_until("a back in the queue", || vm_ids(&queue) == ["a"]);
        cancel.cancel();
        let _ = refused.join().unwrap();
        let lines = lines.lock().unwrap();
        let refusal = lines
            .iter()
            .find(|l| l.contains("framework refused"))
            .unwrap_or_else(|| panic!("no refusal line: {lines:?}"));
        assert!(refusal.contains(&request_id), "{refusal}");
        assert!(refusal.contains("place 1"), "{refusal}");
        assert!(refusal.contains("pause of 3 s"), "{refusal}");
        assert!(refusal.contains(LINE), "{refusal}");
    }

    #[test]
    fn contention_holds_back_a_start_without_a_wait_while_a_start_is_unproven() {
        let (queue, _) = quick_queue(2);
        let cancel = CancellationToken::new();
        let a = queue.admit(&ask("a", None), &cancel, &quiet()).unwrap();
        let b = queue.admit(&ask("b", None), &cancel, &quiet()).unwrap();
        // The framework refuses a, which has no wait. b is unproven, and the queue is empty.
        let (outcome, _a) = refuse(a, &cancel, quiet()).join().unwrap();
        assert_eq!(outcome.unwrap(), Readmission::Stands);
        assert!(queue.in_contention());
        let error = queue
            .admit(&ask("d", None), &cancel, &quiet())
            .err()
            .expect("contention blocks the admission");
        let limit = limit_of(error);
        assert_eq!(limit.admission.refused_by, RefusedBy::App);
        assert!(
            limit.admission.unseen_possible,
            "contention names the unseen guest"
        );
        assert_eq!(limit.raw, "", "no tart run of this start took part");
        assert!(queue.waiting().is_empty(), "it took no place");

        // b is accepted, nothing is unproven, and contention ends: the next start is admitted.
        b.accepted();
        let d = queue.admit(&ask("d", None), &cancel, &quiet());
        assert!(d.is_ok(), "the count shows a free slot again");
        assert!(!queue.in_contention());
    }

    #[test]
    fn the_pause_never_blocks_a_start_without_a_wait() {
        let (queue, _) = queue(2);
        let cancel = CancellationToken::new();
        let a = queue.admit(&ask("a", None), &cancel, &quiet()).unwrap();
        let (outcome, _a) = refuse(a, &cancel, quiet()).join().unwrap();
        assert_eq!(outcome.unwrap(), Readmission::Stands);
        // The pause of 3 s runs. Nothing is unproven, so contention has ended too.
        let began = Instant::now();
        let b = queue.admit(&ask("b", None), &cancel, &quiet());
        assert!(b.is_ok());
        assert!(began.elapsed() < Duration::from_millis(500));
    }

    #[test]
    fn a_start_that_a_refusal_keeps_in_contention_is_tried_again_after_the_pause_and_not_before() {
        let (queue, _) = quick_queue(2);
        let cancel = CancellationToken::new();
        let a = queue.admit(&ask("a", Some(30)), &cancel, &quiet()).unwrap();
        let refused_at = Instant::now();
        let (outcome, _a) = refuse(a, &cancel, quiet()).join().unwrap();
        assert_eq!(outcome.unwrap(), Readmission::Admitted);
        assert!(
            refused_at.elapsed() >= Duration::from_millis(300),
            "the pause held the retry back: {:?}",
            refused_at.elapsed()
        );
    }

    #[test]
    fn a_line_after_a_refusal_repeats_only_when_something_changed_or_30_s_passed() {
        let mut reporter = RefusalReporter::default();
        let pause = Duration::from_secs(3);
        let first = reporter
            .note("r-1", 1, LINE, pause, Duration::ZERO)
            .expect("the first refusal speaks");
        assert!(first.contains("pause of 3 s"), "{first}");
        assert!(reporter
            .note("r-1", 1, LINE, pause, Duration::from_secs(4))
            .is_none());
        assert!(reporter
            .note("r-1", 2, LINE, pause, Duration::from_secs(8))
            .is_some());
        assert!(reporter
            .note("r-1", 2, "another line", pause, Duration::from_secs(12))
            .is_some());
        assert!(reporter
            .note("r-1", 2, "another line", pause, Duration::from_secs(42))
            .is_some());
    }
}
