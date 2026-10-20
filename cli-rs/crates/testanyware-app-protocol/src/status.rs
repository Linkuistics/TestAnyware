//! The body of `GET /v1/status`.

use serde::{Deserialize, Serialize};

use crate::hello::Directories;
use crate::version::ProtocolRange;

/// The app's state. The fields `build`, `protocol_range` and `directories` keep their
/// names in every protocol version, because `doctor` reads them to report a mismatch.
/// Later leaves add fields. A reader must ignore the fields it does not know, and must
/// accept a body that lacks the fields a later version added.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusResponse {
    pub build: String,
    pub protocol_range: ProtocolRange,
    pub directories: Directories,
    /// What the app did with each VM it found when it started (spec: "Lifecycle and adoption").
    #[serde(default)]
    pub adoption: AdoptionStatus,
}

/// The adoption results of the app's current run.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdoptionStatus {
    /// True when the app has read every sidecar and finished the probes of the survivors. The
    /// app serves its socket before this, so a reader that needs the results asks again.
    pub finished: bool,
    /// The VMs the app serves again.
    pub adopted: Vec<AdoptedVm>,
    /// The VMs the app did not adopt, each with the reason. The app signalled none of them.
    pub not_adopted: Vec<NotAdoptedVm>,
}

/// A VM the app serves again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdoptedVm {
    pub id: String,
    /// True for a VM that an older start made: the meta records no identity. The app serves its
    /// streams, and stops it by name or through its control sockets, never by a signal.
    pub legacy: bool,
    pub agent: ProbeAnswer,
    pub vnc: ProbeAnswer,
}

/// What a probe of a survivor found. A timeout is `NotAnswering`, and it never means that the
/// guest is dead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeAnswer {
    /// The probe has not finished.
    Pending,
    Answering,
    NotAnswering,
    /// The app chose not to probe. A VNC probe of a tart guest that no keeper holds could crash
    /// the guest (ADR-0018), so the app does not make it.
    NotProbed,
}

/// A VM the app did not adopt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotAdoptedVm {
    pub id: String,
    /// A stable word: `process_gone`, `start_time_differs`, `boot_session_differs`,
    /// `no_boot_session` or `meta_unreadable`.
    pub reason: String,
    /// The reason in a sentence.
    pub detail: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_status_body_without_adoption_still_reads() {
        let body = r#"{
            "build": "1.0.0",
            "protocol_range": {"min": 1, "max": 1},
            "directories": {"state_dir": "/s", "data_dir": "/d"}
        }"#;
        let status: StatusResponse = serde_json::from_str(body).unwrap();
        assert_eq!(status.adoption, AdoptionStatus::default());
        assert!(!status.adoption.finished);
    }

    #[test]
    fn the_answers_serialise_as_snake_case_words() {
        let vm = AdoptedVm {
            id: "v".into(),
            legacy: true,
            agent: ProbeAnswer::NotAnswering,
            vnc: ProbeAnswer::NotProbed,
        };
        let json = serde_json::to_value(vm).unwrap();
        assert_eq!(json["agent"], "not_answering");
        assert_eq!(json["vnc"], "not_probed");
    }
}
