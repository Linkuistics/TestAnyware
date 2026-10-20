//! The checks that every request except `GET /v1/status` passes before it is served.

use testanyware_app_protocol::{
    decide, AppError, ClientHello, ConfigurationStatus, Decision, Directories, Peer, ProtocolRange,
};

/// What the app is, as the checks see it.
#[derive(Debug, Clone)]
pub struct AppIdentity {
    pub build: String,
    pub protocol_range: ProtocolRange,
    pub directories: Directories,
}

/// Check the protocol range first, then the directories. Return the protocol version that
/// the pair uses. A client that states no range, or no directories, fails like one that
/// states other values. `bundle_build` is the build of the bundle on disk, which decides
/// the remedy of a range that does not overlap. `configuration` states what the app made of
/// `config.rhai`, and a directory mismatch calls it to word its remedy.
pub fn check_request(
    app: &AppIdentity,
    hello: &ClientHello,
    bundle_build: Option<&str>,
    configuration: &dyn Fn() -> ConfigurationStatus,
) -> Result<u32, AppError> {
    let app_peer = Peer {
        build: Some(app.build.clone()),
        protocol_range: Some(app.protocol_range),
    };
    let client_peer = Peer::from(hello);
    if let Decision::Fail(remedy) = decide(&client_peer, &app_peer, bundle_build) {
        return Err(AppError::incompatible_protocol(
            &app_peer,
            &client_peer,
            remedy,
        ));
    }
    // `decide` fails exactly when the two ranges do not overlap, so the pair has a version.
    let version = hello
        .protocol_range
        .and_then(|range| app.protocol_range.highest_common(range))
        .expect("decide let a request through, so the ranges overlap");
    if hello.directories.as_ref() != Some(&app.directories) {
        return Err(AppError::incompatible_directories(
            &app.directories,
            hello.directories.as_ref(),
            &configuration(),
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

    fn none() -> ConfigurationStatus {
        ConfigurationStatus::default()
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
        assert_eq!(check_request(&app(), &hello, None, &none).unwrap(), 2);
    }

    #[test]
    fn disjoint_range_fails_with_cause_protocol() {
        let hello = ClientHello::new(ProtocolRange::new(3, 4), "2.0.0", dirs("a"));
        let err = check_request(&app(), &hello, None, &none).unwrap_err();
        assert_eq!(err.details["cause"], "protocol");
    }

    #[test]
    fn range_is_checked_before_directories() {
        let hello = ClientHello::new(ProtocolRange::new(3, 4), "2.0.0", dirs("other"));
        assert_eq!(
            check_request(&app(), &hello, None, &none)
                .unwrap_err()
                .details["cause"],
            "protocol"
        );
    }

    #[test]
    fn other_directories_fail_with_cause_directories() {
        let hello = ClientHello::new(ProtocolRange::new(1, 1), "1.0.0", dirs("other"));
        assert_eq!(
            check_request(&app(), &hello, None, &none)
                .unwrap_err()
                .details["cause"],
            "directories"
        );
    }

    #[test]
    fn a_silent_client_fails_on_the_range() {
        let err = check_request(&app(), &ClientHello::default(), None, &none).unwrap_err();
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
            check_request(&app(), &hello, None, &none)
                .unwrap_err()
                .details["cause"],
            "directories"
        );
    }

    #[test]
    fn a_range_that_does_not_overlap_carries_the_remedy_of_the_table() {
        // The app is 1.0.0 with range 1-2.
        let newer_cli = ClientHello::new(ProtocolRange::new(3, 4), "2.0.0", dirs("a"));
        let older_cli = ClientHello::new(ProtocolRange::new(0, 0), "0.1.0", dirs("a"));
        let cases = [
            // The app is the older side. A newer bundle means quit and open.
            (&newer_cli, Some("2.0.0"), "quit_and_open_app"),
            (&newer_cli, Some("1.0.0"), "upgrade_cask"),
            (&newer_cli, None, "upgrade_cask"),
            // The Host CLI is the older side.
            (&older_cli, Some("2.0.0"), "upgrade_host_cli"),
        ];
        for (hello, bundle, remedy) in cases {
            let err = check_request(&app(), hello, bundle, &none).unwrap_err();
            assert_eq!(err.details["remedy"], remedy, "{bundle:?}");
            assert!(err.remediation.is_some());
        }
    }
}
