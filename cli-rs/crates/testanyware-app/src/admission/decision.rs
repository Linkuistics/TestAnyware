//! The pure decision of the admission queue (spec "admission-queue": "The admission decision").
//!
//! The function reads the queue, the number of own holders, the slot limit and the time. It
//! touches no clock, no file and no process, so a test can set every input. The state of the
//! queue (`super`) gathers the inputs and applies the answer under one lock.
//!
//! This increment decides by the app's own count only. The seen holders, the pause and the
//! contention of a refusal by the framework come in later increments, and each one adds an input
//! here.

use std::time::Duration;

/// One start in the queue. The queue lists its waiters in the order of their join.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Waiter {
    pub request_id: String,
    /// The moment when the wait of this waiter ends, on the clock of the queue. `None` for a
    /// waiter without a bound.
    pub deadline: Option<Duration>,
}

/// What the function reads.
#[derive(Debug, Clone, Copy)]
pub struct Input<'a> {
    /// The waiters in the order of their join.
    pub queue: &'a [Waiter],
    /// The own holders: each running macOS guest that has a meta, and each start that the app
    /// admitted and whose guest is not yet gone. A guest that two holders name counts once.
    pub own_holders: usize,
    /// The slots that the app may hold. The limit of Apple is 2.
    pub slots: usize,
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
/// admission at once. It never admits a waiter before an earlier one. Then it expires each
/// waiter whose deadline has come and that it did not admit. So a waiter gets one last decision
/// at its deadline, and the last try comes before the failure.
pub fn decide(input: Input<'_>) -> Decision {
    let free = input.slots.saturating_sub(input.own_holders);
    let admitted = free.min(input.queue.len());
    let (admit, rest) = input.queue.split_at(admitted);
    Decision {
        admit: admit.iter().map(|w| w.request_id.clone()).collect(),
        expire: rest
            .iter()
            .filter(|w| w.deadline.is_some_and(|deadline| input.now >= deadline))
            .map(|w| w.request_id.clone())
            .collect(),
    }
}

/// The one decision of a start without `--wait`, or with `--wait=0`: the app admits it only when
/// the queue holds no waiter and the count shows a free slot. Such a start never passes a waiter
/// and never becomes one.
pub fn admits_without_wait(waiters: usize, own_holders: usize, slots: usize) -> bool {
    waiters == 0 && own_holders < slots
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secs(n: u64) -> Duration {
        Duration::from_secs(n)
    }

    fn waiter(id: &str, deadline: Option<u64>) -> Waiter {
        Waiter {
            request_id: id.into(),
            deadline: deadline.map(secs),
        }
    }

    fn decide_at(queue: &[Waiter], own: usize, now: u64) -> Decision {
        decide(Input {
            queue,
            own_holders: own,
            slots: 2,
            now: secs(now),
        })
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
                now: secs(0),
            })
        };
        assert_eq!(one_slot(0).admit, ids(&["r-1"]));
        assert_eq!(one_slot(1), Decision::default());
        let none = decide(Input {
            queue: &queue,
            own_holders: 0,
            slots: 0,
            now: secs(0),
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
        assert!(admits_without_wait(0, 0, 2));
        assert!(admits_without_wait(0, 1, 2));
        assert!(!admits_without_wait(0, 2, 2));
        // It never passes a waiter, even when the count shows a free slot.
        assert!(!admits_without_wait(1, 0, 2));
        assert!(!admits_without_wait(0, 0, 0));
    }
}
