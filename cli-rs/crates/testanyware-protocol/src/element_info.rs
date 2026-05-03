use serde::{Deserialize, Serialize};

use crate::unified_role::UnifiedRole;

/// Accessibility-tree element returned by the in-VM agent.
///
/// Mirrors `ElementInfo` in
/// `cli/Sources/TestAnywareAgentProtocol/ElementInfo.swift`. Position and
/// size are encoded as flattened per-axis keys (`positionX`, `positionY`,
/// `sizeWidth`, `sizeHeight`) — a quirk of the original Swift Codable
/// implementation that the Rust types must preserve byte-for-byte.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ElementInfo {
    pub role: UnifiedRole,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    pub enabled: bool,
    pub focused: bool,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub showing: Option<bool>,

    #[serde(rename = "positionX", default, skip_serializing_if = "Option::is_none")]
    pub position_x: Option<f64>,

    #[serde(rename = "positionY", default, skip_serializing_if = "Option::is_none")]
    pub position_y: Option<f64>,

    #[serde(rename = "sizeWidth", default, skip_serializing_if = "Option::is_none")]
    pub size_width: Option<f64>,

    #[serde(rename = "sizeHeight", default, skip_serializing_if = "Option::is_none")]
    pub size_height: Option<f64>,

    #[serde(rename = "childCount")]
    pub child_count: i64,

    pub actions: Vec<String>,

    #[serde(rename = "platformRole", default, skip_serializing_if = "Option::is_none")]
    pub platform_role: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<ElementInfo>>,
}

impl ElementInfo {
    /// Returns `(x, y)` only when both axes are present, mirroring Swift's
    /// `position: CGPoint?` semantics.
    pub fn position(&self) -> Option<(f64, f64)> {
        match (self.position_x, self.position_y) {
            (Some(x), Some(y)) => Some((x, y)),
            _ => None,
        }
    }

    /// Returns `(width, height)` only when both dimensions are present.
    pub fn size(&self) -> Option<(f64, f64)> {
        match (self.size_width, self.size_height) {
            (Some(w), Some(h)) => Some((w, h)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full_element() -> ElementInfo {
        ElementInfo {
            role: UnifiedRole::Button,
            label: Some("OK".to_string()),
            value: Some("pressed".to_string()),
            description: Some("Confirms the dialog".to_string()),
            id: Some("btn-ok".to_string()),
            enabled: true,
            focused: false,
            showing: None,
            position_x: Some(10.5),
            position_y: Some(20.0),
            size_width: Some(80.0),
            size_height: Some(30.0),
            child_count: 0,
            actions: vec!["AXPress".into(), "AXShowMenu".into()],
            platform_role: Some("AXButton".into()),
            children: None,
        }
    }

    #[test]
    fn round_trip_all_fields() {
        let element = full_element();
        let data = serde_json::to_string(&element).unwrap();
        let decoded: ElementInfo = serde_json::from_str(&data).unwrap();
        assert_eq!(decoded, element);
    }

    #[test]
    fn round_trip_optional_nil() {
        let element = ElementInfo {
            role: UnifiedRole::Unknown,
            label: None,
            value: None,
            description: None,
            id: None,
            enabled: false,
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
        };
        let data = serde_json::to_string(&element).unwrap();
        let decoded: ElementInfo = serde_json::from_str(&data).unwrap();
        assert_eq!(decoded, element);
    }

    #[test]
    fn keys_are_camel_case() {
        let element = full_element();
        let json: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&element).unwrap()).unwrap();
        let map = json.as_object().unwrap();
        for key in [
            "role",
            "label",
            "enabled",
            "focused",
            "childCount",
            "actions",
            "platformRole",
            "positionX",
            "positionY",
            "sizeWidth",
            "sizeHeight",
        ] {
            assert!(map.contains_key(key), "missing key: {key}");
        }
        for forbidden in ["child_count", "platform_role", "position_x", "size_width"] {
            assert!(!map.contains_key(forbidden), "snake_case leaked: {forbidden}");
        }
    }
}
