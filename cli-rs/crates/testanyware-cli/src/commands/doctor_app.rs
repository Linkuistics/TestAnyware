//! The `app` check of `testanyware doctor` (spec: "What the commands report about the app").
//!
//! On a macOS host every guest connection goes through the TestAnyware app, so `doctor` says
//! whether the app is installed, whether it runs, and whether the pair works: the login item,
//! the Local Network grant, the configuration, the adoption of VMs, the directories and the
//! versions. Each failing item carries a remedy. On a Linux or Windows host the check reports
//! that it does not apply.
//!
//! [`AppCheck::classify`] is the pure core. It takes what the probes found and decides every
//! item, so the unit tests need no app. [`AppCheck::gather`] is the runtime layer: it looks for
//! the bundle and reads `GET /v1/status`, a request that changes nothing in the app and that the
//! app answers for any protocol range and any directories.
//!
//! A mismatch of directories or of protocol is a failing item of this check. It is never an
//! error envelope, because `doctor` exists to report it.

use std::path::PathBuf;

use serde::Serialize;
use serde_json::{json, Map, Value};
use testanyware_app_protocol::version::SUPPORTED_RANGE;
use testanyware_app_protocol::{
    decide, AppError, ConfigurationState, Decision, Directories, GrantState, LoginItemState, Peer, Remedy,
    StatusResponse, LOCAL_NETWORK_REMEDY, SETUP_REMEDY,
};

use crate::app_route::{self, InstalledApp};
use crate::resolve::{app_socket_for_host, EnvProvider};

/// The remedy of a login item that is off. A person turns it on in the app's menu.
const START_AT_LOGIN_REMEDY: &str = "Turn on \"Start at Login\" in the TestAnyware menu.";

/// The remedy of a login item that macOS holds back until a person allows it.
const LOGIN_APPROVAL_REMEDY: &str =
    "Choose \"Open Login Items Settings…\" in the TestAnyware menu, and allow TestAnyware there.";

const ALLOW_NETWORK_AT_FIRST_GUEST: &str = "Reach a guest through the app, and allow the Local \
     Network alert when it appears. A person does this once.";

/// How one item came out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemStatus {
    Pass,
    /// Advisory. It never flips `ok`.
    Warn,
    Fail,
    /// Not checked: the host or the state of the app leaves nothing to check.
    Skip,
}

impl ItemStatus {
    fn word(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Warn => "warn",
            Self::Fail => "fail",
            Self::Skip => "skip",
        }
    }

    fn mark(self) -> &'static str {
        match self {
            Self::Pass => "✓",
            Self::Warn => "!",
            Self::Fail => "✗",
            Self::Skip => "-",
        }
    }
}

/// One line of the check: a verdict, the facts behind it and, when it is not a pass, a remedy.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub status: ItemStatus,
    /// A stable word, so that a reader need not parse the summary.
    pub verdict: &'static str,
    pub summary: String,
    fields: Map<String, Value>,
    pub remedy: Option<String>,
}

impl Item {
    fn new(status: ItemStatus, verdict: &'static str, summary: impl Into<String>) -> Self {
        Self {
            status,
            verdict,
            summary: summary.into(),
            fields: Map::new(),
            remedy: None,
        }
    }

    fn field(mut self, key: &str, value: impl Serialize) -> Self {
        let value = serde_json::to_value(value).expect("an item field serialises");
        self.fields.insert(key.to_string(), value);
        self
    }

    fn remedy(mut self, text: impl Into<String>) -> Self {
        self.remedy = Some(text.into());
        self
    }

    fn to_json(&self) -> Value {
        let mut object = Map::new();
        object.insert("status".into(), json!(self.status.word()));
        object.insert("verdict".into(), json!(self.verdict));
        object.insert("summary".into(), json!(self.summary));
        object.extend(self.fields.clone());
        if let Some(remedy) = &self.remedy {
            object.insert("remedy".into(), json!(remedy));
        }
        Value::Object(object)
    }
}

/// What the probes found. The classifier decides from this and from nothing else.
pub struct AppProbe {
    /// The socket that `doctor` asked.
    pub socket: PathBuf,
    /// The places where the Host CLI looks for the bundle.
    pub searched: Vec<PathBuf>,
    pub installed: Option<InstalledApp>,
    /// The answer of `GET /v1/status`, or the error that the request met.
    pub status: Result<StatusResponse, AppError>,
    /// This Host CLI as the version check sees it.
    pub cli: Peer,
    /// The directories this Host CLI resolved.
    pub cli_directories: Directories,
}

/// The `app` check.
#[derive(Debug, Clone, PartialEq)]
pub enum AppCheck {
    /// A Linux or Windows host has no app.
    NotApplicable,
    Report {
        socket: PathBuf,
        /// The items in report order, each under its stable name.
        items: Vec<(&'static str, Item)>,
    },
}

impl AppCheck {
    /// Probe this host. `macos_host` is a parameter so that a test shows the other hosts.
    pub async fn gather(macos_host: bool) -> Self {
        if !macos_host {
            return Self::NotApplicable;
        }
        let socket = app_socket_for_host(&EnvProvider::process(), true)
            .expect("a macOS host always has an app socket");
        let (searched, installed) = app_route::installed_app_here();
        let status = app_route::status(socket.clone()).await;
        let hello = app_route::client_hello();
        Self::classify(AppProbe {
            socket,
            searched,
            installed,
            status,
            cli: Peer {
                build: Some(env!("CARGO_PKG_VERSION").to_string()),
                protocol_range: Some(SUPPORTED_RANGE),
            },
            cli_directories: hello.directories.expect("the Host CLI states its directories"),
        })
    }

    pub fn classify(probe: AppProbe) -> Self {
        let mut items = vec![
            ("installed", installed_item(&probe)),
            ("running", running_item(&probe)),
        ];
        match &probe.status {
            Ok(status) => {
                items.push(("login_item", login_item(status)));
                items.push(("local_network", local_network_item(status)));
                items.push(("configuration", configuration_item(status)));
                items.push(("adoption", adoption_item(status)));
                items.push(("directories", directories_item(status, &probe.cli_directories)));
                items.push(("versions", versions_item(status, &probe.cli)));
                items.push(("bundle", bundle_item(status)));
            }
            Err(_) => {
                for name in [
                    "login_item",
                    "local_network",
                    "configuration",
                    "adoption",
                    "directories",
                    "versions",
                    "bundle",
                ] {
                    items.push((
                        name,
                        Item::new(
                            ItemStatus::Skip,
                            "app_not_running",
                            "not checked: the app does not answer",
                        ),
                    ));
                }
            }
        }
        Self::Report {
            socket: probe.socket,
            items,
        }
    }

    /// Only a failing item flips `ok`. A host without an app has nothing to fail.
    pub fn is_ok(&self) -> bool {
        match self {
            Self::NotApplicable => true,
            Self::Report { items, .. } => items.iter().all(|(_, i)| i.status != ItemStatus::Fail),
        }
    }

    pub fn to_json(&self) -> Value {
        match self {
            Self::NotApplicable => json!({
                "status": "skip", "verdict": "not_applicable",
                "reason": "The TestAnyware app belongs to macOS hosts. This host keeps the direct path.",
            }),
            Self::Report { socket, items } => {
                let mut object = Map::new();
                object.insert(
                    "status".into(),
                    json!(if self.is_ok() { "pass" } else { "fail" }),
                );
                object.insert("socket".into(), json!(socket));
                for (name, item) in items {
                    object.insert((*name).to_string(), item.to_json());
                }
                Value::Object(object)
            }
        }
    }

    pub fn lines(&self) -> Vec<String> {
        match self {
            Self::NotApplicable => vec![
                "  - not applicable: the TestAnyware app belongs to macOS hosts".to_string(),
            ],
            Self::Report { socket, items } => {
                let mut lines = vec![format!("  socket:          {}", socket.display())];
                let mut last_remedy: Option<&str> = None;
                for (name, item) in items {
                    lines.push(format!(
                        "  {} {}: {}",
                        item.status.mark(),
                        name.replace('_', " "),
                        item.summary
                    ));
                    // One remedy can answer two items, and a person reads it once.
                    if let Some(remedy) = item.remedy.as_deref().filter(|r| Some(*r) != last_remedy) {
                        lines.push(format!("    remedy: {remedy}"));
                        last_remedy = Some(remedy);
                    }
                }
                lines
            }
        }
    }
}

fn installed_item(probe: &AppProbe) -> Item {
    match &probe.installed {
        Some(app) => {
            let version = app.version.as_deref();
            Item::new(
                ItemStatus::Pass,
                "installed",
                match version {
                    Some(v) => format!("{} (version {v})", app.path.display()),
                    None => format!("{} (version unreadable)", app.path.display()),
                },
            )
            .field("path", &app.path)
            .field("version", version)
        }
        None => {
            let places: Vec<String> = probe.searched.iter().map(|p| p.display().to_string()).collect();
            let summary = format!("no bundle in {}", places.join(" or "));
            let item = Item::new(ItemStatus::Fail, "not_found", summary)
                .field("path", Value::Null)
                .field("version", Value::Null)
                .field("searched", &probe.searched);
            if probe.status.is_ok() {
                // An app that answers works, wherever it lives. A development bundle does.
                Item { status: ItemStatus::Warn, ..item }
            } else {
                item.remedy(SETUP_REMEDY)
            }
        }
    }
}

fn running_item(probe: &AppProbe) -> Item {
    match &probe.status {
        Ok(status) => {
            let range = status.protocol_range;
            let mut item = Item::new(
                ItemStatus::Pass,
                "running",
                format!(
                    "build {}, protocol {}-{}",
                    status.build, range.min, range.max
                ),
            )
            .field("build", &status.build)
            .field("protocol_range", range)
            .field("executable", &status.executable);
            if let Some(path) = &status.executable {
                item.summary.push_str(&format!(" ({})", path.display()));
            }
            item
        }
        Err(error) => {
            let cause = error.details["cause"].as_str().unwrap_or("unknown");
            let verdict = if cause == "not_running" { "not_running" } else { "unreachable" };
            let item = Item::new(ItemStatus::Fail, verdict, error.message.clone())
                .field("error", error);
            // The missing install already carries the setup, and the same sentence twice helps no one.
            match (&error.remediation, probe.installed.is_some()) {
                (Some(remedy), true) => item.remedy(remedy.clone()),
                _ => item,
            }
        }
    }
}

fn login_item(status: &StatusResponse) -> Item {
    match status.login_item {
        LoginItemState::Enabled => Item::new(ItemStatus::Pass, "enabled", "starts at login"),
        LoginItemState::RequiresApproval => Item::new(
            ItemStatus::Fail,
            "requires_approval",
            "macOS waits for a person to allow the login item",
        )
        .remedy(LOGIN_APPROVAL_REMEDY),
        LoginItemState::NotRegistered | LoginItemState::NotFound => Item::new(
            ItemStatus::Fail,
            "not_registered",
            "the app does not start at login",
        )
        .remedy(START_AT_LOGIN_REMEDY),
        LoginItemState::Unobserved => Item::new(
            ItemStatus::Skip,
            "unobserved",
            "this run of the app has no login item",
        ),
    }
}

fn local_network_item(status: &StatusResponse) -> Item {
    let seen = &status.local_network;
    let item = |status, verdict, summary: String| {
        Item::new(status, verdict, summary)
            .field("destination", &seen.destination)
            .field("observed_at", seen.observed_at)
    };
    let at = seen.destination.as_deref().unwrap_or("an unknown address");
    match seen.state {
        GrantState::Allowed => item(ItemStatus::Pass, "allowed", format!("a dial to {at} connected")),
        GrantState::Denied => item(
            ItemStatus::Fail,
            "denied",
            format!("a dial to {at} failed, and the platform `nc` proved the denial"),
        )
        .remedy(LOCAL_NETWORK_REMEDY),
        GrantState::NotYetSeen => item(
            ItemStatus::Warn,
            "not_yet_seen",
            "no guest connection has shown the grant yet".to_string(),
        )
        .remedy(ALLOW_NETWORK_AT_FIRST_GUEST),
    }
}

fn configuration_item(status: &StatusResponse) -> Item {
    let config = &status.configuration;
    let file = config.file.as_ref();
    let item = |status, verdict, summary: String| {
        Item::new(status, verdict, summary)
            .field("file", file)
            .field("evaluated_at", config.evaluated_at)
            .field("changed_since_evaluation", config.changed_since_evaluation)
    };
    let name = file.map_or_else(|| "config.rhai".to_string(), |f| f.display().to_string());
    match config.state {
        ConfigurationState::Failed => {
            let message = config.error.as_ref().map_or(name.clone(), |e| e.message.clone());
            item(ItemStatus::Fail, "invalid", message)
                .field("error", &config.error)
                .remedy(format!(
                    "Fix {name}, then quit TestAnyware from its menu and open it again. \
                     The app runs with its built-in settings until then."
                ))
        }
        _ if config.changed_since_evaluation => item(
            ItemStatus::Warn,
            "changed",
            format!("{name} changed after the app evaluated it"),
        )
        .remedy("Quit TestAnyware from its menu, and open it again, so that it reads the file as it is now."),
        ConfigurationState::Loaded => item(ItemStatus::Pass, "loaded", format!("{name} evaluated")),
        ConfigurationState::Absent => {
            item(ItemStatus::Pass, "absent", format!("no file at {name} (the file is optional)"))
        }
        ConfigurationState::Disabled => item(ItemStatus::Pass, "disabled", "the run reads no file".to_string()),
    }
}

fn adoption_item(status: &StatusResponse) -> Item {
    let adoption = &status.adoption;
    let item = |status, verdict, summary: String| {
        Item::new(status, verdict, summary)
            .field("finished", adoption.finished)
            .field("adopted", adoption.adopted.len())
            .field("not_adopted", &adoption.not_adopted)
    };
    if !adoption.finished {
        return item(
            ItemStatus::Warn,
            "pending",
            "the app still probes the VMs it found".to_string(),
        )
        .remedy("Run `testanyware doctor` again in a few seconds.");
    }
    if adoption.not_adopted.is_empty() {
        return item(
            ItemStatus::Pass,
            "adopted",
            format!("{} VM(s) adopted, none left out", adoption.adopted.len()),
        );
    }
    let left_out: Vec<String> = adoption
        .not_adopted
        .iter()
        .map(|vm| format!("{} ({})", vm.id, vm.reason))
        .collect();
    item(
        ItemStatus::Warn,
        "not_adopted",
        format!("not adopted: {}", left_out.join(", ")),
    )
    .remedy(
        "The app signalled none of these VMs, and no command reaches them through the app. \
         Stop each one by hand if you no longer need it. Quit TestAnyware and open it again to try the adoption again.",
    )
}

fn directories_item(status: &StatusResponse, cli: &Directories) -> Item {
    let item = |state, verdict, summary: String| {
        Item::new(state, verdict, summary)
            .field("app", &status.directories)
            .field("cli", cli)
    };
    if &status.directories == cli {
        return item(
            ItemStatus::Pass,
            "match",
            "the Host CLI and the app resolve the same directories".to_string(),
        );
    }
    // The commands fail with this same error object, so the message and the remedy agree.
    let error =
        AppError::incompatible_directories(&status.directories, Some(cli), &status.configuration);
    item(ItemStatus::Fail, "mismatch", error.message)
        .remedy(error.remediation.expect("a directory mismatch carries a remedy"))
}

fn versions_item(status: &StatusResponse, cli: &Peer) -> Item {
    let app = Peer {
        build: Some(status.build.clone()),
        protocol_range: Some(status.protocol_range),
    };
    let builds = |item: Item| {
        item.field("cli_build", &cli.build)
            .field("app_build", &app.build)
            .field("cli_protocol_range", cli.protocol_range)
            .field("app_protocol_range", app.protocol_range)
    };
    match decide(cli, &app, status.bundle_build.as_deref()) {
        Decision::Serve => builds(Item::new(
            ItemStatus::Pass,
            "match",
            format!("the Host CLI and the app are both build {}", status.build),
        )),
        Decision::ServeWithNotice(notice) => builds(
            Item::new(
                ItemStatus::Warn,
                "builds_differ",
                format!(
                    "the Host CLI is build {} and the app is build {}",
                    cli.build.as_deref().unwrap_or("unknown"),
                    status.build
                ),
            )
            .field("remedy_word", notice.remedy.word()),
        )
        .remedy(notice.remedy.text()),
        Decision::Fail(remedy) => builds(
            Item::new(
                ItemStatus::Fail,
                "no_common_version",
                "the Host CLI and the app share no protocol version".to_string(),
            )
            .field("remedy_word", remedy.word()),
        )
        .remedy(remedy.text()),
    }
}

fn bundle_item(status: &StatusResponse) -> Item {
    let item = |state, verdict, summary: String| {
        Item::new(state, verdict, summary)
            .field("app_build", &status.build)
            .field("bundle_build", &status.bundle_build)
    };
    match (&status.bundle_build, status.older_than_bundle) {
        (None, _) => item(
            ItemStatus::Skip,
            "no_bundle",
            "the app runs from no bundle, or its Info.plist does not read".to_string(),
        ),
        (Some(bundle), true) => item(
            ItemStatus::Warn,
            "older_than_bundle",
            format!("the running app is build {}, and its bundle on disk is build {bundle}", status.build),
        )
        .remedy(Remedy::QuitAndOpenApp.text()),
        (Some(bundle), false) => item(
            ItemStatus::Pass,
            "current",
            format!("the running app is not older than its bundle (build {bundle})"),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use testanyware_app_protocol::{
        AdoptionStatus, ConfigurationStatus, LocalNetworkStatus, NotAdoptedVm, ProtocolRange,
    };

    fn dirs(root: &str) -> Directories {
        Directories {
            state_dir: format!("/{root}/state").into(),
            data_dir: format!("/{root}/data").into(),
        }
    }

    /// A healthy app: the CLI's build and range, the same directories, a login item that is on and a
    /// grant that is allowed.
    fn healthy() -> StatusResponse {
        StatusResponse {
            build: "1.0.0".into(),
            protocol_range: ProtocolRange::new(1, 1),
            directories: dirs("a"),
            bundle_build: Some("1.0.0".into()),
            older_than_bundle: false,
            adoption: AdoptionStatus {
                finished: true,
                ..AdoptionStatus::default()
            },
            configuration: ConfigurationStatus {
                state: ConfigurationState::Absent,
                file: Some("/h/.config/testanyware/config.rhai".into()),
                ..ConfigurationStatus::default()
            },
            executable: Some("/Applications/TestAnyware.app/Contents/MacOS/testanyware-app".into()),
            local_network: LocalNetworkStatus {
                state: GrantState::Allowed,
                destination: Some("192.168.64.5:8648".into()),
                observed_at: Some(1_000),
            },
            login_item: LoginItemState::Enabled,
        }
    }

    fn installed() -> Option<InstalledApp> {
        Some(InstalledApp {
            path: "/Applications/TestAnyware.app".into(),
            version: Some("1.0.0".into()),
        })
    }

    fn probe(status: Result<StatusResponse, AppError>) -> AppProbe {
        AppProbe {
            socket: "/h/s.sock".into(),
            searched: vec!["/Applications/TestAnyware.app".into()],
            installed: installed(),
            status,
            cli: Peer {
                build: Some("1.0.0".into()),
                protocol_range: Some(ProtocolRange::new(1, 1)),
            },
            cli_directories: dirs("a"),
        }
    }

    fn item<'a>(check: &'a AppCheck, name: &str) -> &'a Item {
        match check {
            AppCheck::Report { items, .. } => &items.iter().find(|(n, _)| *n == name).unwrap().1,
            AppCheck::NotApplicable => panic!("no items"),
        }
    }

    fn classify(status: StatusResponse) -> AppCheck {
        AppCheck::classify(probe(Ok(status)))
    }

    #[test]
    fn a_healthy_app_passes_every_item_and_the_report_is_ok() {
        let check = classify(healthy());
        assert!(check.is_ok());
        let body = check.to_json();
        assert_eq!(body["status"], "pass");
        for name in [
            "installed", "running", "login_item", "local_network", "configuration", "adoption",
            "directories", "versions", "bundle",
        ] {
            assert_eq!(body[name]["status"], "pass", "{name}: {body}");
            assert!(body[name].get("remedy").is_none(), "{name}");
        }
        assert_eq!(body["installed"]["path"], "/Applications/TestAnyware.app");
        assert_eq!(body["installed"]["version"], "1.0.0");
        assert_eq!(body["running"]["build"], "1.0.0");
        assert_eq!(body["running"]["protocol_range"]["max"], 1);
        assert_eq!(body["socket"], "/h/s.sock");
    }

    #[test]
    fn other_directories_are_a_failing_item_with_the_remedy_the_commands_give() {
        let mut status = healthy();
        status.directories = dirs("b");
        let check = classify(status.clone());
        assert!(!check.is_ok());
        let item = item(&check, "directories");
        assert_eq!((item.status, item.verdict), (ItemStatus::Fail, "mismatch"));
        let error = AppError::incompatible_directories(
            &status.directories,
            Some(&dirs("a")),
            &status.configuration,
        );
        assert_eq!(item.remedy, error.remediation);
        let body = check.to_json();
        assert_eq!(body["directories"]["app"]["state_dir"], "/b/state");
        assert_eq!(body["directories"]["cli"]["state_dir"], "/a/state");
    }

    #[test]
    fn disjoint_protocol_ranges_are_a_failing_item_with_the_remedy_of_the_table() {
        let mut status = healthy();
        status.protocol_range = ProtocolRange::new(2, 3);
        status.build = "2.0.0".into();
        let check = classify(status);
        assert!(!check.is_ok());
        let item = item(&check, "versions");
        assert_eq!((item.status, item.verdict), (ItemStatus::Fail, "no_common_version"));
        assert_eq!(item.remedy.as_deref(), Some(Remedy::UpgradeHostCli.text()));
        assert_eq!(check.to_json()["versions"]["remedy_word"], "upgrade_host_cli");
    }

    #[test]
    fn builds_that_differ_warn_with_a_remedy_and_do_not_fail() {
        let mut status = healthy();
        status.build = "0.9.0".into();
        status.bundle_build = Some("1.0.0".into());
        status.older_than_bundle = true;
        let check = classify(status);
        assert!(check.is_ok());
        let versions = item(&check, "versions");
        assert_eq!((versions.status, versions.verdict), (ItemStatus::Warn, "builds_differ"));
        assert_eq!(versions.remedy.as_deref(), Some(Remedy::QuitAndOpenApp.text()));
        let bundle = item(&check, "bundle");
        assert_eq!((bundle.status, bundle.verdict), (ItemStatus::Warn, "older_than_bundle"));
        assert_eq!(bundle.remedy.as_deref(), Some(Remedy::QuitAndOpenApp.text()));
    }

    #[test]
    fn an_app_with_no_bundle_skips_the_bundle_item() {
        let mut status = healthy();
        status.bundle_build = None;
        assert_eq!(item(&classify(status), "bundle").status, ItemStatus::Skip);
    }

    #[test]
    fn a_silent_app_fails_running_and_skips_everything_that_needs_it() {
        let error = AppError::unreachable(
            testanyware_app_protocol::UnreachableCause::NotRunning,
            std::path::Path::new("/h/s.sock"),
            "Nothing answers.",
            "Open TestAnyware.",
        );
        let check = AppCheck::classify(probe(Err(error)));
        assert!(!check.is_ok());
        let running = item(&check, "running");
        assert_eq!((running.status, running.verdict), (ItemStatus::Fail, "not_running"));
        assert_eq!(running.remedy.as_deref(), Some("Open TestAnyware."));
        assert_eq!(check.to_json()["running"]["error"]["code"], "APP_UNREACHABLE");
        for name in ["login_item", "directories", "versions", "bundle"] {
            assert_eq!(item(&check, name).status, ItemStatus::Skip, "{name}");
        }
        assert_eq!(item(&check, "installed").status, ItemStatus::Pass);
    }

    #[test]
    fn a_missing_bundle_and_a_silent_app_carry_the_setup_once() {
        let error = AppError::unreachable(
            testanyware_app_protocol::UnreachableCause::NotRunning,
            std::path::Path::new("/h/s.sock"),
            "Nothing answers.",
            "Open TestAnyware.",
        );
        let mut missing = probe(Err(error));
        missing.installed = None;
        let check = AppCheck::classify(missing);
        let installed = item(&check, "installed");
        assert_eq!((installed.status, installed.verdict), (ItemStatus::Fail, "not_found"));
        assert_eq!(installed.remedy.as_deref(), Some(SETUP_REMEDY));
        assert_eq!(item(&check, "running").remedy, None);
        let lines = check.lines().join("\n");
        assert_eq!(lines.matches("Set up the TestAnyware app once").count(), 1, "{lines}");
    }

    #[test]
    fn a_running_app_outside_the_install_places_is_a_warning() {
        let mut outside = probe(Ok(healthy()));
        outside.installed = None;
        let check = AppCheck::classify(outside);
        assert!(check.is_ok());
        assert_eq!(item(&check, "installed").status, ItemStatus::Warn);
    }

    #[test]
    fn an_installed_bundle_with_an_unreadable_plist_has_a_null_version() {
        let mut unreadable = probe(Ok(healthy()));
        unreadable.installed = Some(InstalledApp {
            path: "/Applications/TestAnyware.app".into(),
            version: None,
        });
        let body = AppCheck::classify(unreadable).to_json();
        assert_eq!(body["installed"]["status"], "pass");
        assert_eq!(body["installed"]["version"], Value::Null);
    }

    #[test]
    fn each_state_of_the_login_item_has_its_verdict_and_remedy() {
        let cases = [
            (LoginItemState::Enabled, ItemStatus::Pass, None),
            (LoginItemState::NotRegistered, ItemStatus::Fail, Some(START_AT_LOGIN_REMEDY)),
            (LoginItemState::NotFound, ItemStatus::Fail, Some(START_AT_LOGIN_REMEDY)),
            (LoginItemState::RequiresApproval, ItemStatus::Fail, Some(LOGIN_APPROVAL_REMEDY)),
            (LoginItemState::Unobserved, ItemStatus::Skip, None),
        ];
        for (state, status, remedy) in cases {
            let mut app = healthy();
            app.login_item = state;
            let check = classify(app);
            let item = item(&check, "login_item");
            assert_eq!((item.status, item.remedy.as_deref()), (status, remedy), "{state:?}");
        }
    }

    #[test]
    fn each_state_of_the_grant_has_its_verdict_and_remedy() {
        for (state, status, remedy) in [
            (GrantState::Allowed, ItemStatus::Pass, None),
            (GrantState::Denied, ItemStatus::Fail, Some(LOCAL_NETWORK_REMEDY)),
            (GrantState::NotYetSeen, ItemStatus::Warn, Some(ALLOW_NETWORK_AT_FIRST_GUEST)),
        ] {
            let mut app = healthy();
            app.local_network.state = state;
            let check = classify(app);
            let item = item(&check, "local_network");
            assert_eq!((item.status, item.remedy.as_deref()), (status, remedy), "{state:?}");
        }
    }

    #[test]
    fn a_configuration_that_failed_in_the_app_is_a_failing_item_with_the_error() {
        let mut app = healthy();
        app.configuration.state = ConfigurationState::Failed;
        app.configuration.error = Some(AppError::config_invalid(
            std::path::Path::new("/h/config.rhai"),
            Some(2),
            "syntax error",
        ));
        let check = classify(app);
        assert!(!check.is_ok());
        let item = item(&check, "configuration");
        assert_eq!((item.status, item.verdict), (ItemStatus::Fail, "invalid"));
        assert!(item.remedy.as_deref().unwrap().contains("open it again"));
        assert_eq!(check.to_json()["configuration"]["error"]["code"], "CONFIG_INVALID");
    }

    #[test]
    fn a_file_that_changed_after_the_evaluation_is_a_warning_with_the_restart() {
        let mut app = healthy();
        app.configuration.state = ConfigurationState::Loaded;
        app.configuration.changed_since_evaluation = true;
        let check = classify(app);
        assert!(check.is_ok());
        let item = item(&check, "configuration");
        assert_eq!((item.status, item.verdict), (ItemStatus::Warn, "changed"));
        assert!(item.remedy.as_deref().unwrap().contains("open it again"));
    }

    #[test]
    fn a_vm_that_the_app_did_not_adopt_is_named_with_its_reason() {
        let mut app = healthy();
        app.adoption.not_adopted = vec![NotAdoptedVm {
            id: "vm-1".into(),
            reason: "process_gone".into(),
            detail: "the process is gone".into(),
        }];
        let check = classify(app);
        assert!(check.is_ok(), "advisory: the VM keeps running");
        let item = item(&check, "adoption");
        assert_eq!((item.status, item.verdict), (ItemStatus::Warn, "not_adopted"));
        assert!(item.summary.contains("vm-1") && item.summary.contains("process_gone"));
        assert!(item.remedy.is_some());
        assert_eq!(check.to_json()["adoption"]["not_adopted"][0]["id"], "vm-1");
    }

    #[test]
    fn an_adoption_that_has_not_finished_asks_for_a_second_run() {
        let mut app = healthy();
        app.adoption.finished = false;
        let check = classify(app);
        assert_eq!(item(&check, "adoption").verdict, "pending");
    }

    #[test]
    fn a_linux_or_windows_host_reports_that_the_check_does_not_apply() {
        let check = block_on(AppCheck::gather(false));
        assert_eq!(check, AppCheck::NotApplicable);
        assert!(check.is_ok());
        let body = check.to_json();
        assert_eq!(body["status"], "skip");
        assert_eq!(body["verdict"], "not_applicable");
        assert!(body["reason"].as_str().unwrap().contains("macOS"));
        assert!(check.lines()[0].contains("not applicable"));
    }

    fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(future)
    }

    #[test]
    fn the_text_has_one_line_per_item_and_a_remedy_under_each_non_pass() {
        let mut app = healthy();
        app.login_item = LoginItemState::NotRegistered;
        let lines = classify(app).lines();
        assert!(lines.iter().any(|l| l.starts_with("  ✗ login item:")), "{lines:?}");
        assert!(lines.iter().any(|l| l == &format!("    remedy: {START_AT_LOGIN_REMEDY}")));
        assert!(lines.iter().any(|l| l.starts_with("  ✓ installed:")));
    }

    /// Every report this check can make declares only what `doctor.json` declares: each group
    /// key, each status and verdict of an item, and each field of it.
    #[test]
    fn every_report_fits_the_doctor_schema() {
        let schema: Value = serde_json::from_str(include_str!(
            "../../../../../docs/reference/cli-schemas/doctor.json"
        ))
        .unwrap();
        let checks = &schema["properties"]["checks"];
        assert!(checks["required"].as_array().unwrap().iter().any(|r| r == "app"));
        let app = &checks["properties"]["app"];

        let mut reports = vec![AppCheck::NotApplicable];
        let mut cases: Vec<StatusResponse> = vec![healthy()];
        for state in [LoginItemState::NotRegistered, LoginItemState::RequiresApproval, LoginItemState::Unobserved] {
            cases.push(StatusResponse { login_item: state, ..healthy() });
        }
        cases.push(StatusResponse { directories: dirs("b"), ..healthy() });
        cases.push(StatusResponse {
            protocol_range: ProtocolRange::new(2, 2),
            build: "2.0.0".into(),
            ..healthy()
        });
        cases.push(StatusResponse { bundle_build: None, ..healthy() });
        cases.push(StatusResponse { build: "0.9.0".into(), older_than_bundle: true, ..healthy() });
        let mut failed = healthy();
        failed.configuration.state = ConfigurationState::Failed;
        failed.configuration.error =
            Some(AppError::config_invalid(std::path::Path::new("/h/c.rhai"), None, "bad"));
        cases.push(failed);
        let mut adoption = healthy();
        adoption.adoption.not_adopted = vec![NotAdoptedVm {
            id: "v".into(),
            reason: "process_gone".into(),
            detail: "gone".into(),
        }];
        cases.push(adoption);
        let mut unseen = healthy();
        unseen.local_network = LocalNetworkStatus::default();
        cases.push(unseen);
        reports.extend(cases.into_iter().map(classify));
        let silent = AppError::unreachable(
            testanyware_app_protocol::UnreachableCause::NotRunning,
            std::path::Path::new("/h/s.sock"),
            "Nothing answers.",
            "Open TestAnyware.",
        );
        let mut missing = probe(Err(silent));
        missing.installed = None;
        reports.push(AppCheck::classify(missing));

        for report in reports {
            let body = report.to_json();
            assert!(app["properties"]["status"]["enum"].as_array().unwrap().contains(&body["status"]));
            for (key, value) in body.as_object().unwrap() {
                let declared = app["properties"].get(key).unwrap_or_else(|| panic!("{key} not in schema"));
                let Some(item) = value.as_object().filter(|_| declared["properties"].is_object()) else {
                    continue;
                };
                let allowed = |field: &str, v: &Value| {
                    declared["properties"][field]["enum"].as_array().unwrap().contains(v)
                };
                assert!(allowed("status", &item["status"]), "{key}: status {}", item["status"]);
                assert!(allowed("verdict", &item["verdict"]), "{key}: verdict {}", item["verdict"]);
                for field in item.keys() {
                    assert!(declared["properties"].get(field).is_some(), "{key}.{field} not in schema");
                }
                for required in declared["required"].as_array().unwrap() {
                    assert!(item.contains_key(required.as_str().unwrap()), "{key}: {required} missing");
                }
            }
        }
    }
}
