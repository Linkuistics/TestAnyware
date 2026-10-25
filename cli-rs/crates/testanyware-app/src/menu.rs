//! What the status item's menu shows, as a pure function of the app's state (spec: "The TestAnyware
//! app"). The AppKit shell turns these entries into menu items and holds no decision of its own.

use testanyware_app_protocol::{
    ConfigurationState, ConfigurationStatus, NotAdoptedVm, RequestedBy, SlotsView,
};

use crate::grant::GrantObservation;

/// What the app last learned about the Local Network grant, without the dial that taught it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalNetworkState {
    Allowed,
    NotYetSeen,
    Denied,
}

impl From<&GrantObservation> for LocalNetworkState {
    fn from(observation: &GrantObservation) -> Self {
        match observation {
            GrantObservation::NotYetSeen => Self::NotYetSeen,
            GrantObservation::Allowed { .. } => Self::Allowed,
            GrantObservation::Denied { .. } => Self::Denied,
        }
    }
}

/// The status of `SMAppService.mainApp`, as macOS reports it.
/// https://developer.apple.com/documentation/servicemanagement/smappservice/status-swift.enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginItemStatus {
    NotRegistered,
    Enabled,
    RequiresApproval,
    /// Koine observed this status from a signed bundle before its first `register()`, so it
    /// counts as not registered.
    NotFound,
}

impl From<LoginItemStatus> for testanyware_app_protocol::LoginItemState {
    fn from(status: LoginItemStatus) -> Self {
        match status {
            LoginItemStatus::NotRegistered => Self::NotRegistered,
            LoginItemStatus::Enabled => Self::Enabled,
            LoginItemStatus::RequiresApproval => Self::RequiresApproval,
            LoginItemStatus::NotFound => Self::NotFound,
        }
    }
}

impl LoginItemStatus {
    /// Both `Enabled` and `RequiresApproval` mean that the app asked to start at login.
    pub fn is_registered(self) -> bool {
        matches!(self, Self::Enabled | Self::RequiresApproval)
    }
}

/// A running VM, as one line of the menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuVm {
    pub id: String,
    pub platform: String,
    pub backend: String,
}

/// Everything the menu depends on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuState {
    pub version: String,
    /// Does the app serve its socket now?
    pub serving: bool,
    pub local_network: LocalNetworkState,
    /// The running VMs, or `None` when the listing did not answer in time.
    pub running_vms: Option<Vec<MenuVm>>,
    /// The VMs that the app found at its start and did not adopt.
    pub not_adopted: Vec<NotAdoptedVm>,
    /// The macOS guest slots, each holder and each waiter, as `vm slots` shows them.
    pub slots: SlotsView,
    pub login: LoginItemStatus,
    /// The error text of the last failed `register()` or `unregister()`.
    pub login_problem: Option<String>,
    /// What the app made of `config.rhai` when it started.
    pub configuration: ConfigurationStatus,
}

/// What a click on a menu item does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    OpenLocalNetworkSettings,
    /// Register the app as a login item when it is not one, and unregister it when it is.
    ToggleStartAtLogin,
    OpenLoginItemsSettings,
    /// Open `config.rhai` in the person's text editor.
    OpenConfigFile,
    Quit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuEntry {
    /// A line of text that does nothing.
    Text(String),
    Separator,
    Action {
        title: String,
        action: MenuAction,
    },
    /// An item with a check mark.
    Switch {
        title: String,
        on: bool,
        action: MenuAction,
    },
}

/// The URL that opens System Settings for the Local Network row. No Apple page documents an
/// anchor for it. Measured on macOS 27.0.1 (2026-10-20): this URL opens Privacy & Security, the
/// pane that holds the Local Network row, and no anchor of the extension reaches the row itself.
/// `?Privacy_Camera` does reach the Camera pane, so the extension reads anchors this way and
/// has none for Local Network. Revisit when a macOS release adds one.
pub const LOCAL_NETWORK_SETTINGS_URL: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_LocalNetwork";

/// The title of the Quit item. The text tells the person what a quit leaves behind.
pub const QUIT_TITLE: &str = "Quit TestAnyware (each VM keeps running)";

/// The lines for the running VMs: a heading and one line for each VM, or one line that says why
/// there is no list.
fn running_vm_entries(running: Option<&[MenuVm]>) -> Vec<MenuEntry> {
    match running {
        None => vec![MenuEntry::Text(
            "Running VMs: no answer from the listing".to_string(),
        )],
        Some([]) => vec![MenuEntry::Text("No VMs running".to_string())],
        Some(vms) => std::iter::once(MenuEntry::Text("Running VMs:".to_string()))
            .chain(vms.iter().map(|vm| {
                MenuEntry::Text(format!("  {} ({}, {})", vm.id, vm.platform, vm.backend))
            }))
            .collect(),
    }
}

/// The lines for the VMs the app did not adopt. The heading says what the app did about them.
fn not_adopted_entries(not_adopted: &[NotAdoptedVm]) -> Vec<MenuEntry> {
    if not_adopted.is_empty() {
        return Vec::new();
    }
    std::iter::once(MenuEntry::Separator)
        .chain(std::iter::once(MenuEntry::Text(
            "Not adopted (the app signals nothing):".to_string(),
        )))
        .chain(
            not_adopted
                .iter()
                .map(|vm| MenuEntry::Text(format!("  {}: {}", vm.id, vm.reason.replace('_', " ")))),
        )
        .collect()
}

/// Who asked, as the text after a comma. A holder that no request made says nothing.
fn who_asked(by: &Option<RequestedBy>) -> String {
    match by {
        None => String::new(),
        Some(by) => format!(
            ", pid {} in {}",
            by.pid.map_or_else(|| "?".to_string(), |pid| pid.to_string()),
            by.cwd.as_deref().unwrap_or("?")
        ),
    }
}

/// The lines for the macOS guest slots: the count, each holder, and each waiter with its place
/// and who asked (spec "admission-queue": "The command surface").
fn slots_entries(slots: &SlotsView) -> Vec<MenuEntry> {
    let mut entries = vec![MenuEntry::Text(format!(
        "macOS guest slots: {} of {} in use (macos_slots {})",
        slots.holders.len(),
        slots.limit,
        slots.slots
    ))];
    for holder in &slots.holders {
        entries.push(MenuEntry::Text(format!(
            "  {} {}: {}{}",
            holder.kind.as_str(),
            holder.vms.join(", "),
            holder.state.as_str(),
            who_asked(&holder.requested_by)
        )));
    }
    if slots.queue.is_empty() {
        entries.push(MenuEntry::Text("  No start waits".to_string()));
    } else {
        entries.push(MenuEntry::Text("Waiting:".to_string()));
        for waiter in &slots.queue {
            entries.push(MenuEntry::Text(format!(
                "  {}. {} ({}){}",
                waiter.place,
                waiter.vms.join(", "),
                waiter.request_id,
                who_asked(&waiter.requested_by)
            )));
        }
    }
    if let Some(contention) = &slots.contention {
        entries.push(MenuEntry::Text(format!(
            "  The framework refused a start: {}",
            contention.tart_error
        )));
    }
    entries
}

/// A time in seconds since the Unix epoch, as `2025-10-09 08:53:20 UTC`. The menu is a pure
/// function, so it does not ask the host for a time zone.
pub fn format_utc(seconds: u64) -> String {
    let (days, rest) = (seconds / 86_400, seconds % 86_400);
    // Days since 1970-01-01 to a civil date, by Howard Hinnant's algorithm.
    // https://howardhinnant.github.io/date_algorithms.html#civil_from_days
    let shifted = days as i64 + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02} UTC",
        rest / 3_600,
        rest % 3_600 / 60,
        rest % 60
    )
}

/// The lines for `config.rhai`: the file, when the app evaluated it and any error. A file that
/// is there gets an item that opens it.
fn configuration_entries(configuration: &ConfigurationStatus) -> Vec<MenuEntry> {
    let file = configuration
        .file
        .as_ref()
        .map_or_else(String::new, |f| f.display().to_string());
    let mut entries = vec![MenuEntry::Text(match configuration.state {
        ConfigurationState::Disabled => "Configuration: no file is read".to_string(),
        ConfigurationState::Absent => format!("Configuration: no file at {file}"),
        ConfigurationState::Loaded => format!("Configuration: {file}"),
        ConfigurationState::Failed => format!("Configuration: {file} (failed)"),
    })];
    if let Some(seconds) = configuration.evaluated_at {
        entries.push(MenuEntry::Text(format!(
            "Last evaluated: {}",
            format_utc(seconds)
        )));
    }
    if let Some(error) = &configuration.error {
        let message = error.details["message"].as_str().unwrap_or(&error.message);
        entries.push(MenuEntry::Text(match error.details["line"].as_u64() {
            Some(line) => format!("Error at line {line}: {message}"),
            None => format!("Error: {message}"),
        }));
        entries.push(MenuEntry::Text(
            "The app runs with its built-in defaults".to_string(),
        ));
    }
    if configuration.changed_since_evaluation {
        entries.push(MenuEntry::Text(
            "The file changed since then. Quit and open TestAnyware to read it".to_string(),
        ));
    }
    if matches!(
        configuration.state,
        ConfigurationState::Loaded | ConfigurationState::Failed
    ) {
        entries.push(MenuEntry::Action {
            title: "Open config.rhai".to_string(),
            action: MenuAction::OpenConfigFile,
        });
    }
    entries
}

pub fn build_menu(state: &MenuState) -> Vec<MenuEntry> {
    let mut menu = vec![
        MenuEntry::Text(format!("TestAnyware {}", state.version)),
        MenuEntry::Text(
            if state.serving {
                "Serving its socket"
            } else {
                "Not serving its socket"
            }
            .to_string(),
        ),
        MenuEntry::Separator,
        MenuEntry::Text(
            match state.local_network {
                LocalNetworkState::Allowed => "Local Network: allowed",
                LocalNetworkState::NotYetSeen => "Local Network: not yet seen",
                LocalNetworkState::Denied => "Local Network: denied",
            }
            .to_string(),
        ),
        MenuEntry::Action {
            title: "Open Local Network Settings…".to_string(),
            action: MenuAction::OpenLocalNetworkSettings,
        },
        MenuEntry::Separator,
    ];
    menu.extend(running_vm_entries(state.running_vms.as_deref()));
    menu.extend(not_adopted_entries(&state.not_adopted));
    menu.push(MenuEntry::Separator);
    menu.extend(slots_entries(&state.slots));
    menu.push(MenuEntry::Separator);
    menu.push(MenuEntry::Switch {
        title: "Start at Login".to_string(),
        on: state.login.is_registered(),
        action: MenuAction::ToggleStartAtLogin,
    });
    if state.login == LoginItemStatus::RequiresApproval {
        menu.push(MenuEntry::Text(
            "Start at Login requires approval in System Settings".to_string(),
        ));
        menu.push(MenuEntry::Action {
            title: "Open Login Items Settings…".to_string(),
            action: MenuAction::OpenLoginItemsSettings,
        });
    }
    if let Some(problem) = &state.login_problem {
        menu.push(MenuEntry::Text(problem.clone()));
    }
    menu.push(MenuEntry::Separator);
    menu.extend(configuration_entries(&state.configuration));
    menu.push(MenuEntry::Separator);
    menu.push(MenuEntry::Action {
        title: QUIT_TITLE.to_string(),
        action: MenuAction::Quit,
    });
    menu
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;
    use testanyware_app_protocol::{
        HolderKind, HolderState, SlotContention, SlotHolder, SlotWaiter, WaiterKind,
    };
    use std::time::SystemTime;

    fn state() -> MenuState {
        MenuState {
            version: "1.2.3".to_string(),
            serving: true,
            local_network: LocalNetworkState::NotYetSeen,
            running_vms: Some(Vec::new()),
            not_adopted: Vec::new(),
            slots: SlotsView {
                slots: 2,
                limit: 2,
                ..SlotsView::empty()
            },
            login: LoginItemStatus::NotRegistered,
            login_problem: None,
            configuration: ConfigurationStatus::default(),
        }
    }

    fn holder(kind: HolderKind, vm: &str, by: Option<RequestedBy>) -> SlotHolder {
        SlotHolder {
            kind,
            vms: vec![vm.to_string()],
            state: HolderState::Running,
            request_id: by.as_ref().map(|_| "r-1".to_string()),
            requested_by: by,
            since: Some(1),
        }
    }

    fn waiter(place: u32, vm: &str, by: Option<RequestedBy>) -> SlotWaiter {
        SlotWaiter {
            request_id: format!("r-{place}"),
            place,
            kind: WaiterKind::Vm,
            vms: vec![vm.to_string()],
            requested_by: by,
            since: 1,
            deadline: Some(61),
        }
    }

    fn by(pid: u32, cwd: &str) -> Option<RequestedBy> {
        Some(RequestedBy {
            pid: Some(pid),
            cwd: Some(cwd.to_string()),
        })
    }

    #[test]
    fn menu_shows_the_slots_each_holder_and_each_waiter_with_its_place_and_who_asked() {
        let menu = build_menu(&MenuState {
            slots: SlotsView {
                holders: vec![
                    holder(HolderKind::Vm, "vm-a", by(11, "/trees/a")),
                    holder(HolderKind::External, "outside", None),
                ],
                queue: vec![
                    waiter(1, "vm-b", by(22, "/trees/b")),
                    waiter(2, "vm-c", None),
                ],
                ..state().slots
            },
            ..state()
        });
        let lines = texts(&menu);
        let heading = lines
            .iter()
            .position(|t| *t == "macOS guest slots: 2 of 2 in use (macos_slots 2)")
            .expect("a heading");
        assert_eq!(
            lines[heading + 1..heading + 6],
            [
                "  vm vm-a: running, pid 11 in /trees/a",
                "  external outside: running",
                "Waiting:",
                "  1. vm-b (r-1), pid 22 in /trees/b",
                "  2. vm-c (r-2)",
            ]
        );
    }

    #[test]
    fn menu_says_so_when_no_start_waits_and_when_the_framework_refused() {
        let menu = build_menu(&state());
        assert!(texts(&menu).contains(&"  No start waits"));
        let menu = build_menu(&MenuState {
            slots: SlotsView {
                contention: Some(SlotContention {
                    since: 1,
                    tart_error: "limit".to_string(),
                    holders: Vec::new(),
                }),
                ..state().slots
            },
            ..state()
        });
        assert!(texts(&menu).contains(&"  The framework refused a start: limit"));
    }

    fn loaded() -> ConfigurationStatus {
        ConfigurationStatus {
            state: ConfigurationState::Loaded,
            file: Some("/h/.config/testanyware/config.rhai".into()),
            evaluated_at: Some(1_760_000_000),
            ..ConfigurationStatus::default()
        }
    }

    fn failed(line: Option<usize>) -> ConfigurationStatus {
        let file = std::path::PathBuf::from("/h/.config/testanyware/config.rhai");
        ConfigurationStatus {
            state: ConfigurationState::Failed,
            error: Some(testanyware_app_protocol::AppError::config_invalid(
                &file,
                line,
                "syntax error: Unexpected ';'",
            )),
            file: Some(file),
            evaluated_at: Some(1_760_000_000),
            ..ConfigurationStatus::default()
        }
    }

    fn opens_the_file(menu: &[MenuEntry]) -> bool {
        menu.iter().any(|e| {
            matches!(
                e,
                MenuEntry::Action {
                    action: MenuAction::OpenConfigFile,
                    ..
                }
            )
        })
    }

    #[test]
    fn menu_shows_the_file_the_time_of_its_evaluation_and_an_item_that_opens_it() {
        let menu = build_menu(&MenuState {
            configuration: loaded(),
            ..state()
        });
        let lines = texts(&menu);
        assert!(lines.contains(&"Configuration: /h/.config/testanyware/config.rhai"));
        assert!(lines.contains(&"Last evaluated: 2025-10-09 08:53:20 UTC"));
        assert!(opens_the_file(&menu));
        assert!(!lines.iter().any(|t| t.starts_with("Error")));
    }

    #[test]
    fn menu_shows_the_error_of_a_file_that_fails_with_its_line() {
        let menu = build_menu(&MenuState {
            configuration: failed(Some(3)),
            ..state()
        });
        let lines = texts(&menu);
        assert!(lines.contains(&"Configuration: /h/.config/testanyware/config.rhai (failed)"));
        assert!(lines.contains(&"Error at line 3: syntax error: Unexpected ';'"));
        assert!(lines.contains(&"The app runs with its built-in defaults"));
        // A file that fails is still a file a person must be able to open.
        assert!(opens_the_file(&menu));
    }

    #[test]
    fn menu_shows_an_error_with_no_line_without_one() {
        let menu = build_menu(&MenuState {
            configuration: failed(None),
            ..state()
        });
        assert!(texts(&menu).contains(&"Error: syntax error: Unexpected ';'"));
    }

    #[test]
    fn menu_offers_no_item_to_open_a_file_that_is_not_there() {
        for configuration in [
            ConfigurationStatus::default(),
            ConfigurationStatus {
                state: ConfigurationState::Absent,
                file: Some("/h/config.rhai".into()),
                evaluated_at: Some(1),
                ..ConfigurationStatus::default()
            },
        ] {
            let menu = build_menu(&MenuState {
                configuration,
                ..state()
            });
            assert!(!opens_the_file(&menu));
            assert!(texts(&menu)
                .iter()
                .any(|t| t.starts_with("Configuration: no file")));
        }
    }

    #[test]
    fn menu_asks_for_a_restart_when_the_file_changed_since_the_evaluation() {
        let changed = ConfigurationStatus {
            changed_since_evaluation: true,
            ..loaded()
        };
        let menu = build_menu(&MenuState {
            configuration: changed,
            ..state()
        });
        assert!(texts(&menu)
            .iter()
            .any(|t| t.contains("Quit and open TestAnyware")));
        let quiet = build_menu(&MenuState {
            configuration: loaded(),
            ..state()
        });
        assert!(!texts(&quiet)
            .iter()
            .any(|t| t.contains("Quit and open TestAnyware")));
    }

    #[test]
    fn the_configuration_comes_after_the_login_switch_and_before_quit() {
        let menu = build_menu(&MenuState {
            configuration: loaded(),
            ..state()
        });
        let login = menu
            .iter()
            .position(|e| {
                matches!(
                    e,
                    MenuEntry::Switch {
                        action: MenuAction::ToggleStartAtLogin,
                        ..
                    }
                )
            })
            .unwrap();
        let file = menu
            .iter()
            .position(|e| matches!(e, MenuEntry::Text(t) if t.starts_with("Configuration:")))
            .unwrap();
        assert!(login < file && file < menu.len() - 1, "{menu:?}");
    }

    #[test]
    fn format_utc_reads_known_instants() {
        assert_eq!(format_utc(0), "1970-01-01 00:00:00 UTC");
        assert_eq!(format_utc(951_782_400), "2000-02-29 00:00:00 UTC");
        assert_eq!(format_utc(1_760_000_000), "2025-10-09 08:53:20 UTC");
        assert_eq!(
            format_utc(1_760_000_000 + 86_399),
            "2025-10-10 08:53:19 UTC"
        );
    }

    fn vm(id: &str, platform: &str, backend: &str) -> MenuVm {
        MenuVm {
            id: id.to_string(),
            platform: platform.to_string(),
            backend: backend.to_string(),
        }
    }

    fn texts(menu: &[MenuEntry]) -> Vec<&str> {
        menu.iter()
            .filter_map(|e| match e {
                MenuEntry::Text(t) => Some(t.as_str()),
                _ => None,
            })
            .collect()
    }

    fn login_switch(menu: &[MenuEntry]) -> Option<bool> {
        menu.iter().find_map(|e| match e {
            MenuEntry::Switch {
                on,
                action: MenuAction::ToggleStartAtLogin,
                ..
            } => Some(*on),
            _ => None,
        })
    }

    #[test]
    fn menu_shows_the_version_and_whether_the_socket_is_served() {
        let served = build_menu(&state());
        assert_eq!(texts(&served)[0], "TestAnyware 1.2.3");
        assert_eq!(texts(&served)[1], "Serving its socket");

        let stopped = build_menu(&MenuState {
            serving: false,
            ..state()
        });
        assert_eq!(texts(&stopped)[1], "Not serving its socket");
    }

    #[test]
    fn menu_names_each_local_network_state_and_always_offers_the_pane() {
        for (local_network, line) in [
            (LocalNetworkState::Allowed, "Local Network: allowed"),
            (LocalNetworkState::NotYetSeen, "Local Network: not yet seen"),
            (LocalNetworkState::Denied, "Local Network: denied"),
        ] {
            let menu = build_menu(&MenuState {
                local_network,
                ..state()
            });
            assert!(texts(&menu).contains(&line), "{line}");
            assert!(menu.iter().any(|e| matches!(
                e,
                MenuEntry::Action {
                    action: MenuAction::OpenLocalNetworkSettings,
                    ..
                }
            )));
        }
    }

    #[test]
    fn grant_observation_maps_to_the_three_states() {
        let destination: SocketAddr = "192.168.64.2:8648".parse().unwrap();
        let at = SystemTime::UNIX_EPOCH;
        assert_eq!(
            LocalNetworkState::from(&GrantObservation::NotYetSeen),
            LocalNetworkState::NotYetSeen
        );
        assert_eq!(
            LocalNetworkState::from(&GrantObservation::Allowed { destination, at }),
            LocalNetworkState::Allowed
        );
        assert_eq!(
            LocalNetworkState::from(&GrantObservation::Denied { destination, at }),
            LocalNetworkState::Denied
        );
    }

    #[test]
    fn start_at_login_switch_is_on_for_both_registered_statuses() {
        for (login, on) in [
            (LoginItemStatus::Enabled, true),
            (LoginItemStatus::RequiresApproval, true),
            (LoginItemStatus::NotRegistered, false),
            (LoginItemStatus::NotFound, false),
        ] {
            assert_eq!(
                login_switch(&build_menu(&MenuState { login, ..state() })),
                Some(on)
            );
        }
    }

    #[test]
    fn requires_approval_is_reported_with_a_way_to_the_login_items_pane() {
        let menu = build_menu(&MenuState {
            login: LoginItemStatus::RequiresApproval,
            ..state()
        });
        assert!(texts(&menu).iter().any(|t| t.contains("requires approval")));
        assert!(menu.iter().any(|e| matches!(
            e,
            MenuEntry::Action {
                action: MenuAction::OpenLoginItemsSettings,
                ..
            }
        )));

        let enabled = build_menu(&MenuState {
            login: LoginItemStatus::Enabled,
            ..state()
        });
        assert!(!enabled.iter().any(|e| matches!(
            e,
            MenuEntry::Action {
                action: MenuAction::OpenLoginItemsSettings,
                ..
            }
        )));
    }

    #[test]
    fn a_failed_registration_shows_its_error() {
        let menu = build_menu(&MenuState {
            login_problem: Some("Operation not permitted".to_string()),
            ..state()
        });
        assert!(texts(&menu).contains(&"Operation not permitted"));
        assert!(!texts(&build_menu(&state())).contains(&"Operation not permitted"));
    }

    #[test]
    fn menu_says_that_no_vm_runs() {
        assert!(texts(&build_menu(&state())).contains(&"No VMs running"));
    }

    #[test]
    fn menu_lists_each_running_vm_under_a_heading() {
        let menu = build_menu(&MenuState {
            running_vms: Some(vec![
                vm("testanyware-aa", "macos", "tart"),
                vm("testanyware-bb", "windows", "qemu"),
            ]),
            ..state()
        });
        let lines = texts(&menu);
        let heading = lines
            .iter()
            .position(|t| *t == "Running VMs:")
            .expect("a heading");
        assert_eq!(
            lines[heading + 1..heading + 3],
            [
                "  testanyware-aa (macos, tart)",
                "  testanyware-bb (windows, qemu)"
            ]
        );
        assert!(!lines.contains(&"No VMs running"));
    }

    #[test]
    fn menu_says_so_when_the_listing_does_not_answer() {
        let menu = build_menu(&MenuState {
            running_vms: None,
            ..state()
        });
        assert!(texts(&menu)
            .iter()
            .any(|t| t.starts_with("Running VMs: no answer")));
    }

    #[test]
    fn the_running_vms_come_after_the_local_network_and_before_the_login_switch() {
        let menu = build_menu(&MenuState {
            running_vms: Some(vec![vm("testanyware-aa", "macos", "tart")]),
            ..state()
        });
        let at = |wanted: &dyn Fn(&MenuEntry) -> bool| menu.iter().position(wanted).unwrap();
        let pane = at(&|e| {
            matches!(
                e,
                MenuEntry::Action {
                    action: MenuAction::OpenLocalNetworkSettings,
                    ..
                }
            )
        });
        let vms = at(&|e| matches!(e, MenuEntry::Text(t) if t == "Running VMs:"));
        let login = at(&|e| {
            matches!(
                e,
                MenuEntry::Switch {
                    action: MenuAction::ToggleStartAtLogin,
                    ..
                }
            )
        });
        assert!(pane < vms && vms < login, "{menu:?}");
    }

    #[test]
    fn menu_names_each_vm_that_was_not_adopted_with_its_reason() {
        let none = build_menu(&state());
        assert!(!texts(&none).iter().any(|t| t.starts_with("Not adopted")));

        let menu = build_menu(&MenuState {
            not_adopted: vec![NotAdoptedVm {
                id: "testanyware-aa".into(),
                reason: "start_time_differs".into(),
                detail: "d".into(),
            }],
            ..state()
        });
        let lines = texts(&menu);
        let heading = lines
            .iter()
            .position(|t| *t == "Not adopted (the app signals nothing):")
            .expect("the heading");
        assert_eq!(lines[heading + 1], "  testanyware-aa: start time differs");
    }

    #[test]
    fn quit_is_last_and_says_that_each_vm_keeps_running() {
        let menu = build_menu(&state());
        let Some(MenuEntry::Action { title, action }) = menu.last() else {
            panic!("the last entry is not an action: {menu:?}");
        };
        assert_eq!(*action, MenuAction::Quit);
        assert!(title.contains("each VM keeps running"));
    }
}
