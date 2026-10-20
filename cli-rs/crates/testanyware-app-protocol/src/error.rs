//! The error object of the app. It uses the Host CLI's own vocabulary:
//! `code`, `message`, `remediation` and `details`.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::hello::{ClientHello, Directories};
use crate::version::ProtocolRange;

/// Code for a request that the two sides cannot serve together.
pub const APP_INCOMPATIBLE: &str = "APP_INCOMPATIBLE";

/// An error that the app sends before an upgrade, as a JSON object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

    /// The two protocol ranges share no version. The error names both builds and both
    /// ranges. The remedy table belongs to the version-skew leaf, so there is no remedy here.
    pub fn incompatible_protocol(app: &Peer, client: &Peer) -> Self {
        Self::new(
            APP_INCOMPATIBLE,
            format!(
                "The TestAnyware app ({}) and the Host CLI ({}) share no protocol version.",
                describe(app),
                describe(client)
            ),
            None,
            json!({ "cause": "protocol", "app": app, "client": client }),
        )
    }

    /// The Host CLI resolved other directories than the app did. The error names both
    /// pairs. The remedy names each environment variable that differs.
    pub fn incompatible_directories(app: &Directories, client: Option<&Directories>) -> Self {
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
        let mut differing = Vec::new();
        if client.map_or(true, |c| c.state_dir != app.state_dir) {
            differing.push("XDG_STATE_HOME");
        }
        if client.map_or(true, |c| c.data_dir != app.data_dir) {
            differing.push("XDG_DATA_HOME");
        }
        let remediation = format!(
            "Make the Host CLI and the app resolve the same directories. Set or unset {}.",
            differing.join(" and ")
        );
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
    fn protocol_error_names_both_builds_and_both_ranges() {
        let err = AppError::incompatible_protocol(&peer("1.0.0", 1, 1), &peer("2.0.0", 2, 3));
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
    }

    #[test]
    fn protocol_error_for_a_silent_client_has_null_fields() {
        let err = AppError::incompatible_protocol(
            &peer("1.0.0", 1, 1),
            &Peer::from(&ClientHello::default()),
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
        let err = AppError::incompatible_directories(&app, Some(&client));
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
