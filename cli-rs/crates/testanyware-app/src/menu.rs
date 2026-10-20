//! What the status item's menu shows, as a pure function of the app's state (spec: "The TestAnyware
//! app"). The AppKit shell turns these entries into menu items and holds no decision of its own.

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
    pub login: LoginItemStatus,
    /// The error text of the last failed `register()` or `unregister()`.
    pub login_problem: Option<String>,
}

/// What a click on a menu item does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    OpenLocalNetworkSettings,
    /// Register the app as a login item when it is not one, and unregister it when it is.
    ToggleStartAtLogin,
    OpenLoginItemsSettings,
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
        None => vec![MenuEntry::Text("Running VMs: no answer from the listing".to_string())],
        Some([]) => vec![MenuEntry::Text("No VMs running".to_string())],
        Some(vms) => std::iter::once(MenuEntry::Text("Running VMs:".to_string()))
            .chain(vms.iter().map(|vm| {
                MenuEntry::Text(format!("  {} ({}, {})", vm.id, vm.platform, vm.backend))
            }))
            .collect(),
    }
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
    use std::time::SystemTime;

    fn state() -> MenuState {
        MenuState {
            version: "1.2.3".to_string(),
            serving: true,
            local_network: LocalNetworkState::NotYetSeen,
            running_vms: Some(Vec::new()),
            login: LoginItemStatus::NotRegistered,
            login_problem: None,
        }
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
        let heading = lines.iter().position(|t| *t == "Running VMs:").expect("a heading");
        assert_eq!(
            lines[heading + 1..heading + 3],
            ["  testanyware-aa (macos, tart)", "  testanyware-bb (windows, qemu)"]
        );
        assert!(!lines.contains(&"No VMs running"));
    }

    #[test]
    fn menu_says_so_when_the_listing_does_not_answer() {
        let menu = build_menu(&MenuState {
            running_vms: None,
            ..state()
        });
        assert!(texts(&menu).iter().any(|t| t.starts_with("Running VMs: no answer")));
    }

    #[test]
    fn the_running_vms_come_after_the_local_network_and_before_the_login_switch() {
        let menu = build_menu(&MenuState {
            running_vms: Some(vec![vm("testanyware-aa", "macos", "tart")]),
            ..state()
        });
        let at = |wanted: &dyn Fn(&MenuEntry) -> bool| menu.iter().position(wanted).unwrap();
        let pane = at(&|e| matches!(e, MenuEntry::Action { action: MenuAction::OpenLocalNetworkSettings, .. }));
        let vms = at(&|e| matches!(e, MenuEntry::Text(t) if t == "Running VMs:"));
        let login = at(&|e| matches!(e, MenuEntry::Switch { action: MenuAction::ToggleStartAtLogin, .. }));
        assert!(pane < vms && vms < login, "{menu:?}");
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
