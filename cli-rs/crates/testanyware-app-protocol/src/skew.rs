//! What a pair of builds and protocol ranges means for a request (spec: "Version skew").
//!
//! [`decide`] is a pure function. The app's gate calls it to refuse a request whose ranges do
//! not overlap, and the Host CLI calls it on each response to find the builds that differ. The
//! remedy table of the spec is the [`Remedy`] type, so no caller words a remedy itself.

use std::cmp::Ordering;

use semver::Version;

use crate::error::Peer;

/// What a person does about a mismatch. A client never does it: it never restarts the app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Remedy {
    /// The running app is older, and the bundle on disk is newer.
    QuitAndOpenApp,
    /// The running app and its bundle are both older.
    UpgradeCask,
    /// The Host CLI is older.
    UpgradeHostCli,
}

impl Remedy {
    /// The remedy as a sentence, for the error and for the notice.
    pub fn text(self) -> &'static str {
        match self {
            Self::QuitAndOpenApp => "Quit TestAnyware from its menu, and open it again.",
            Self::UpgradeCask => "Run `brew upgrade --cask testanyware-app`.",
            Self::UpgradeHostCli => "Run `brew upgrade testanyware`.",
        }
    }

    /// A stable word for `details.remedy`, so that a reader need not parse the sentence.
    pub fn word(self) -> &'static str {
        match self {
            Self::QuitAndOpenApp => "quit_and_open_app",
            Self::UpgradeCask => "upgrade_cask",
            Self::UpgradeHostCli => "upgrade_host_cli",
        }
    }
}

/// The notice for a pair whose builds differ and whose ranges overlap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub cli_build: Option<String>,
    pub app_build: Option<String>,
    pub remedy: Remedy,
}

impl Notice {
    /// The one line for stderr. It names both builds and the remedy.
    pub fn line(&self) -> String {
        let build = |b: &Option<String>| b.clone().unwrap_or_else(|| "unknown".to_string());
        format!(
            "testanyware: notice: the Host CLI is build {} and the running TestAnyware app is \
             build {}. {}",
            build(&self.cli_build),
            build(&self.app_build),
            self.remedy.text()
        )
    }
}

/// What to do with a request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// The builds agree, and the ranges overlap.
    Serve,
    /// The ranges overlap, and the builds differ.
    ServeWithNotice(Notice),
    /// The ranges do not overlap.
    Fail(Remedy),
}

/// Decide from the Host CLI's side, the app's side and the build of the app bundle on disk.
///
/// A side that states no range counts as older than a side that does: it predates the protocol.
/// A bundle build that is unknown, or that does not read, never counts as newer than the
/// running app, so the remedy is then the cask upgrade, which quits and opens the app again.
pub fn decide(cli: &Peer, app: &Peer, bundle_build: Option<&str>) -> Decision {
    let overlap = match (cli.protocol_range, app.protocol_range) {
        (Some(c), Some(a)) => c.highest_common(a).is_some(),
        _ => false,
    };
    if !overlap {
        let cli_older = match (cli.protocol_range, app.protocol_range) {
            (Some(c), Some(a)) => c.max < a.min,
            (None, _) => true,
            (Some(_), None) => false,
        };
        return Decision::Fail(if cli_older {
            Remedy::UpgradeHostCli
        } else {
            app_remedy(app.build.as_deref(), bundle_build)
        });
    }
    if cli.build.is_some() && cli.build == app.build {
        return Decision::Serve;
    }
    let remedy = if is_older(cli.build.as_deref(), app.build.as_deref()) {
        Remedy::UpgradeHostCli
    } else {
        app_remedy(app.build.as_deref(), bundle_build)
    };
    Decision::ServeWithNotice(Notice {
        cli_build: cli.build.clone(),
        app_build: app.build.clone(),
        remedy,
    })
}

/// The remedy for a request that needs protocol version `needed`, when the app's range ends below
/// it, or `None` when the app can serve it (spec "admission-queue": "The protocol": "Version
/// skew"). An app that states no range predates the protocol, so it is below every version. The
/// app is then the side to change, and the remedy is the one of an older app.
pub fn needs_version(needed: u32, app: &Peer, bundle_build: Option<&str>) -> Option<Remedy> {
    let serves = app.protocol_range.is_some_and(|range| range.max >= needed);
    (!serves).then(|| app_remedy(app.build.as_deref(), bundle_build))
}

/// Whether the running app is older than its bundle on disk. `doctor` and the status response
/// report it.
pub fn older_than_bundle(app_build: &str, bundle_build: Option<&str>) -> bool {
    is_older(Some(app_build), bundle_build)
}

/// The remedy when the app is the side to change.
fn app_remedy(app_build: Option<&str>, bundle_build: Option<&str>) -> Remedy {
    if is_older(app_build, bundle_build) {
        Remedy::QuitAndOpenApp
    } else {
        Remedy::UpgradeCask
    }
}

/// Whether `a` is strictly older than `b`. A build that is missing or is no version never is,
/// and nothing is older than it.
fn is_older(a: Option<&str>, b: Option<&str>) -> bool {
    let parse = |s: Option<&str>| s.and_then(|s| Version::parse(s).ok());
    matches!((parse(a), parse(b)), (Some(a), Some(b)) if a.cmp(&b) == Ordering::Less)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::version::ProtocolRange;

    fn peer(build: &str, min: u32, max: u32) -> Peer {
        Peer {
            build: Some(build.into()),
            protocol_range: Some(ProtocolRange::new(min, max)),
        }
    }

    // ---- the rows of the table, for ranges that do not overlap ----

    #[test]
    fn an_older_app_with_a_newer_bundle_gets_quit_and_open() {
        let d = decide(&peer("2.0.0", 2, 2), &peer("1.0.0", 1, 1), Some("2.0.0"));
        assert_eq!(d, Decision::Fail(Remedy::QuitAndOpenApp));
    }

    #[test]
    fn an_older_app_with_an_older_bundle_gets_the_cask_upgrade() {
        let d = decide(&peer("2.0.0", 2, 2), &peer("1.0.0", 1, 1), Some("1.0.0"));
        assert_eq!(d, Decision::Fail(Remedy::UpgradeCask));
        let d = decide(&peer("2.0.0", 2, 2), &peer("1.0.0", 1, 1), Some("1.5.0"));
        assert_eq!(d, Decision::Fail(Remedy::QuitAndOpenApp), "a newer bundle");
    }

    #[test]
    fn an_older_host_cli_gets_the_cli_upgrade_whatever_the_bundle() {
        for bundle in [None, Some("1.0.0"), Some("9.0.0")] {
            let d = decide(&peer("1.0.0", 1, 1), &peer("2.0.0", 2, 3), bundle);
            assert_eq!(d, Decision::Fail(Remedy::UpgradeHostCli), "{bundle:?}");
        }
    }

    #[test]
    fn a_bundle_that_is_unknown_or_unreadable_never_counts_as_newer() {
        for bundle in [None, Some("not a version")] {
            let d = decide(&peer("2.0.0", 2, 2), &peer("1.0.0", 1, 1), bundle);
            assert_eq!(d, Decision::Fail(Remedy::UpgradeCask), "{bundle:?}");
        }
    }

    #[test]
    fn a_client_that_states_no_range_is_the_older_side() {
        let silent = Peer {
            build: None,
            protocol_range: None,
        };
        let d = decide(&silent, &peer("1.0.0", 1, 1), Some("9.0.0"));
        assert_eq!(d, Decision::Fail(Remedy::UpgradeHostCli));
    }

    #[test]
    fn an_app_that_states_no_range_is_the_older_side() {
        let silent = Peer {
            build: None,
            protocol_range: None,
        };
        let d = decide(&peer("1.0.0", 1, 1), &silent, Some("1.0.0"));
        assert_eq!(d, Decision::Fail(Remedy::UpgradeCask));
    }

    // ---- a request that needs a version ----

    #[test]
    fn an_app_whose_range_ends_below_the_version_cannot_serve_the_request() {
        let app = peer("1.0.0", 1, 1);
        assert_eq!(
            needs_version(2, &app, Some("2.0.0")),
            Some(Remedy::QuitAndOpenApp)
        );
        assert_eq!(needs_version(2, &app, Some("1.0.0")), Some(Remedy::UpgradeCask));
        assert_eq!(needs_version(2, &app, None), Some(Remedy::UpgradeCask));
    }

    #[test]
    fn an_app_whose_range_reaches_the_version_serves_the_request() {
        assert_eq!(needs_version(2, &peer("2.0.0", 1, 2), None), None);
        assert_eq!(needs_version(2, &peer("2.0.0", 2, 3), None), None);
        assert_eq!(needs_version(1, &peer("1.0.0", 1, 1), None), None);
    }

    #[test]
    fn an_app_that_states_no_range_cannot_serve_a_request_of_any_version() {
        let silent = Peer {
            build: None,
            protocol_range: None,
        };
        assert_eq!(needs_version(1, &silent, None), Some(Remedy::UpgradeCask));
    }

    // ---- ranges that overlap ----

    #[test]
    fn equal_builds_with_overlapping_ranges_serve_without_a_notice() {
        let d = decide(&peer("1.0.0", 1, 1), &peer("1.0.0", 1, 2), Some("2.0.0"));
        assert_eq!(d, Decision::Serve);
    }

    fn notice(d: Decision) -> Notice {
        match d {
            Decision::ServeWithNotice(n) => n,
            other => panic!("expected a notice, got {other:?}"),
        }
    }

    #[test]
    fn builds_that_differ_with_overlapping_ranges_give_a_notice_with_both_builds() {
        let n = notice(decide(&peer("1.1.0", 1, 1), &peer("1.0.0", 1, 1), None));
        assert_eq!(n.cli_build.as_deref(), Some("1.1.0"));
        assert_eq!(n.app_build.as_deref(), Some("1.0.0"));
        let line = n.line();
        assert!(line.contains("1.1.0") && line.contains("1.0.0"), "{line}");
        assert!(!line.contains('\n'), "{line:?}");
    }

    #[test]
    fn the_notice_names_the_remedy_of_the_older_side() {
        let app_older_bundle_newer = notice(decide(
            &peer("1.1.0", 1, 1),
            &peer("1.0.0", 1, 1),
            Some("1.1.0"),
        ));
        assert_eq!(app_older_bundle_newer.remedy, Remedy::QuitAndOpenApp);
        let app_and_bundle_older = notice(decide(
            &peer("1.1.0", 1, 1),
            &peer("1.0.0", 1, 1),
            Some("1.0.0"),
        ));
        assert_eq!(app_and_bundle_older.remedy, Remedy::UpgradeCask);
        let cli_older = notice(decide(
            &peer("1.0.0", 1, 1),
            &peer("1.1.0", 1, 1),
            Some("1.1.0"),
        ));
        assert_eq!(cli_older.remedy, Remedy::UpgradeHostCli);
        assert!(cli_older.line().contains("brew upgrade testanyware"));
    }

    #[test]
    fn a_pre_release_is_older_than_its_release() {
        let n = notice(decide(
            &peer("1.0.0", 1, 1),
            &peer("1.0.0-rc.1", 1, 1),
            None,
        ));
        assert_eq!(n.remedy, Remedy::UpgradeCask);
        let n = notice(decide(
            &peer("1.0.0-rc.1", 1, 1),
            &peer("1.0.0", 1, 1),
            None,
        ));
        assert_eq!(n.remedy, Remedy::UpgradeHostCli);
    }

    #[test]
    fn builds_that_do_not_read_still_give_a_notice_and_the_app_remedy() {
        let n = notice(decide(&peer("1.0.0", 1, 1), &peer("nightly", 1, 1), None));
        assert_eq!(n.remedy, Remedy::UpgradeCask);
    }

    #[test]
    fn an_app_that_states_no_build_gives_a_notice_that_says_unknown() {
        let app = Peer {
            build: None,
            protocol_range: Some(ProtocolRange::new(1, 1)),
        };
        let n = notice(decide(&peer("1.0.0", 1, 1), &app, None));
        assert!(n.line().contains("build unknown"), "{}", n.line());
    }

    // ---- the status field ----

    #[test]
    fn the_app_is_older_than_its_bundle_only_when_both_builds_read_and_the_bundle_is_newer() {
        assert!(older_than_bundle("1.0.0", Some("1.0.1")));
        assert!(!older_than_bundle("1.0.1", Some("1.0.1")));
        assert!(!older_than_bundle("1.0.1", Some("1.0.0")));
        assert!(!older_than_bundle("1.0.0", None));
        assert!(!older_than_bundle("1.0.0", Some("junk")));
    }

    #[test]
    fn each_remedy_has_a_sentence_and_a_word() {
        use Remedy::*;
        assert_eq!(
            QuitAndOpenApp.text(),
            "Quit TestAnyware from its menu, and open it again."
        );
        assert_eq!(
            UpgradeCask.text(),
            "Run `brew upgrade --cask testanyware-app`."
        );
        assert_eq!(UpgradeHostCli.text(), "Run `brew upgrade testanyware`.");
        let words = [QuitAndOpenApp, UpgradeCask, UpgradeHostCli].map(Remedy::word);
        assert_eq!(
            words,
            ["quit_and_open_app", "upgrade_cask", "upgrade_host_cli"]
        );
    }
}
