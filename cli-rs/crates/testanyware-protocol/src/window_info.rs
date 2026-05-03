use serde::{Deserialize, Serialize};

use crate::element_info::ElementInfo;

/// Window descriptor returned by the in-VM agent.
///
/// Mirrors `WindowInfo` in
/// `cli/Sources/TestAnywareAgentProtocol/WindowInfo.swift`. Unlike
/// `ElementInfo`, the size and position are required (never absent on the
/// wire).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WindowInfo {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    #[serde(rename = "windowType")]
    pub window_type: String,

    #[serde(rename = "sizeWidth")]
    pub size_width: f64,

    #[serde(rename = "sizeHeight")]
    pub size_height: f64,

    #[serde(rename = "positionX")]
    pub position_x: f64,

    #[serde(rename = "positionY")]
    pub position_y: f64,

    #[serde(rename = "appName")]
    pub app_name: String,

    pub focused: bool,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elements: Option<Vec<ElementInfo>>,
}

impl WindowInfo {
    pub fn position(&self) -> (f64, f64) {
        (self.position_x, self.position_y)
    }

    pub fn size(&self) -> (f64, f64) {
        (self.size_width, self.size_height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unified_role::UnifiedRole;

    #[test]
    fn round_trip_with_title() {
        let window = WindowInfo {
            title: Some("My App — Document".into()),
            window_type: "window".into(),
            size_width: 1024.0,
            size_height: 768.0,
            position_x: 0.0,
            position_y: 0.0,
            app_name: "MyApp".into(),
            focused: true,
            elements: None,
        };
        let data = serde_json::to_string(&window).unwrap();
        let decoded: WindowInfo = serde_json::from_str(&data).unwrap();
        assert_eq!(decoded, window);
    }

    #[test]
    fn round_trip_without_title() {
        let window = WindowInfo {
            title: None,
            window_type: "menu".into(),
            size_width: 200.0,
            size_height: 300.0,
            position_x: 50.0,
            position_y: 100.0,
            app_name: "MyApp".into(),
            focused: false,
            elements: None,
        };
        let data = serde_json::to_string(&window).unwrap();
        let decoded: WindowInfo = serde_json::from_str(&data).unwrap();
        assert_eq!(decoded, window);
    }

    #[test]
    fn round_trip_with_elements() {
        let element = ElementInfo {
            role: UnifiedRole::Button,
            label: Some("Close".into()),
            value: None,
            description: None,
            id: None,
            enabled: true,
            focused: false,
            showing: None,
            position_x: Some(8.0),
            position_y: Some(8.0),
            size_width: Some(14.0),
            size_height: Some(14.0),
            child_count: 0,
            actions: vec!["AXPress".into()],
            platform_role: None,
            children: None,
        };
        let window = WindowInfo {
            title: Some("My Window".into()),
            window_type: "window".into(),
            size_width: 800.0,
            size_height: 600.0,
            position_x: 100.0,
            position_y: 50.0,
            app_name: "MyApp".into(),
            focused: true,
            elements: Some(vec![element]),
        };
        let data = serde_json::to_string(&window).unwrap();
        let decoded: WindowInfo = serde_json::from_str(&data).unwrap();
        assert_eq!(decoded, window);
    }
}
