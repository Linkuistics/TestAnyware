//! `GET /v1/slots`: the view that `vm slots --json` prints (spec "admission-queue": "Who asked",
//! "The protocol" and "The command surface").
//!
//! The app builds the view from its queue, and the Host CLI prints it. The same types serve both,
//! so the JSON of the app is the JSON of the command. A Linux or Windows host has no app, and the
//! Host CLI makes [`SlotsView::empty`] itself.

use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::stream::is_plain_vm_id;

/// The path of the requests. `GET` reads the view, and `POST` holds a slot for a golden creation.
/// Both are requests of version 2.
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

/// `POST /v1/slots`: the hold request of a golden creation (spec "admission-queue": "The
/// protocol"). The golden creation joins the queue, and after its admission the open connection is
/// the hold.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HoldRequest {
    /// The VMs that the golden creation boots: its setup VM and, for an IPSW build, the VM
    /// `<base>-building`. The count names them, so that a guest that tart reports under one of
    /// these names counts once.
    pub vms: Vec<String>,
    /// The working directory of the Host CLI, which names the worktree of the grove. An older
    /// Host CLI states none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
}

impl HoldRequest {
    /// Check the request before the app opens a stream. The names go into the count and into
    /// `vm slots`, so each must be a plain VM id, and a golden creation boots at least one VM.
    pub fn validate(&self) -> Result<(), String> {
        if self.vms.is_empty() {
            return Err("the request names no VM that the golden creation boots".into());
        }
        match self.vms.iter().find(|vm| !is_plain_vm_id(vm)) {
            Some(bad) => Err(format!(
                "the VM name '{bad}' has characters other than letters, digits, '.', '-' and '_', \
                 or starts with a dot"
            )),
            None => Ok(()),
        }
    }
}

/// One line of the response to a hold request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum HoldEvent {
    /// A line for stderr about the wait, as a waiting start gets it.
    Progress { line: String },
    /// The app admitted the golden creation. The app sends nothing more, and it holds the slot
    /// until the client closes the connection.
    Admitted { request_id: String },
    /// The request failed, such as `VM_LIMIT_REACHED` when `macos_slots` is 0. It ends the stream.
    Error { error: AppError },
}

impl HoldEvent {
    /// The line of the stream: the event as JSON, and a newline.
    pub fn to_line(&self) -> String {
        let mut line = serde_json::to_string(self).expect("an event serialises");
        line.push('\n');
        line
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

    #[test]
    fn a_hold_request_names_plain_vm_ids_and_at_least_one() {
        let ok = HoldRequest {
            vms: vec!["testanyware-setup-ab12cd34".into(), "base-building".into()],
            cwd: None,
        };
        assert_eq!(ok.validate(), Ok(()));
        for bad in [
            vec![],
            vec!["../x".to_string()],
            vec!["ok".into(), "a/b".into()],
        ] {
            let request = HoldRequest {
                vms: bad,
                cwd: None,
            };
            assert!(request.validate().is_err(), "{request:?}");
        }
    }

    #[test]
    fn the_events_of_a_hold_are_lines_of_json() {
        let admitted = HoldEvent::Admitted {
            request_id: "r-1".into(),
        };
        assert_eq!(
            admitted.to_line(),
            "{\"event\":\"admitted\",\"request_id\":\"r-1\"}\n"
        );
        let mut lines = crate::start::NdjsonLines::<HoldEvent>::default();
        let both = format!(
            "{}{}",
            HoldEvent::Progress {
                line: "waiting".into()
            }
            .to_line(),
            admitted.to_line()
        );
        let (first, second) = both.as_bytes().split_at(10);
        assert_eq!(lines.push(first).unwrap(), vec![]);
        assert_eq!(lines.push(second).unwrap().len(), 2);
    }
}
