//! The pure decision of the admission queue (spec "admission-queue": "The admission decision" and
//! "A refusal by the framework").
//!
//! The function reads the queue, the number of own holders, the slot limit, the last refusal and the
//! time. It touches no clock, no file and no process, so a test can set every input. The state of
//! the queue (`super`) gathers the inputs and applies the answer under one lock.
//!
//! The count has two kinds of holder. The own holders count against `macos_slots` and against the
//! two slots of Apple. The seen holders, the other macOS guests that tart reports, count against
//! the two slots only. The function admits a waiter only on an answer of tart whose observation
//! started after the waiter joined, and it admits nothing before the first answer.

use std::time::Duration;

/// The app admits nothing for this long after a refusal by the framework.
pub const PAUSE: Duration = Duration::from_secs(3);

/// The slots that Apple allows for macOS guests on one Mac.
pub const APPLE_LIMIT: usize = 2;

/// One start in the queue. The queue lists its waiters in the order of their join.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Waiter {
    pub request_id: String,
    /// The moment when the wait of this waiter ends, on the clock of the queue. `None` for a
    /// waiter without a bound.
    pub deadline: Option<Duration>,
    /// The number of the next observation at the join of this waiter. An answer is newer than
    /// the join when its observation has this number or a later one.
    pub joined_obs: u64,
    /// Whether the waiter is a golden creation. The app sees its hold and not its boots, so the
    /// framework never answers the admission of a golden creation, and the app never counts it
    /// as unproven.
    pub golden: bool,
}

/// What the function reads.
#[derive(Debug, Clone, Copy)]
pub struct Input<'a> {
    /// The waiters in the order of their join.
    pub queue: &'a [Waiter],
    /// The own holders: each running macOS guest that has a meta, and each start that the app
    /// admitted and whose guest is not yet gone. A guest that two holders name counts once.
    pub own_holders: usize,
    /// The other macOS guests that the last answer of tart reports running. A guest that an own
    /// holder names is not among them.
    pub seen_holders: usize,
    /// The number of the observation of the last answer, or `None` before the first answer.
    pub answer: Option<u64>,
    /// The admitted starts that the framework has neither accepted nor refused. Each one is also
    /// an own holder. A golden creation is never unproven.
    pub unproven: usize,
    /// The slots that the app may hold. The limit of Apple is 2.
    pub slots: usize,
    /// Whether the app is in contention.
    pub contention: bool,
    /// The time of the last refusal by the framework, on the clock of the queue.
    pub last_refusal: Option<Duration>,
    /// How long the app admits nothing after a refusal.
    pub pause: Duration,
    /// The time now, on the clock of the queue.
    pub now: Duration,
}

/// What the function answers.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Decision {
    /// The waiters to admit, head first.
    pub admit: Vec<String>,
    /// The waiters whose deadline has come and that this decision did not admit.
    pub expire: Vec<String>,
}

/// Decide which waiters the app admits, and which waiters run out of time.
///
/// First the function admits the head while the count shows a free slot, and it counts each
/// admission at once. The count shows a free slot while the own holders are below `slots` and
/// the own and seen holders together are below the two slots of Apple. A waiter needs an answer
/// that is newer than its join, so the function admits nothing before the first answer. It
/// never admits a waiter before an earlier one. The pause after a refusal
/// holds back every admission, except that it ends at the deadline of the head: the pause never
/// costs the head its last try. In contention the function admits the head alone, and only while
/// no admitted start is unproven. Then it expires each waiter whose deadline has come and that it
/// did not admit. So a waiter gets one last decision at its deadline, and the last try comes
/// before the failure.
pub fn decide(input: Input<'_>) -> Decision {
    let paused = input
        .last_refusal
        .is_some_and(|at| input.now < at + input.pause);
    let mut admit = Vec::new();
    // The starts among the admissions of this decision. Only they await the framework.
    let mut admitted_unproven = 0;
    for waiter in input.queue {
        // Each admission is a holder at once, and it is unproven until the framework answers.
        let held = input.own_holders + admit.len();
        let unproven = input.unproven + admitted_unproven;
        let newer = input
            .answer
            .is_some_and(|number| number >= waiter.joined_obs);
        let due = waiter
            .deadline
            .is_some_and(|deadline| input.now >= deadline);
        let blocked = !newer
            || held >= input.slots
            || held + input.seen_holders >= APPLE_LIMIT
            || (paused && !due)
            || (input.contention && unproven > 0);
        if blocked {
            break;
        }
        admit.push(waiter.request_id.clone());
        if !waiter.golden {
            admitted_unproven += 1;
        }
    }
    let expire = input.queue[admit.len()..]
        .iter()
        .filter(|w| w.deadline.is_some_and(|deadline| input.now >= deadline))
        .map(|w| w.request_id.clone())
        .collect();
    Decision { admit, expire }
}

/// The one decision of a start without `--wait`, or with `--wait=0`: the app admits it only when
/// the queue holds no waiter, the count shows a free slot and contention does not hold back an
/// admission. Such a start never passes a waiter and never becomes one. The pause after a refusal
/// never blocks it, because the decision never waits for the pause. The caller passes the seen
/// holders of an answer that is newer than the join of the start.
pub fn admits_without_wait(
    waiters: usize,
    own_holders: usize,
    seen_holders: usize,
    slots: usize,
    contention: bool,
    unproven: usize,
) -> bool {
    waiters == 0
        && own_holders < slots
        && own_holders + seen_holders < APPLE_LIMIT
        && !(contention && unproven > 0)
}

/// Whether the own holders alone refuse a start without `--wait` at its join: they fill
/// `slots`, or they fill the two slots of Apple. No answer of tart ends such a hold, so the
/// refusal waits for no observation (spec "A start without `--wait`").
pub fn own_holders_fill_the_slots(admitted: usize, slots: usize) -> bool {
    admitted >= slots || admitted >= APPLE_LIMIT
}

/// Contention ends when the queue holds no waiter and no admitted start is unproven. Then the app
/// admits by the count again.
pub fn contention_after_settle(contention: bool, waiters: usize, unproven: usize) -> bool {
    contention && (waiters > 0 || unproven > 0)
}

/// Contention ends when the framework accepts a start that the app admitted in contention. An
/// acceptance of a start that the app admitted before the refusal proves only that one slot was
/// free for that try, so it does not end contention.
pub fn contention_after_acceptance(contention: bool, admitted_in_contention: bool) -> bool {
    contention && !admitted_in_contention
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secs(n: u64) -> Duration {
        Duration::from_secs(n)
    }

    fn waiter(id: &str, deadline: Option<u64>) -> Waiter {
        joined_at(id, deadline, 0)
    }

    /// A waiter that joined before the observation with number `joined_obs`.
    fn joined_at(id: &str, deadline: Option<u64>, joined_obs: u64) -> Waiter {
        Waiter {
            request_id: id.into(),
            deadline: deadline.map(secs),
            joined_obs,
            golden: false,
        }
    }

    /// A golden creation that joined before the observation 0.
    fn golden(id: &str) -> Waiter {
        Waiter {
            golden: true,
            ..waiter(id, None)
        }
    }

    fn decide_at(queue: &[Waiter], own: usize, now: u64) -> Decision {
        decide(Input {
            queue,
            own_holders: own,
            slots: 2,
            now: secs(now),
            ..quiet()
        })
    }

    /// An input with no refusal and no unproven start, for the cases that the first increment
    /// covered. A test sets the members that it is about.
    fn quiet() -> Input<'static> {
        Input {
            queue: &[],
            own_holders: 0,
            seen_holders: 0,
            // The answer of observation 0 is newer than the join of a waiter of observation 0.
            answer: Some(0),
            unproven: 0,
            slots: 2,
            contention: false,
            last_refusal: None,
            pause: PAUSE,
            now: Duration::ZERO,
        }
    }

    fn ids(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn a_join_while_both_slots_are_held_admits_nobody() {
        let queue = [waiter("r-1", None)];
        assert_eq!(decide_at(&queue, 2, 0), Decision::default());
    }

    #[test]
    fn a_join_while_a_slot_is_free_admits_the_joiner() {
        let queue = [waiter("r-1", None)];
        assert_eq!(decide_at(&queue, 1, 0).admit, ids(&["r-1"]));
    }

    #[test]
    fn the_app_admits_the_head_and_never_a_later_waiter_first() {
        let queue = [
            waiter("r-1", None),
            waiter("r-2", None),
            waiter("r-3", None),
        ];
        assert_eq!(decide_at(&queue, 1, 0).admit, ids(&["r-1"]));
    }

    #[test]
    fn two_free_slots_admit_two_waiters_at_once() {
        let queue = [
            waiter("r-1", None),
            waiter("r-2", None),
            waiter("r-3", None),
        ];
        assert_eq!(decide_at(&queue, 0, 0).admit, ids(&["r-1", "r-2"]));
    }

    #[test]
    fn a_release_admits_the_next_waiter_in_order() {
        let queue = [waiter("r-2", None), waiter("r-3", None)];
        assert_eq!(decide_at(&queue, 2, 5), Decision::default());
        assert_eq!(decide_at(&queue, 1, 6).admit, ids(&["r-2"]));
    }

    #[test]
    fn a_leave_moves_the_places_up() {
        let before = [
            waiter("r-1", None),
            waiter("r-2", None),
            waiter("r-3", None),
        ];
        let after = [waiter("r-1", None), waiter("r-3", None)];
        assert_eq!(decide_at(&before, 1, 0).admit, ids(&["r-1"]));
        // The head leaves, and the next waiter is the head.
        assert_eq!(decide_at(&after[1..], 1, 0).admit, ids(&["r-3"]));
    }

    #[test]
    fn a_tick_changes_nothing_before_a_deadline() {
        let queue = [waiter("r-1", Some(10))];
        assert_eq!(decide_at(&queue, 2, 3), Decision::default());
        assert_eq!(decide_at(&queue, 2, 6), Decision::default());
    }

    #[test]
    fn a_waiter_that_the_count_blocks_expires_at_its_deadline() {
        let queue = [waiter("r-1", Some(10))];
        assert_eq!(decide_at(&queue, 2, 9), Decision::default());
        assert_eq!(decide_at(&queue, 2, 10).expire, ids(&["r-1"]));
        assert_eq!(decide_at(&queue, 2, 11).expire, ids(&["r-1"]));
    }

    #[test]
    fn the_last_try_at_a_deadline_admits_before_it_expires() {
        // A slot freed in the last moment: the decision at the deadline admits the head.
        let queue = [waiter("r-1", Some(10))];
        let decision = decide_at(&queue, 1, 10);
        assert_eq!(decision.admit, ids(&["r-1"]));
        assert_eq!(decision.expire, Vec::<String>::new());
    }

    #[test]
    fn a_waiter_behind_the_head_expires_when_only_the_head_gets_the_slot() {
        let queue = [waiter("r-1", Some(10)), waiter("r-2", Some(10))];
        let decision = decide_at(&queue, 1, 10);
        assert_eq!(decision.admit, ids(&["r-1"]));
        assert_eq!(decision.expire, ids(&["r-2"]));
    }

    #[test]
    fn a_waiter_without_a_bound_never_expires() {
        let queue = [waiter("r-1", None)];
        assert_eq!(decide_at(&queue, 2, 1_000_000), Decision::default());
    }

    #[test]
    fn an_empty_queue_decides_nothing() {
        assert_eq!(decide_at(&[], 0, 0), Decision::default());
    }

    #[test]
    fn the_slot_limit_of_the_config_lowers_the_admissions() {
        let queue = [waiter("r-1", None)];
        let one_slot = |own| {
            decide(Input {
                queue: &queue,
                own_holders: own,
                slots: 1,
                ..quiet()
            })
        };
        assert_eq!(one_slot(0).admit, ids(&["r-1"]));
        assert_eq!(one_slot(1), Decision::default());
        let none = decide(Input {
            queue: &queue,
            slots: 0,
            ..quiet()
        });
        assert_eq!(none, Decision::default());
    }

    #[test]
    fn own_holders_above_the_limit_leave_no_free_slot() {
        let queue = [waiter("r-1", None)];
        assert_eq!(decide_at(&queue, 3, 0), Decision::default());
    }

    #[test]
    fn a_start_with_wait_zero_is_admitted_only_by_an_empty_queue_and_a_free_slot() {
        assert!(admits_without_wait(0, 0, 0, 2, false, 0));
        assert!(admits_without_wait(0, 1, 0, 2, false, 0));
        assert!(!admits_without_wait(0, 2, 0, 2, false, 0));
        // It never passes a waiter, even when the count shows a free slot.
        assert!(!admits_without_wait(1, 0, 0, 2, false, 0));
        assert!(!admits_without_wait(0, 0, 0, 0, false, 0));
    }

    // The refusal by the framework: the pause and contention.

    /// The state after one refusal at the second 10, with A refused and back at the head.
    fn after_a_refusal<'a>(
        queue: &'a [Waiter],
        own: usize,
        unproven: usize,
        now: u64,
    ) -> Input<'a> {
        Input {
            queue,
            own_holders: own,
            unproven,
            contention: true,
            last_refusal: Some(secs(10)),
            now: secs(now),
            ..quiet()
        }
    }

    #[test]
    fn a_refusal_holds_back_every_admission_for_the_pause() {
        let queue = [waiter("r-1", Some(600)), waiter("r-2", Some(600))];
        assert_eq!(
            decide(after_a_refusal(&queue, 0, 0, 10)),
            Decision::default()
        );
        assert_eq!(
            decide(after_a_refusal(&queue, 0, 0, 12)),
            Decision::default()
        );
    }

    #[test]
    fn the_end_of_the_pause_admits_the_head_alone() {
        let queue = [waiter("r-1", Some(600)), waiter("r-2", Some(600))];
        // Two slots are free by the count, and contention still admits one start.
        let decision = decide(after_a_refusal(&queue, 0, 0, 13));
        assert_eq!(decision.admit, ids(&["r-1"]));
        assert_eq!(decision.expire, Vec::<String>::new());
    }

    #[test]
    fn the_pause_ends_at_the_deadline_of_the_head() {
        // The head has its last try during the pause.
        let queue = [waiter("r-1", Some(11))];
        assert_eq!(
            decide(after_a_refusal(&queue, 0, 0, 10)),
            Decision::default()
        );
        let decision = decide(after_a_refusal(&queue, 0, 0, 11));
        assert_eq!(
            decision.admit,
            ids(&["r-1"]),
            "the pause never costs the last try"
        );
        assert_eq!(decision.expire, Vec::<String>::new());
    }

    #[test]
    fn a_waiter_behind_the_head_that_meets_its_deadline_in_the_pause_gets_no_admission() {
        let queue = [waiter("r-1", Some(11)), waiter("r-2", Some(11))];
        let decision = decide(after_a_refusal(&queue, 0, 0, 11));
        assert_eq!(decision.admit, ids(&["r-1"]));
        assert_eq!(decision.expire, ids(&["r-2"]));
    }

    #[test]
    fn the_pause_holds_whether_or_not_contention_holds() {
        let queue = [waiter("r-1", Some(600))];
        let input = Input {
            contention: false,
            ..after_a_refusal(&queue, 0, 0, 11)
        };
        assert_eq!(decide(input), Decision::default());
        let input = Input {
            now: secs(13),
            ..input
        };
        assert_eq!(decide(input).admit, ids(&["r-1"]));
    }

    #[test]
    fn contention_admits_nothing_while_an_admitted_start_is_unproven() {
        let queue = [waiter("r-3", Some(600))];
        // A and B were admitted together. The framework refused A, and B is still unproven.
        let decision = decide(after_a_refusal(&queue, 1, 1, 20));
        assert_eq!(decision, Decision::default());
        // B was accepted. It holds a slot, and nothing is unproven.
        assert_eq!(
            decide(after_a_refusal(&queue, 1, 0, 20)).admit,
            ids(&["r-3"])
        );
    }

    #[test]
    fn two_refusals_together_leave_the_earlier_start_at_the_head() {
        // The queue lists A before B whichever refusal came first, so contention tries A.
        let queue = [waiter("r-a", Some(600)), waiter("r-b", Some(600))];
        let decision = decide(after_a_refusal(&queue, 0, 0, 13));
        assert_eq!(decision.admit, ids(&["r-a"]));
    }

    #[test]
    fn an_acceptance_before_the_refusal_does_not_end_contention() {
        // B was admitted before the refusal of A and accepted. C waits behind A.
        assert!(contention_after_acceptance(true, false));
        let queue = [waiter("r-a", Some(600)), waiter("r-c", Some(600))];
        let input = after_a_refusal(&queue, 1, 0, 20);
        // Contention holds: the app admits A alone, and then no more.
        assert_eq!(decide(input).admit, ids(&["r-a"]));
        // The retry of A is accepted, and contention ends.
        assert!(!contention_after_acceptance(true, true));
        let after = Input {
            contention: false,
            own_holders: 2,
            ..input
        };
        assert_eq!(decide(after), Decision::default(), "the count is full now");
    }

    #[test]
    fn contention_ends_when_no_waiter_and_no_unproven_start_remain() {
        assert!(contention_after_settle(true, 1, 0));
        assert!(contention_after_settle(true, 0, 1));
        assert!(!contention_after_settle(true, 0, 0));
        assert!(!contention_after_settle(false, 3, 3));
    }

    #[test]
    fn outside_contention_the_count_admits_two_starts_at_once_again() {
        let queue = [
            waiter("r-1", None),
            waiter("r-2", None),
            waiter("r-3", None),
        ];
        let input = Input {
            queue: &queue,
            contention: false,
            last_refusal: Some(secs(10)),
            now: secs(13),
            ..quiet()
        };
        assert_eq!(decide(input).admit, ids(&["r-1", "r-2"]));
    }

    #[test]
    fn a_golden_hold_counts_for_the_slots_and_is_never_unproven() {
        // The golden hold is a synthetic holder: it is one own holder and no unproven start.
        let queue = [waiter("r-1", Some(600)), waiter("r-2", Some(600))];
        let input = after_a_refusal(&queue, 1, 0, 20);
        assert_eq!(
            decide(input).admit,
            ids(&["r-1"]),
            "contention admits the head beside the golden hold, and the head alone"
        );
        // With two holders, a golden hold and a guest, no slot is free.
        assert_eq!(
            decide(after_a_refusal(&queue, 2, 0, 20)),
            Decision::default()
        );
    }

    #[test]
    fn a_golden_creation_admitted_in_contention_leaves_no_unproven_start() {
        // The head is a golden creation: it is a holder at once and never unproven, so the start
        // behind it takes the other slot in the same decision.
        let queue = [golden("g-1"), waiter("r-2", Some(600))];
        assert_eq!(
            decide(after_a_refusal(&queue, 0, 0, 20)).admit,
            ids(&["g-1", "r-2"])
        );
    }

    #[test]
    fn a_golden_creation_behind_an_unproven_start_waits_in_contention() {
        let queue = [waiter("r-1", Some(600)), golden("g-2")];
        assert_eq!(
            decide(after_a_refusal(&queue, 0, 0, 20)).admit,
            ids(&["r-1"]),
            "the start is unproven, and contention admits one try at a time"
        );
        // Without contention both take a slot.
        assert_eq!(decide_at(&queue, 0, 0).admit, ids(&["r-1", "g-2"]));
    }

    #[test]
    fn contention_blocks_a_start_without_a_wait_only_while_a_start_is_unproven() {
        assert!(!admits_without_wait(0, 0, 0, 2, true, 1));
        assert!(admits_without_wait(0, 0, 0, 2, true, 0));
        assert!(admits_without_wait(0, 0, 0, 2, false, 1));
    }

    // The seen holders and the answer of tart.

    fn with_seen<'a>(queue: &'a [Waiter], own: usize, seen: usize, slots: usize) -> Input<'a> {
        Input {
            queue,
            own_holders: own,
            seen_holders: seen,
            slots,
            ..quiet()
        }
    }

    #[test]
    fn a_seen_holder_counts_against_the_two_slots() {
        let queue = [waiter("r-1", None)];
        assert_eq!(decide(with_seen(&queue, 0, 1, 2)).admit, ids(&["r-1"]));
        assert_eq!(decide(with_seen(&queue, 1, 1, 2)), Decision::default());
        assert_eq!(decide(with_seen(&queue, 0, 2, 2)), Decision::default());
    }

    #[test]
    fn a_seen_holder_does_not_count_against_macos_slots() {
        // With `macos_slots` 1, one seen guest and no own holder leave the app's share free:
        // the app admits one start, and the two slots of Apple are then full.
        let queue = [waiter("r-1", None), waiter("r-2", None)];
        assert_eq!(decide(with_seen(&queue, 0, 1, 1)).admit, ids(&["r-1"]));
        // The same count with `macos_slots` 2 admits the head only, because of the two slots.
        assert_eq!(decide(with_seen(&queue, 0, 1, 2)).admit, ids(&["r-1"]));
    }

    #[test]
    fn an_external_guest_beside_an_own_holder_fills_the_two_slots() {
        let queue = [waiter("r-1", None)];
        assert_eq!(decide(with_seen(&queue, 1, 1, 2)), Decision::default());
    }

    #[test]
    fn before_the_first_answer_the_function_admits_nothing_and_a_deadline_fails_the_head() {
        let queue = [waiter("r-1", Some(10))];
        let before = |now| {
            decide(Input {
                answer: None,
                now: secs(now),
                ..with_seen(&queue, 0, 0, 2)
            })
        };
        assert_eq!(before(0), Decision::default());
        assert_eq!(before(9), Decision::default());
        assert_eq!(before(10).expire, ids(&["r-1"]));
        assert_eq!(before(10).admit, Vec::<String>::new());
    }

    #[test]
    fn an_answer_whose_observation_started_before_the_join_admits_no_waiter() {
        // The head joined before observation 5. The answer of observation 4 may miss a guest
        // that started before the join.
        let queue = [joined_at("r-1", Some(10), 5)];
        let older = |now| {
            decide(Input {
                answer: Some(4),
                now: secs(now),
                ..with_seen(&queue, 0, 0, 2)
            })
        };
        assert_eq!(older(3), Decision::default());
        // A deadline fails the head, because no answer came that is newer than the join.
        assert_eq!(older(10).expire, ids(&["r-1"]));
    }

    #[test]
    fn an_answer_of_the_observation_that_the_join_noted_is_newer_than_the_join() {
        let queue = [joined_at("r-1", None, 5)];
        let answer = |number| {
            decide(Input {
                answer: Some(number),
                ..with_seen(&queue, 0, 0, 2)
            })
        };
        assert_eq!(answer(5).admit, ids(&["r-1"]));
        assert_eq!(answer(6).admit, ids(&["r-1"]));
    }

    #[test]
    fn a_later_waiter_needs_an_answer_newer_than_its_own_join() {
        let queue = [joined_at("r-1", None, 1), joined_at("r-2", None, 3)];
        let decision = decide(Input {
            answer: Some(2),
            ..with_seen(&queue, 0, 0, 2)
        });
        assert_eq!(
            decision.admit,
            ids(&["r-1"]),
            "r-2 joined after observation 2 began"
        );
    }

    #[test]
    fn the_own_holders_alone_refuse_a_start_without_a_wait_at_its_join() {
        assert!(!own_holders_fill_the_slots(0, 2));
        assert!(!own_holders_fill_the_slots(1, 2));
        assert!(own_holders_fill_the_slots(2, 2));
        assert!(own_holders_fill_the_slots(1, 1));
        assert!(own_holders_fill_the_slots(0, 0));
    }

    #[test]
    fn a_seen_holder_refuses_a_start_without_a_wait_beside_one_own_holder() {
        assert!(admits_without_wait(0, 0, 1, 2, false, 0));
        assert!(!admits_without_wait(0, 1, 1, 2, false, 0));
        assert!(!admits_without_wait(0, 0, 2, 2, false, 0));
    }
}
