//! The error object of the app. It uses the Host CLI's own vocabulary:
//! `code`, `message`, `remediation` and `details`.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::hello::{ClientHello, Directories};
use crate::skew::Remedy;
use crate::status::{ConfigurationState, ConfigurationStatus};
use crate::version::ProtocolRange;

/// Code for a request that the two sides cannot serve together.
pub const APP_INCOMPATIBLE: &str = "APP_INCOMPATIBLE";

/// Code for a `config.rhai` that fails to evaluate or names an unknown key. The Host CLI makes
/// this error itself, and the app reports the same object for its own evaluation.
pub const CONFIG_INVALID: &str = "CONFIG_INVALID";

/// Code for a client that cannot reach the app on its socket. The Host CLI makes this error
/// itself, because no app answered.
pub const APP_UNREACHABLE: &str = "APP_UNREACHABLE";

/// Why a client cannot reach the app. `details.cause` of an `APP_UNREACHABLE` error holds
/// the name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnreachableCause {
    /// Nothing answers on the socket: the file is missing or refuses the connection.
    NotRunning,
    /// The owner or the mode of the socket directory is unsafe.
    UnsafeSocketDirectory,
    /// The app accepted the connection and closed it before it answered.
    Quitting,
    /// The socket path has more bytes than a socket address holds.
    SocketPathTooLong,
    /// Another failure of the socket or of the connection. `details.errno` or the message
    /// names it.
    SocketError,
}

impl UnreachableCause {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotRunning => "not_running",
            Self::UnsafeSocketDirectory => "unsafe_socket_directory",
            Self::Quitting => "quitting",
            Self::SocketPathTooLong => "socket_path_too_long",
            Self::SocketError => "socket_error",
        }
    }
}

/// Code for a macOS host that has no TestAnyware app bundle. The Host CLI makes this error
/// itself, from a socket that nothing answers and a bundle that no search path holds.
pub const APP_NOT_INSTALLED: &str = "APP_NOT_INSTALLED";

/// The one-time setup, as the remedy of a missing or silent app (spec: "How the app starts").
/// A person does it. No client does.
pub const SETUP_REMEDY: &str = "Set up the TestAnyware app once, as a person. \
1. Install the cask with `brew install --cask linkuistics/taps/testanyware-app`. The release provides the cask. \
2. Open TestAnyware from /Applications. \
3. Answer the \"downloaded from the Internet\" dialog with Open. \
4. Turn on \"Start at Login\" in the app's menu. \
5. Allow the Local Network alert when it appears. \
The Host CLI never starts the app.";

/// The remedy of a Local Network denial: the app holds the grant, and a person gives it once
/// (spec: "The Local Network diagnosis"). The app words its stream errors with it, and `doctor`
/// words the failing observation with it.
pub const LOCAL_NETWORK_REMEDY: &str = "Allow TestAnyware in System Settings, Privacy & Security, \
     Local Network. A person does this once, and again when an upgrade of TestAnyware asks.";

/// An error that the app sends before an upgrade, as a JSON object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppError {
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remediation: Option<String>,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub details: Value,
}

/// One side of a mismatch, as the error object names it. A field is `null` when the
/// side did not state it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Peer {
    pub build: Option<String>,
    pub protocol_range: Option<ProtocolRange>,
}

impl AppError {
    pub fn new(
        code: &str,
        message: impl Into<String>,
        remediation: Option<String>,
        details: Value,
    ) -> Self {
        Self {
            code: code.to_string(),
            message: message.into(),
            remediation,
            details,
        }
    }

    /// `config.rhai` fails. The error names the file, the line (`null` for an error that no line
    /// holds) and the message of the evaluator, and the remedy.
    pub fn config_invalid(file: &std::path::Path, line: Option<usize>, message: &str) -> Self {
        let place = match line {
            Some(line) => format!("{}:{line}", file.display()),
            None => file.display().to_string(),
        };
        Self::new(
            CONFIG_INVALID,
            format!("{place}: {message}"),
            Some(format!(
                "Fix {} or remove it. To run once without it, set TESTANYWARE_CONFIG=none. \
                 `testanyware doctor` runs with a file that fails and reports this error.",
                file.display()
            )),
            json!({ "file": file, "line": line, "message": message }),
        )
    }

    /// The client cannot reach the app. The error names the socket and the cause.
    pub fn unreachable(
        cause: UnreachableCause,
        socket: &std::path::Path,
        message: impl Into<String>,
        remediation: impl Into<String>,
    ) -> Self {
        Self::new(
            APP_UNREACHABLE,
            message,
            Some(remediation.into()),
            json!({ "cause": cause.as_str(), "socket": socket }),
        )
    }

    /// No TestAnyware app bundle exists on the host, and nothing answers on `socket`. The
    /// error names the places that the Host CLI searched.
    pub fn not_installed(socket: &std::path::Path, searched: &[std::path::PathBuf]) -> Self {
        Self::new(
            APP_NOT_INSTALLED,
            format!(
                "The TestAnyware app is not installed, and nothing answers on {}. \
                 Every guest connection on a macOS host goes through the app.",
                socket.display()
            ),
            Some(SETUP_REMEDY.to_string()),
            json!({ "socket": socket, "searched": searched }),
        )
    }

    /// Add one field to `details`.
    pub fn with_detail(mut self, key: &str, value: impl Serialize) -> Self {
        if let Value::Object(map) = &mut self.details {
            map.insert(
                key.to_string(),
                serde_json::to_value(value).unwrap_or(Value::Null),
            );
        }
        self
    }

    /// The two protocol ranges share no version. The error names both builds and both ranges,
    /// and one remedy of the table in the spec ("Version skew").
    pub fn incompatible_protocol(app: &Peer, client: &Peer, remedy: Remedy) -> Self {
        Self::new(
            APP_INCOMPATIBLE,
            format!(
                "The TestAnyware app ({}) and the Host CLI ({}) share no protocol version.",
                describe(app),
                describe(client)
            ),
            Some(remedy.text().to_string()),
            json!({
                "cause": "protocol",
                "remedy": remedy.word(),
                "app": app,
                "client": client,
            }),
        )
    }

    /// The Host CLI resolved other directories than the app did. The error names both pairs.
    /// The remedy names each setting that differs, in its variable and in `config.rhai`, and
    /// then what the app made of the file: a file that fails, or a file that changed after the
    /// app evaluated it (spec: "Configuration").
    pub fn incompatible_directories(
        app: &Directories,
        client: Option<&Directories>,
        config: &ConfigurationStatus,
    ) -> Self {
        let message = match client {
            Some(c) => format!(
                "The Host CLI resolved state directory {} and data directory {}. \
                 The TestAnyware app uses state directory {} and data directory {}.",
                c.state_dir.display(),
                c.data_dir.display(),
                app.state_dir.display(),
                app.data_dir.display()
            ),
            None => format!(
                "The request did not state its directories. The TestAnyware app uses state \
                 directory {} and data directory {}.",
                app.state_dir.display(),
                app.data_dir.display()
            ),
        };
        let state_differs = client.map_or(true, |c| c.state_dir != app.state_dir);
        let data_differs = client.map_or(true, |c| c.data_dir != app.data_dir);
        let mut variables = Vec::new();
        let mut settings = Vec::new();
        if state_differs {
            variables.push("XDG_STATE_HOME");
            settings.push("`state_dir`");
        }
        if data_differs {
            variables.push("XDG_DATA_HOME");
            settings.push("`data_dir`");
        }
        let file = config
            .file
            .as_ref()
            .map_or_else(|| "config.rhai".to_string(), |f| f.display().to_string());
        let mut remediation = format!(
            "Make the Host CLI and the app resolve the same directories. \
             Set or unset {}, or name {} in config.rhai.",
            variables.join(" and "),
            settings.join(" and "),
        );
        match (&config.state, &config.error) {
            (ConfigurationState::Failed, error) => {
                let why = error
                    .as_ref()
                    .and_then(|e| e.details["message"].as_str())
                    .map_or(String::new(), |m| format!(": {m}"));
                remediation.push_str(&format!(
                    " The app runs with its built-in directories, because {file} fails{why}. \
                     Fix the file, then quit and open TestAnyware."
                ));
            }
            _ if config.changed_since_evaluation => remediation.push_str(&format!(
                " The app evaluated an older {file}. \
                 Quit and open TestAnyware, so that it reads the file as it is now."
            )),
            _ => {}
        }
        Self::new(
            APP_INCOMPATIBLE,
            message,
            Some(remediation),
            json!({ "cause": "directories", "app": app, "client": client }),
        )
    }
}

/// The two sides of a request, for the app's compatibility check.
impl From<&ClientHello> for Peer {
    fn from(hello: &ClientHello) -> Self {
        Peer {
            build: hello.build.clone(),
            protocol_range: hello.protocol_range,
        }
    }
}

fn describe(peer: &Peer) -> String {
    let build = peer.build.as_deref().unwrap_or("unknown build");
    match peer.protocol_range {
        Some(range) => format!("build {build}, protocol {range}"),
        None => format!("build {build}, protocol range not stated"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peer(build: &str, min: u32, max: u32) -> Peer {
        Peer {
            build: Some(build.into()),
            protocol_range: Some(ProtocolRange::new(min, max)),
        }
    }

    #[test]
    fn config_invalid_names_the_file_the_line_and_the_message() {
        let file = std::path::Path::new("/h/.config/testanyware/config.rhai");
        let err = AppError::config_invalid(file, Some(3), "syntax error: Unexpected ';'");
        assert_eq!(err.code, "CONFIG_INVALID");
        assert_eq!(
            err.message,
            "/h/.config/testanyware/config.rhai:3: syntax error: Unexpected ';'"
        );
        assert_eq!(err.details["file"], "/h/.config/testanyware/config.rhai");
        assert_eq!(err.details["line"], 3);
        assert_eq!(err.details["message"], "syntax error: Unexpected ';'");
        let remedy = err.remediation.unwrap();
        assert!(remedy.contains("TESTANYWARE_CONFIG=none"), "{remedy}");
        assert!(remedy.contains("config.rhai"), "{remedy}");
    }

    #[test]
    fn config_invalid_without_a_line_has_a_null_line() {
        let err =
            AppError::config_invalid(std::path::Path::new("/c.rhai"), None, "unknown key `x`");
        assert_eq!(err.message, "/c.rhai: unknown key `x`");
        assert!(err.details["line"].is_null());
    }

    #[test]
    fn protocol_error_names_both_builds_and_both_ranges() {
        let err = AppError::incompatible_protocol(
            &peer("1.0.0", 1, 1),
            &peer("2.0.0", 2, 3),
            Remedy::UpgradeCask,
        );
        assert_eq!(err.code, "APP_INCOMPATIBLE");
        assert_eq!(err.details["cause"], "protocol");
        assert_eq!(err.details["app"]["build"], "1.0.0");
        assert_eq!(
            err.details["app"]["protocol_range"],
            json!({ "min": 1, "max": 1 })
        );
        assert_eq!(err.details["client"]["build"], "2.0.0");
        assert_eq!(
            err.details["client"]["protocol_range"],
            json!({ "min": 2, "max": 3 })
        );
        for needle in ["1.0.0", "2.0.0", "1-1", "2-3"] {
            assert!(
                err.message.contains(needle),
                "{needle} missing from {:?}",
                err.message
            );
        }
        assert_eq!(err.details["remedy"], "upgrade_cask");
        assert_eq!(
            err.remediation.as_deref(),
            Some("Run `brew upgrade --cask testanyware-app`.")
        );
    }

    #[test]
    fn protocol_error_for_a_silent_client_has_null_fields() {
        let err = AppError::incompatible_protocol(
            &peer("1.0.0", 1, 1),
            &Peer::from(&ClientHello::default()),
            Remedy::UpgradeHostCli,
        );
        assert_eq!(err.details["client"]["protocol_range"], Value::Null);
        assert_eq!(err.details["client"]["build"], Value::Null);
    }

    #[test]
    fn directories_error_names_both_pairs_and_the_differing_variable() {
        let app = Directories {
            state_dir: "/a/state".into(),
            data_dir: "/a/data".into(),
        };
        let client = Directories {
            state_dir: "/c/state".into(),
            data_dir: "/a/data".into(),
        };
        let err = AppError::incompatible_directories(
            &app,
            Some(&client),
            &ConfigurationStatus::default(),
        );
        assert_eq!(err.details["cause"], "directories");
        assert_eq!(err.details["app"]["state_dir"], "/a/state");
        assert_eq!(err.details["client"]["state_dir"], "/c/state");
        for needle in ["/a/state", "/a/data", "/c/state"] {
            assert!(
                err.message.contains(needle),
                "{needle} missing from {:?}",
                err.message
            );
        }
        let remedy = err.remediation.unwrap();
        assert!(remedy.contains("XDG_STATE_HOME"));
        assert!(!remedy.contains("XDG_DATA_HOME"));
        // The remedy also names the file's own setting, and only the one that differs.
        assert!(remedy.contains("config.rhai"), "{remedy}");
        assert!(
            remedy.contains("`state_dir`") && !remedy.contains("`data_dir`"),
            "{remedy}"
        );
    }

    fn mismatch() -> (Directories, Directories) {
        let app = Directories {
            state_dir: "/a/state".into(),
            data_dir: "/a/data".into(),
        };
        let client = Directories {
            state_dir: "/c/state".into(),
            data_dir: "/c/data".into(),
        };
        (app, client)
    }

    #[test]
    fn directories_remedy_names_a_file_that_fails_and_why_the_app_ignores_it() {
        let (app, client) = mismatch();
        let file = std::path::Path::new("/h/.config/testanyware/config.rhai");
        let config = ConfigurationStatus {
            state: ConfigurationState::Failed,
            file: Some(file.to_path_buf()),
            error: Some(AppError::config_invalid(file, Some(2), "syntax error")),
            ..ConfigurationStatus::default()
        };
        let remedy = AppError::incompatible_directories(&app, Some(&client), &config)
            .remediation
            .unwrap();
        assert!(
            remedy.contains("XDG_STATE_HOME and XDG_DATA_HOME"),
            "{remedy}"
        );
        assert!(
            remedy.contains("/h/.config/testanyware/config.rhai fails: syntax error"),
            "{remedy}"
        );
        assert!(remedy.contains("quit and open TestAnyware"), "{remedy}");
    }

    #[test]
    fn directories_remedy_asks_for_a_restart_when_the_app_evaluated_an_older_file() {
        let (app, client) = mismatch();
        let config = ConfigurationStatus {
            state: ConfigurationState::Loaded,
            file: Some("/h/config.rhai".into()),
            changed_since_evaluation: true,
            ..ConfigurationStatus::default()
        };
        let remedy = AppError::incompatible_directories(&app, Some(&client), &config)
            .remediation
            .unwrap();
        assert!(
            remedy.contains("evaluated an older /h/config.rhai"),
            "{remedy}"
        );
        assert!(remedy.contains("Quit and open TestAnyware"), "{remedy}");
        // A file that has not changed needs no restart.
        let same = ConfigurationStatus {
            changed_since_evaluation: false,
            ..config
        };
        let remedy = AppError::incompatible_directories(&app, Some(&client), &same)
            .remediation
            .unwrap();
        assert!(!remedy.contains("Quit and open"), "{remedy}");
    }

    #[test]
    fn unreachable_names_the_cause_and_the_socket() {
        let err = AppError::unreachable(
            UnreachableCause::NotRunning,
            std::path::Path::new("/s/testanyware.sock"),
            "Nothing answers.",
            "Start the app.",
        )
        .with_detail("errno", 2);
        assert_eq!(err.code, "APP_UNREACHABLE");
        assert_eq!(err.details["cause"], "not_running");
        assert_eq!(err.details["socket"], "/s/testanyware.sock");
        assert_eq!(err.details["errno"], 2);
        assert_eq!(err.remediation.as_deref(), Some("Start the app."));
    }

    #[test]
    fn each_unreachable_cause_has_its_spec_name() {
        use UnreachableCause::*;
        let names: Vec<_> = [
            NotRunning,
            UnsafeSocketDirectory,
            Quitting,
            SocketPathTooLong,
            SocketError,
        ]
        .map(UnreachableCause::as_str)
        .into();
        assert_eq!(
            names,
            [
                "not_running",
                "unsafe_socket_directory",
                "quitting",
                "socket_path_too_long",
                "socket_error"
            ]
        );
    }

    #[test]
    fn error_object_serialises_without_empty_fields() {
        let err = AppError::new("X", "m", None, Value::Null);
        assert_eq!(
            serde_json::to_value(&err).unwrap(),
            json!({ "code": "X", "message": "m" })
        );
    }
}
