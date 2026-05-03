use serde::{Deserialize, Serialize};

use crate::element_info::ElementInfo;
use crate::window_info::WindowInfo;

/// Response body for `GET /snapshot` and `GET /windows`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnapshotResponse {
    pub windows: Vec<WindowInfo>,
}

/// Response body for action endpoints (e.g. `POST /press`, `POST /click`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionResponse {
    pub success: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Response body for any endpoint that returned an HTTP 4xx/5xx error.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

/// Response body for `GET /inspect`.
///
/// Mirrors `InspectResponse` in Swift, including the flattened
/// `boundsX`/`boundsY`/`boundsWidth`/`boundsHeight` keys (rather than a
/// nested `bounds` object).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InspectResponse {
    pub element: ElementInfo,

    #[serde(rename = "fontFamily", default, skip_serializing_if = "Option::is_none")]
    pub font_family: Option<String>,

    #[serde(rename = "fontSize", default, skip_serializing_if = "Option::is_none")]
    pub font_size: Option<f64>,

    #[serde(rename = "fontWeight", default, skip_serializing_if = "Option::is_none")]
    pub font_weight: Option<String>,

    #[serde(rename = "textColor", default, skip_serializing_if = "Option::is_none")]
    pub text_color: Option<String>,

    #[serde(rename = "boundsX", default, skip_serializing_if = "Option::is_none")]
    pub bounds_x: Option<f64>,

    #[serde(rename = "boundsY", default, skip_serializing_if = "Option::is_none")]
    pub bounds_y: Option<f64>,

    #[serde(rename = "boundsWidth", default, skip_serializing_if = "Option::is_none")]
    pub bounds_width: Option<f64>,

    #[serde(rename = "boundsHeight", default, skip_serializing_if = "Option::is_none")]
    pub bounds_height: Option<f64>,
}

impl InspectResponse {
    /// Returns `(x, y, width, height)` only when all four bounds keys are
    /// present, mirroring Swift's `bounds: CGRect?` semantics.
    pub fn bounds(&self) -> Option<(f64, f64, f64, f64)> {
        match (self.bounds_x, self.bounds_y, self.bounds_width, self.bounds_height) {
            (Some(x), Some(y), Some(w), Some(h)) => Some((x, y, w, h)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unified_role::UnifiedRole;

    fn element() -> ElementInfo {
        ElementInfo {
            role: UnifiedRole::Textfield,
            label: Some("Search".into()),
            value: Some("hello".into()),
            description: None,
            id: Some("search-field".into()),
            enabled: true,
            focused: true,
            showing: None,
            position_x: Some(20.0),
            position_y: Some(10.0),
            size_width: Some(200.0),
            size_height: Some(24.0),
            child_count: 0,
            actions: vec!["AXConfirm".into()],
            platform_role: Some("AXTextField".into()),
            children: None,
        }
    }

    #[test]
    fn snapshot_round_trip() {
        let snapshot = SnapshotResponse {
            windows: vec![WindowInfo {
                title: Some("Browser".into()),
                window_type: "window".into(),
                size_width: 1200.0,
                size_height: 800.0,
                position_x: 0.0,
                position_y: 0.0,
                app_name: "Safari".into(),
                focused: true,
                elements: Some(vec![element()]),
            }],
        };
        let data = serde_json::to_string(&snapshot).unwrap();
        let decoded: SnapshotResponse = serde_json::from_str(&data).unwrap();
        assert_eq!(decoded, snapshot);
    }

    #[test]
    fn snapshot_empty_windows() {
        let snapshot = SnapshotResponse { windows: vec![] };
        let data = serde_json::to_string(&snapshot).unwrap();
        let decoded: SnapshotResponse = serde_json::from_str(&data).unwrap();
        assert_eq!(decoded, snapshot);
    }

    #[test]
    fn action_success_with_message() {
        let response = ActionResponse {
            success: true,
            message: Some("Clicked successfully".into()),
        };
        let data = serde_json::to_string(&response).unwrap();
        let decoded: ActionResponse = serde_json::from_str(&data).unwrap();
        assert_eq!(decoded, response);
    }

    #[test]
    fn action_failure_no_message() {
        let response = ActionResponse {
            success: false,
            message: None,
        };
        let data = serde_json::to_string(&response).unwrap();
        let decoded: ActionResponse = serde_json::from_str(&data).unwrap();
        assert_eq!(decoded, response);
    }

    #[test]
    fn error_round_trip() {
        let response = ErrorResponse {
            error: "elementNotFound".into(),
            details: Some("No element matched the given selector".into()),
        };
        let data = serde_json::to_string(&response).unwrap();
        let decoded: ErrorResponse = serde_json::from_str(&data).unwrap();
        assert_eq!(decoded, response);
    }

    #[test]
    fn error_no_details() {
        let response = ErrorResponse {
            error: "timeout".into(),
            details: None,
        };
        let data = serde_json::to_string(&response).unwrap();
        let decoded: ErrorResponse = serde_json::from_str(&data).unwrap();
        assert_eq!(decoded, response);
    }

    #[test]
    fn inspect_round_trip_all_fields() {
        let response = InspectResponse {
            element: ElementInfo {
                role: UnifiedRole::Text,
                label: Some("Hello World".into()),
                value: None,
                description: None,
                id: None,
                enabled: true,
                focused: false,
                showing: None,
                position_x: Some(5.0),
                position_y: Some(5.0),
                size_width: Some(100.0),
                size_height: Some(20.0),
                child_count: 0,
                actions: vec![],
                platform_role: Some("AXStaticText".into()),
                children: None,
            },
            font_family: Some("Helvetica Neue".into()),
            font_size: Some(13.0),
            font_weight: Some("regular".into()),
            text_color: Some("#000000".into()),
            bounds_x: Some(5.0),
            bounds_y: Some(5.0),
            bounds_width: Some(100.0),
            bounds_height: Some(20.0),
        };
        let data = serde_json::to_string(&response).unwrap();
        let decoded: InspectResponse = serde_json::from_str(&data).unwrap();
        assert_eq!(decoded, response);
    }

    #[test]
    fn inspect_minimal_fields() {
        let response = InspectResponse {
            element: ElementInfo {
                role: UnifiedRole::Button,
                label: Some("Submit".into()),
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
            },
            font_family: None,
            font_size: None,
            font_weight: None,
            text_color: None,
            bounds_x: None,
            bounds_y: None,
            bounds_width: None,
            bounds_height: None,
        };
        let data = serde_json::to_string(&response).unwrap();
        let decoded: InspectResponse = serde_json::from_str(&data).unwrap();
        assert_eq!(decoded, response);
    }

    #[test]
    fn inspect_keys_camel_case() {
        let response = InspectResponse {
            element: ElementInfo {
                role: UnifiedRole::Text,
                label: Some("Hi".into()),
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
            },
            font_family: Some("Arial".into()),
            font_size: Some(12.0),
            font_weight: Some("bold".into()),
            text_color: Some("#FF0000".into()),
            bounds_x: Some(0.0),
            bounds_y: Some(0.0),
            bounds_width: Some(50.0),
            bounds_height: Some(20.0),
        };
        let data = serde_json::to_string(&response).unwrap();
        let map: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&data).unwrap();
        for key in [
            "element",
            "fontFamily",
            "fontSize",
            "fontWeight",
            "textColor",
            "boundsX",
            "boundsY",
            "boundsWidth",
            "boundsHeight",
        ] {
            assert!(map.contains_key(key), "missing key: {key}");
        }
        for forbidden in ["font_family", "font_size", "font_weight", "text_color"] {
            assert!(!map.contains_key(forbidden), "snake_case leaked: {forbidden}");
        }
    }
}
