//! The checks that every request except `GET /v1/status` passes before it is served.

use testanyware_app_protocol::{AppError, ClientHello, Directories, Peer, ProtocolRange};

/// What the app is, as the checks see it.
#[derive(Debug, Clone)]
pub struct AppIdentity {
    pub build: String,
    pub protocol_range: ProtocolRange,
    pub directories: Directories,
}

/// Check the protocol range first, then the directories. Return the protocol version that
/// the pair uses. A client that states no range, or no directories, fails like one that
/// states other values.
pub fn check_request(app: &AppIdentity, hello: &ClientHello) -> Result<u32, AppError> {
    let version = hello
        .protocol_range
        .and_then(|range| app.protocol_range.highest_common(range));
    let Some(version) = version else {
        let app_peer = Peer {
            build: Some(app.build.clone()),
            protocol_range: Some(app.protocol_range),
        };
        return Err(AppError::incompatible_protocol(
            &app_peer,
            &Peer::from(hello),
        ));
    };
    if hello.directories.as_ref() != Some(&app.directories) {
        return Err(AppError::incompatible_directories(
            &app.directories,
            hello.directories.as_ref(),
        ));
    }
    Ok(version)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dirs(s: &str) -> Directories {
        Directories {
            state_dir: format!("/{s}/state").into(),
            data_dir: format!("/{s}/data").into(),
        }
    }

    fn app() -> AppIdentity {
        AppIdentity {
            build: "1.0.0".into(),
            protocol_range: ProtocolRange::new(1, 2),
            directories: dirs("a"),
        }
    }

    #[test]
    fn matching_request_uses_the_highest_common_version() {
        let hello = ClientHello::new(ProtocolRange::new(2, 4), "1.1.0", dirs("a"));
        assert_eq!(check_request(&app(), &hello).unwrap(), 2);
    }

    #[test]
    fn disjoint_range_fails_with_cause_protocol() {
        let hello = ClientHello::new(ProtocolRange::new(3, 4), "2.0.0", dirs("a"));
        let err = check_request(&app(), &hello).unwrap_err();
        assert_eq!(err.details["cause"], "protocol");
    }

    #[test]
    fn range_is_checked_before_directories() {
        let hello = ClientHello::new(ProtocolRange::new(3, 4), "2.0.0", dirs("other"));
        assert_eq!(
            check_request(&app(), &hello).unwrap_err().details["cause"],
            "protocol"
        );
    }

    #[test]
    fn other_directories_fail_with_cause_directories() {
        let hello = ClientHello::new(ProtocolRange::new(1, 1), "1.0.0", dirs("other"));
        assert_eq!(
            check_request(&app(), &hello).unwrap_err().details["cause"],
            "directories"
        );
    }

    #[test]
    fn a_silent_client_fails_on_the_range() {
        let err = check_request(&app(), &ClientHello::default()).unwrap_err();
        assert_eq!(err.details["cause"], "protocol");
        assert_eq!(
            err.details["client"]["protocol_range"],
            serde_json::Value::Null
        );
    }

    #[test]
    fn a_client_without_directories_fails_on_the_directories() {
        let hello = ClientHello {
            directories: None,
            ..ClientHello::new(ProtocolRange::new(1, 1), "1", dirs("a"))
        };
        assert_eq!(
            check_request(&app(), &hello).unwrap_err().details["cause"],
            "directories"
        );
    }
}
