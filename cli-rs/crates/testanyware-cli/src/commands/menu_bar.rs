//! Pure, side-effect-free helpers for the `agent snapshot --open-menu <path>`
//! orchestration: parse the menu path, locate a menu-bar element by label, and
//! derive its click point. Kept separate from the VNC/agent I/O (in
//! `agent::run_snapshot`) so the search/centering logic is unit-testable
//! without a live VM.
//!
//! Ports the Swift `MenuBarLocator` enum
//! (`cli/Sources/TestAnywareDriver/Agent/MenuBarLocator.swift`).

use testanyware_protocol::{ElementInfo, WindowInfo};

/// Split a comma-separated `--open-menu` path ("File, Open Recent") into ordered
/// segments, trimming whitespace around each. Returns `None` when the input is
/// empty or any segment is blank — the caller treats that as a usage error
/// (parity with Swift `MenuBarLocator.parsePath`, which rejects rather than
/// silently dropping blank segments).
pub fn parse_path(raw: &str) -> Option<Vec<String>> {
    let segments: Vec<String> = raw.split(',').map(|s| s.trim().to_string()).collect();
    // `split` always yields at least one item, so an empty input surfaces as a
    // single blank segment — the `any(is_empty)` check rejects both cases.
    if segments.iter().any(String::is_empty) {
        return None;
    }
    Some(segments)
}

/// Depth-first search for the first element whose `label` equals `target`
/// (case-insensitive), across every window's element subtree. Mirrors Swift's
/// label-only match (it does not consider any other field).
pub fn find_element_by_label<'a>(
    target: &str,
    windows: &'a [WindowInfo],
) -> Option<&'a ElementInfo> {
    let needle = target.to_lowercase();
    windows
        .iter()
        .find_map(|window| window.elements.as_deref().and_then(|els| search(&needle, els)))
}

/// DFS one element list for the first label match (`needle` already lowercased).
fn search<'a>(needle: &str, elements: &'a [ElementInfo]) -> Option<&'a ElementInfo> {
    for element in elements {
        if element.label.as_ref().is_some_and(|l| l.to_lowercase() == needle) {
            return Some(element);
        }
        if let Some(hit) = element.children.as_deref().and_then(|c| search(needle, c)) {
            return Some(hit);
        }
    }
    None
}

/// Center point `(x, y)` of the element's frame, rounded to integer screen
/// coordinates. Returns `None` when either position or size is unavailable —
/// without both, no click target can be derived (parity with Swift
/// `centerPoint`).
pub fn center_point(element: &ElementInfo) -> Option<(i32, i32)> {
    let (px, py) = element.position()?;
    let (sw, sh) = element.size()?;
    Some((
        (px + sw / 2.0).round() as i32,
        (py + sh / 2.0).round() as i32,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use testanyware_protocol::UnifiedRole;

    fn el(label: Option<&str>) -> ElementInfo {
        ElementInfo {
            role: UnifiedRole::Unknown,
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
            actions: Vec::new(),
            platform_role: None,
            children: None,
        }
    }

    fn menu_window(elements: Vec<ElementInfo>) -> WindowInfo {
        WindowInfo {
            title: Some("Menu Bar".into()),
            window_type: "menu".into(),
            size_width: 0.0,
            size_height: 0.0,
            position_x: 0.0,
            position_y: 0.0,
            app_name: "App".into(),
            focused: false,
            elements: Some(elements),
        }
    }

    #[test]
    fn parse_path_splits_and_trims() {
        assert_eq!(
            parse_path("File, Open Recent"),
            Some(vec!["File".to_string(), "Open Recent".to_string()])
        );
        assert_eq!(parse_path("File"), Some(vec!["File".to_string()]));
    }

    #[test]
    fn parse_path_rejects_empty_input() {
        assert_eq!(parse_path(""), None);
    }

    #[test]
    fn parse_path_rejects_blank_segment() {
        assert_eq!(parse_path("File,,Edit"), None);
        assert_eq!(parse_path(" , "), None);
    }

    #[test]
    fn find_matches_label_case_insensitively() {
        let windows = vec![menu_window(vec![el(Some("File"))])];
        let found = find_element_by_label("file", &windows).expect("should find File");
        assert_eq!(found.label.as_deref(), Some("File"));
    }

    #[test]
    fn find_descends_into_children() {
        let mut parent = el(Some("File"));
        parent.children = Some(vec![el(Some("Open Recent"))]);
        let windows = vec![menu_window(vec![parent])];
        let found =
            find_element_by_label("open recent", &windows).expect("should find nested item");
        assert_eq!(found.label.as_deref(), Some("Open Recent"));
    }

    #[test]
    fn find_returns_none_when_absent() {
        let windows = vec![menu_window(vec![el(Some("Edit"))])];
        assert!(find_element_by_label("File", &windows).is_none());
    }

    #[test]
    fn center_point_rounds_frame_center() {
        let mut item = el(Some("File"));
        item.position_x = Some(10.0);
        item.position_y = Some(0.0);
        item.size_width = Some(41.0);
        item.size_height = Some(24.0);
        // center = (10 + 20.5, 0 + 12) = (30.5, 12) → rounds to (31, 12)
        assert_eq!(center_point(&item), Some((31, 12)));
    }

    #[test]
    fn center_point_none_without_size() {
        let mut item = el(Some("File"));
        item.position_x = Some(10.0);
        item.position_y = Some(0.0);
        assert_eq!(center_point(&item), None);
    }

    #[test]
    fn center_point_none_without_position() {
        let mut item = el(Some("File"));
        item.size_width = Some(40.0);
        item.size_height = Some(24.0);
        assert_eq!(center_point(&item), None);
    }
}
