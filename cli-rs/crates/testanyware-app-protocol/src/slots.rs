//! `GET /v1/slots`: the view that `vm slots --json` prints (spec "admission-queue": "Who asked",
//! "The protocol" and "The command surface").
//!
//! The app builds the view from its queue, and the Host CLI prints it. The same types serve both,
//! so the JSON of the app is the JSON of the command. A Linux or Windows host has no app, and the
//! Host CLI makes [`SlotsView::empty`] itself.

use serde::{Deserialize, Serialize};

/// The path of the request. It is a request of version 2.
pub const SLOTS_PATH: &str = "/v1/slots";

/// Who asked for a start (spec "Who asked").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestedBy {
    /// The pid of the client process. The app reads it from the kernel for the client's socket, so
    /// the client cannot state a false one. `None` when the kernel states none.
    pub pid: Option<u32>,
    /// The working directory that the Host CLI stated in its request. It names the worktree of the
    /// grove. The app does not check it. An older Host CLI states none.
    pub cwd: Option<String>,
}

/// What holds a slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HolderKind {
    /// A macOS guest that the TestAnyware app started or adopted.
    Vm,
    /// A golden creation that the app admitted.
    Golden,
    /// A macOS guest that tart reports and that the app did not start.
    External,
}

impl HolderKind {
    /// The word of the JSON member.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Vm => "vm",
            Self::Golden => "golden",
            Self::External => "external",
        }
    }
}

/// How a holder holds its slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HolderState {
    /// The app admitted the start, and its guest does not run yet.
    Booting,
    /// The guest runs.
    Running,
    /// A golden creation holds the slot through its connection.
    Held,
}

impl HolderState {
    /// The word of the JSON member.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Booting => "booting",
            Self::Running => "running",
            Self::Held => "held",
        }
    }
}

/// What a waiter will start.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WaiterKind {
    Vm,
    Golden,
}

/// One holder of a slot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlotHolder {
    pub kind: HolderKind,
    /// The names of the guests that the holder runs.
    pub vms: Vec<String>,
    pub state: HolderState,
    /// `None` for a holder that no request of this run of the app made: an adopted VM and a seen
    /// holder.
    pub request_id: Option<String>,
    /// `None` for the same holders.
    pub requested_by: Option<RequestedBy>,
    /// Seconds since the Unix epoch. For a holder that the app admitted, the moment of its
    /// admission. For any other, the moment when the app first noted the holder.
    pub since: Option<u64>,
}

/// One start that waits for a slot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlotWaiter {
    pub request_id: String,
    /// The place in the queue, from 1.
    pub place: u32,
    pub kind: WaiterKind,
    pub vms: Vec<String>,
    pub requested_by: Option<RequestedBy>,
    /// Seconds since the Unix epoch: the join.
    pub since: u64,
    /// Seconds since the Unix epoch: the end of the wait. `None` for a waiter without a bound.
    pub deadline: Option<u64>,
}

/// The last refusal by the framework, while contention holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlotContention {
    /// Seconds since the Unix epoch: the refusal.
    pub since: u64,
    /// The line of tart for the refusal.
    pub tart_error: String,
    /// The holders that the app counted at its last decision.
    pub holders: Vec<String>,
}

/// The data of `vm slots`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlotsView {
    /// The value of `macos_slots` in force.
    pub slots: u32,
    /// Apple's limit.
    pub limit: u32,
    pub holders: Vec<SlotHolder>,
    pub queue: Vec<SlotWaiter>,
    /// `None` while contention does not hold.
    pub contention: Option<SlotContention>,
    /// Seconds since the Unix epoch: the last answer of tart. `None` before the first answer.
    pub observed_at: Option<u64>,
    /// Seconds since the Unix epoch: the first failed observation after the last answer. `None`
    /// while tart answers.
    pub unanswered_since: Option<u64>,
}

impl SlotsView {
    /// The view of a host without an app: no slot, no holder and no waiter.
    pub fn empty() -> Self {
        Self {
            slots: 0,
            limit: 0,
            holders: Vec::new(),
            queue: Vec::new(),
            contention: None,
            observed_at: None,
            unanswered_since: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_view_of_a_host_without_an_app_is_empty_and_has_every_member() {
        assert_eq!(
            serde_json::to_value(SlotsView::empty()).unwrap(),
            json!({
                "slots": 0,
                "limit": 0,
                "holders": [],
                "queue": [],
                "contention": null,
                "observed_at": null,
                "unanswered_since": null,
            })
        );
    }

    #[test]
    fn a_holder_and_a_waiter_state_their_kinds_in_words() {
        let holder = SlotHolder {
            kind: HolderKind::External,
            vms: vec!["guest".into()],
            state: HolderState::Running,
            request_id: None,
            requested_by: None,
            since: Some(7),
        };
        assert_eq!(
            serde_json::to_value(holder).unwrap(),
            json!({
                "kind": "external",
                "vms": ["guest"],
                "state": "running",
                "request_id": null,
                "requested_by": null,
                "since": 7,
            })
        );
        let waiter = SlotWaiter {
            request_id: "r-1".into(),
            place: 1,
            kind: WaiterKind::Vm,
            vms: vec!["vm".into()],
            requested_by: Some(RequestedBy {
                pid: Some(42),
                cwd: None,
            }),
            since: 5,
            deadline: Some(65),
        };
        assert_eq!(
            serde_json::to_value(waiter).unwrap(),
            json!({
                "request_id": "r-1",
                "place": 1,
                "kind": "vm",
                "vms": ["vm"],
                "requested_by": { "pid": 42, "cwd": null },
                "since": 5,
                "deadline": 65,
            })
        );
    }
}
