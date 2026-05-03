use crate::agent_responses::{ActionResponse, ErrorResponse, InspectResponse, SnapshotResponse};
use crate::element_info::ElementInfo;
use crate::unified_role::UnifiedRole;
use crate::window_info::WindowInfo;

/// LLM-optimised plain-text rendering of agent responses.
///
/// Mirrors `AgentFormatter` in
/// `cli/Sources/TestAnywareAgentProtocol/AgentFormatter.swift`. The exact
/// output bytes form part of the user-visible CLI contract — keep this in
/// lockstep with the Swift source. The contract test
/// (`tests/fixtures.rs`) loads canonical JSON inputs and verifies
/// formatter output character-for-character.
pub struct AgentFormatter;

impl AgentFormatter {
    pub fn format_snapshot_json(data: &[u8]) -> serde_json::Result<String> {
        let response: SnapshotResponse = serde_json::from_slice(data)?;
        Ok(Self::format_snapshot(&response))
    }

    pub fn format_snapshot(response: &SnapshotResponse) -> String {
        let mut lines: Vec<String> = Vec::new();
        for window in &response.windows {
            lines.push(format_window_line(window));
            if let Some(elements) = &window.elements {
                for element in elements {
                    format_element(element, 1, &mut lines);
                }
            }
        }
        lines.join("\n")
    }

    pub fn format_windows_json(data: &[u8]) -> serde_json::Result<String> {
        let response: SnapshotResponse = serde_json::from_slice(data)?;
        Ok(Self::format_windows(&response))
    }

    pub fn format_windows(response: &SnapshotResponse) -> String {
        response
            .windows
            .iter()
            .map(format_window_line)
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn format_action_json(data: &[u8]) -> serde_json::Result<String> {
        let response: ActionResponse = serde_json::from_slice(data)?;
        Ok(Self::format_action(&response))
    }

    pub fn format_action(response: &ActionResponse) -> String {
        let prefix = if response.success { "OK" } else { "FAILED" };
        match &response.message {
            Some(msg) => format!("{prefix}: {msg}"),
            None => prefix.to_string(),
        }
    }

    pub fn format_error_json(data: &[u8]) -> serde_json::Result<String> {
        let response: ErrorResponse = serde_json::from_slice(data)?;
        Ok(Self::format_error(&response))
    }

    pub fn format_error(response: &ErrorResponse) -> String {
        match &response.details {
            // U+2014 EM DASH, matching the Swift implementation byte-for-byte.
            Some(d) => format!("Error: {} \u{2014} {}", response.error, d),
            None => format!("Error: {}", response.error),
        }
    }

    pub fn format_inspect_json(data: &[u8]) -> serde_json::Result<String> {
        let response: InspectResponse = serde_json::from_slice(data)?;
        Ok(Self::format_inspect(&response))
    }

    pub fn format_inspect(response: &InspectResponse) -> String {
        let mut lines: Vec<String> = Vec::new();
        lines.push(format_element_line(&response.element, 0));
        if let Some((x, y, w, h)) = response.bounds() {
            lines.push(format!(
                "  bounds: {},{} {}x{}",
                format_coordinate(x),
                format_coordinate(y),
                format_coordinate(w),
                format_coordinate(h),
            ));
        }
        let font_parts = build_font_parts(response);
        if !font_parts.is_empty() {
            lines.push(format!("  font: {}", font_parts.join(" ")));
        }
        lines.join("\n")
    }
}

fn format_window_line(window: &WindowInfo) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(title) = &window.title {
        parts.push(format!("\"{title}\""));
    }
    parts.push(format!("({})", window.window_type));
    parts.push(format!(
        "{}x{}",
        format_coordinate(window.size_width),
        format_coordinate(window.size_height)
    ));
    if window.focused {
        parts.push("[focused]".to_string());
    }
    parts.push(format!("app:\"{}\"", window.app_name));
    parts.join(" ")
}

fn format_element(element: &ElementInfo, indent: usize, lines: &mut Vec<String>) {
    lines.push(format_element_line(element, indent));
    if let Some(children) = &element.children {
        for child in children {
            format_element(child, indent + 1, lines);
        }
    }
}

fn format_element_line(element: &ElementInfo, indent: usize) -> String {
    let prefix = "  ".repeat(indent);
    let mut parts: Vec<String> = Vec::new();
    parts.push(element.role.as_wire_str().to_string());
    if let Some(label) = &element.label {
        parts.push(format!("\"{label}\""));
    }
    if !element.enabled {
        parts.push("[disabled]".to_string());
    }
    if element.focused {
        parts.push("[focused]".to_string());
    }
    if show_child_count(element.role) && element.child_count > 0 {
        parts.push(format!("{} items", element.child_count));
    }
    if let Some(value) = &element.value {
        parts.push(format!("value=\"{value}\""));
    }
    format!("{prefix}{}", parts.join(" "))
}

fn show_child_count(role: UnifiedRole) -> bool {
    matches!(
        role,
        UnifiedRole::List
            | UnifiedRole::Tree
            | UnifiedRole::TreeGrid
            | UnifiedRole::ListBox
            | UnifiedRole::ListGrid
    )
}

fn build_font_parts(response: &InspectResponse) -> Vec<String> {
    let mut parts: Vec<String> = Vec::new();
    if let Some(family) = &response.font_family {
        parts.push(family.clone());
    }
    if let Some(size) = response.font_size {
        parts.push(format!("{}pt", format_coordinate(size)));
    }
    if let Some(weight) = &response.font_weight {
        parts.push(weight.clone());
    }
    parts
}

/// Integer when whole, decimal otherwise. Mirrors Swift's
/// `formatCoordinate` exactly: NaN/Infinity fall through to the decimal
/// branch.
fn format_coordinate(value: f64) -> String {
    if value.is_finite() && value == value.round() {
        format!("{}", value as i64)
    } else {
        format!("{value}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::element_info::ElementInfo;
    use crate::window_info::WindowInfo;

    fn elem_min(role: UnifiedRole, label: Option<&str>) -> ElementInfo {
        ElementInfo {
            role,
            label: label.map(String::from),
            value: None,
            description: None,
            id: None,
            enabled: true,
            focused: false,
            showing: None,
            position_x: None,
            position_y: None,
            size_width: None,
            size_height: None,
            child_count: 0,
            actions: vec![],
            platform_role: None,
            children: None,
        }
    }

    #[test]
    fn snapshot_nested_elements() {
        let snap = SnapshotResponse {
            windows: vec![WindowInfo {
                title: Some("My App".into()),
                window_type: "window".into(),
                size_width: 800.0,
                size_height: 600.0,
                position_x: 0.0,
                position_y: 0.0,
                app_name: "Xcode".into(),
                focused: false,
                elements: Some(vec![ElementInfo {
                    role: UnifiedRole::Toolbar,
                    label: None,
                    value: None,
                    description: None,
                    id: None,
                    enabled: true,
                    focused: false,
                    showing: None,
                    position_x: None,
                    position_y: None,
                    size_width: None,
                    size_height: None,
                    child_count: 2,
                    actions: vec![],
                    platform_role: None,
                    children: Some(vec![
                        elem_min(UnifiedRole::Button, Some("New")),
                        elem_min(UnifiedRole::Button, Some("Save")),
                    ]),
                }]),
            }],
        };
        let expected = "\"My App\" (window) 800x600 app:\"Xcode\"\n  toolbar\n    button \"New\"\n    button \"Save\"";
        assert_eq!(AgentFormatter::format_snapshot(&snap), expected);
    }

    #[test]
    fn snapshot_disabled_and_focused_elements() {
        let mut undo = elem_min(UnifiedRole::Button, Some("Undo"));
        undo.enabled = false;
        let mut search = elem_min(UnifiedRole::Textfield, Some("Search..."));
        search.value = Some("".into());
        search.focused = true;

        let snap = SnapshotResponse {
            windows: vec![WindowInfo {
                title: Some("Editor".into()),
                window_type: "window".into(),
                size_width: 1024.0,
                size_height: 768.0,
                position_x: 0.0,
                position_y: 0.0,
                app_name: "TextEdit".into(),
                focused: true,
                elements: Some(vec![undo, search]),
            }],
        };
        let expected = "\"Editor\" (window) 1024x768 [focused] app:\"TextEdit\"\n  button \"Undo\" [disabled]\n  textfield \"Search...\" [focused] value=\"\"";
        assert_eq!(AgentFormatter::format_snapshot(&snap), expected);
    }

    #[test]
    fn snapshot_window_without_title() {
        let snap = SnapshotResponse {
            windows: vec![WindowInfo {
                title: None,
                window_type: "menu".into(),
                size_width: 200.0,
                size_height: 300.0,
                position_x: 50.0,
                position_y: 100.0,
                app_name: "Finder".into(),
                focused: false,
                elements: None,
            }],
        };
        assert_eq!(
            AgentFormatter::format_snapshot(&snap),
            "(menu) 200x300 app:\"Finder\""
        );
    }

    #[test]
    fn windows_listing() {
        let snap = SnapshotResponse {
            windows: vec![
                WindowInfo {
                    title: Some("My App - main.swift".into()),
                    window_type: "window".into(),
                    size_width: 800.0,
                    size_height: 600.0,
                    position_x: 0.0,
                    position_y: 0.0,
                    app_name: "Xcode".into(),
                    focused: true,
                    elements: None,
                },
                WindowInfo {
                    title: Some("Console".into()),
                    window_type: "window".into(),
                    size_width: 600.0,
                    size_height: 400.0,
                    position_x: 100.0,
                    position_y: 100.0,
                    app_name: "Terminal".into(),
                    focused: false,
                    elements: None,
                },
            ],
        };
        let expected = "\"My App - main.swift\" (window) 800x600 [focused] app:\"Xcode\"\n\"Console\" (window) 600x400 app:\"Terminal\"";
        assert_eq!(AgentFormatter::format_windows(&snap), expected);
    }

    #[test]
    fn action_success_no_message() {
        let response = ActionResponse {
            success: true,
            message: None,
        };
        assert_eq!(AgentFormatter::format_action(&response), "OK");
    }

    #[test]
    fn action_success_with_message() {
        let response = ActionResponse {
            success: true,
            message: Some("Pressed button \"Save\"".into()),
        };
        assert_eq!(
            AgentFormatter::format_action(&response),
            "OK: Pressed button \"Save\""
        );
    }

    #[test]
    fn action_failure_no_message() {
        let response = ActionResponse {
            success: false,
            message: None,
        };
        assert_eq!(AgentFormatter::format_action(&response), "FAILED");
    }

    #[test]
    fn action_failure_with_message() {
        let response = ActionResponse {
            success: false,
            message: Some("element not found".into()),
        };
        assert_eq!(
            AgentFormatter::format_action(&response),
            "FAILED: element not found"
        );
    }

    #[test]
    fn error_with_details() {
        let response = ErrorResponse {
            error: "elementNotFound".into(),
            details: Some("No element matched the given selector".into()),
        };
        assert_eq!(
            AgentFormatter::format_error(&response),
            "Error: elementNotFound \u{2014} No element matched the given selector"
        );
    }

    #[test]
    fn error_without_details() {
        let response = ErrorResponse {
            error: "timeout".into(),
            details: None,
        };
        assert_eq!(AgentFormatter::format_error(&response), "Error: timeout");
    }

    #[test]
    fn inspect_all_fields() {
        let mut element = elem_min(UnifiedRole::Button, Some("Save"));
        element.actions = vec!["AXPress".into()];
        let response = InspectResponse {
            element,
            font_family: Some("SF Pro Display".into()),
            font_size: Some(14.0),
            font_weight: Some("bold".into()),
            text_color: None,
            bounds_x: Some(100.0),
            bounds_y: Some(200.0),
            bounds_width: Some(400.0),
            bounds_height: Some(50.0),
        };
        let expected =
            "button \"Save\"\n  bounds: 100,200 400x50\n  font: SF Pro Display 14pt bold";
        assert_eq!(AgentFormatter::format_inspect(&response), expected);
    }

    #[test]
    fn inspect_minimal() {
        let mut element = elem_min(UnifiedRole::Group, None);
        element.child_count = 3;
        let response = InspectResponse {
            element,
            font_family: None,
            font_size: None,
            font_weight: None,
            text_color: None,
            bounds_x: None,
            bounds_y: None,
            bounds_width: None,
            bounds_height: None,
        };
        assert_eq!(AgentFormatter::format_inspect(&response), "group");
    }

    #[test]
    fn inspect_disabled_focused_textfield() {
        let mut element = elem_min(UnifiedRole::Textfield, Some("Email"));
        element.value = Some("test@example.com".into());
        element.enabled = false;
        element.focused = true;
        let response = InspectResponse {
            element,
            font_family: None,
            font_size: None,
            font_weight: None,
            text_color: None,
            bounds_x: Some(50.0),
            bounds_y: Some(100.0),
            bounds_width: Some(200.0),
            bounds_height: Some(30.0),
        };
        let expected = "textfield \"Email\" [disabled] [focused] value=\"test@example.com\"\n  bounds: 50,100 200x30";
        assert_eq!(AgentFormatter::format_inspect(&response), expected);
    }

    #[test]
    fn snapshot_list_with_child_count() {
        let mut list = ElementInfo {
            role: UnifiedRole::List,
            label: Some("Files".into()),
            value: None,
            description: None,
            id: None,
            enabled: true,
            focused: false,
            showing: None,
            position_x: None,
            position_y: None,
            size_width: None,
            size_height: None,
            child_count: 3,
            actions: vec![],
            platform_role: None,
            children: None,
        };
        list.children = Some(vec![
            elem_min(UnifiedRole::ListItem, Some("First")),
            elem_min(UnifiedRole::ListItem, Some("Second")),
            elem_min(UnifiedRole::ListItem, Some("Third")),
        ]);

        let snap = SnapshotResponse {
            windows: vec![WindowInfo {
                title: Some("Items".into()),
                window_type: "window".into(),
                size_width: 400.0,
                size_height: 300.0,
                position_x: 0.0,
                position_y: 0.0,
                app_name: "Finder".into(),
                focused: false,
                elements: Some(vec![list]),
            }],
        };
        let expected = "\"Items\" (window) 400x300 app:\"Finder\"\n  list \"Files\" 3 items\n    list-item \"First\"\n    list-item \"Second\"\n    list-item \"Third\"";
        assert_eq!(AgentFormatter::format_snapshot(&snap), expected);
    }
}
