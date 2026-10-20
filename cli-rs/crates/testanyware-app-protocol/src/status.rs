//! The body of `GET /v1/status`.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::AppError;
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
    /// The build of the app bundle on disk now, or `None` when the app runs from no bundle or
    /// cannot read its `Info.plist`. A cask upgrade replaces the bundle under a running app.
    #[serde(default)]
    pub bundle_build: Option<String>,
    /// True when the running app is older than its bundle on disk, so that quitting and opening
    /// TestAnyware gives the newer build (spec: "Version skew").
    #[serde(default)]
    pub older_than_bundle: bool,
    /// What the app did with each VM it found when it started (spec: "Lifecycle and adoption").
    #[serde(default)]
    pub adoption: AdoptionStatus,
    /// What the app made of `config.rhai` when it started (spec: "Configuration").
    #[serde(default)]
    pub configuration: ConfigurationStatus,
    /// The executable of the running app. `None` when the app cannot name it.
    #[serde(default)]
    pub executable: Option<PathBuf>,
    /// What the app last learned about the Local Network grant (spec: "The Local Network
    /// diagnosis").
    #[serde(default)]
    pub local_network: LocalNetworkStatus,
    /// The status of the app's login item, as macOS reports it.
    #[serde(default)]
    pub login_item: LoginItemState,
}

/// What the app last learned about the Local Network grant.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GrantState {
    /// No dial of the kind that tells has happened yet.
    #[default]
    NotYetSeen,
    Allowed,
    Denied,
}

/// The app's observation of the grant, with the dial that made it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalNetworkStatus {
    pub state: GrantState,
    /// The address of the dial, as `host:port`. `None` before the first observation.
    pub destination: Option<String>,
    /// When the dial happened, in seconds since the Unix epoch. `None` before the first
    /// observation.
    pub observed_at: Option<u64>,
}

/// The status of `SMAppService.mainApp`, as macOS reports it.
/// https://developer.apple.com/documentation/servicemanagement/smappservice/status-swift.enum
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoginItemState {
    /// The app has no login item to ask, as in its test mode. A reader that sees this reports
    /// nothing about the login item.
    #[default]
    Unobserved,
    NotRegistered,
    Enabled,
    /// The app asked to start at login, and a person has yet to allow it in System Settings.
    RequiresApproval,
    /// macOS knows no login item for this bundle. It counts as not registered.
    NotFound,
}

/// What a run of the app made of `config.rhai`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigurationState {
    /// The run reads no file: `TESTANYWARE_CONFIG=none`, or no home directory to find it in.
    #[default]
    Disabled,
    /// The file does not exist. It is optional.
    Absent,
    /// The file evaluated, and the app uses its settings.
    Loaded,
    /// The file failed. The app runs with its built-in defaults.
    Failed,
}

/// The app's evaluation of `config.rhai`, which `doctor` shows. The app evaluates the file once,
/// when it starts, so this describes that moment and not the file as it is now. The one
/// exception is `changed_since_evaluation`, which compares the two.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigurationStatus {
    pub state: ConfigurationState,
    /// The file the app looked for. `None` when the run reads no file.
    pub file: Option<PathBuf>,
    /// When the app evaluated the file, in seconds since the Unix epoch. `None` when it read none.
    pub evaluated_at: Option<u64>,
    /// True when the file on disk is not the one the app evaluated: it was edited, it appeared
    /// or it was removed. Only a restart of the app reads it again.
    pub changed_since_evaluation: bool,
    /// The `CONFIG_INVALID` object of a file that failed. It is the object that the Host CLI
    /// reports for its own evaluation of the same file.
    pub error: Option<AppError>,
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
        assert_eq!(status.bundle_build, None);
        assert!(!status.older_than_bundle);
        assert!(!status.adoption.finished);
        assert_eq!(status.configuration, ConfigurationStatus::default());
        assert_eq!(status.executable, None);
        assert_eq!(status.local_network, LocalNetworkStatus::default());
        assert_eq!(status.login_item, LoginItemState::Unobserved);
    }

    #[test]
    fn the_grant_and_the_login_item_serialise_as_snake_case_words() {
        let status = LocalNetworkStatus {
            state: GrantState::NotYetSeen,
            ..LocalNetworkStatus::default()
        };
        assert_eq!(serde_json::to_value(status).unwrap()["state"], "not_yet_seen");
        let item = serde_json::to_value(LoginItemState::RequiresApproval).unwrap();
        assert_eq!(item, "requires_approval");
    }

    #[test]
    fn the_states_of_the_configuration_serialise_as_snake_case_words() {
        let status = ConfigurationStatus {
            state: ConfigurationState::Failed,
            ..ConfigurationStatus::default()
        };
        assert_eq!(serde_json::to_value(status).unwrap()["state"], "failed");
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
