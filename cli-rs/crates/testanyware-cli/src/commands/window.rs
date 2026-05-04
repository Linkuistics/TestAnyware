//! `--window`-relative coordinate compensation for input subcommands.
//!
//! Ports `cli/Sources/TestAnywareDriver/Agent/WindowOriginCompensation.swift`
//! plus the `windowOffset(connection:spec:windowFilter:)` helper from
//! `cli/Sources/testanyware/InputCommand.swift`.
//!
//! On macOS Tahoe, the `kAXPositionAttribute` for an `AXWindow` includes
//! the structural drop-shadow inset, so taking the AX origin verbatim
//! makes every `--window`-relative click land ~40 px below the intended
//! target. Subtracting `defaultMacosTopInset` from the y origin restores
//! intent. `TESTANYWARE_WINDOW_TOP_INSET=<int>` overrides the constant
//! for tuning across other macOS versions or window subroles.

use serde_json::json;
use testanyware_agent_client::AgentClient;
use testanyware_protocol::WindowInfo;
use testanyware_rfb::Platform;

use crate::commands::{build_agent_client, exit_agent_error};
use crate::output::{print_error, OutputMode};
use crate::resolve::ConnectionOptions;

pub const DEFAULT_MACOS_TOP_INSET: i32 = 40;

/// Pure compensation. Returns the `(x, y)` to add to caller-supplied
/// window-relative coordinates to land on the absolute screen position.
pub fn compensate(window: &WindowInfo, platform: Option<Platform>, top_inset: Option<i32>) -> (i32, i32) {
    let base_x = window.position_x as i32;
    let base_y = window.position_y as i32;
    if platform != Some(Platform::Macos) {
        return (base_x, base_y);
    }
    let inset = top_inset.unwrap_or(DEFAULT_MACOS_TOP_INSET);
    (base_x, base_y - inset)
}

/// Read the override env var. Empty / unset / non-integer values fall
/// back to the default. Mirrors Swift's `.flatMap(Int.init)` behaviour:
/// only a parseable integer wins.
pub fn top_inset_from_env<F>(get: F) -> Option<i32>
where
    F: Fn(&str) -> Option<String>,
{
    get("TESTANYWARE_WINDOW_TOP_INSET")
        .and_then(|raw| raw.parse::<i32>().ok())
}

/// Resolve a `--window <filter>` to an `(offset_x, offset_y)` for one of
/// the six input subcommands. Returns `(0, 0)` when `filter` is `None`
/// to mirror the Swift helper exactly.
///
/// On agent / lookup failure, the function does not return — it prints
/// the §3.4 envelope and exits.
pub async fn resolve_window_offset(
    opts: &ConnectionOptions,
    platform: Option<Platform>,
    filter: Option<&str>,
    mode: OutputMode,
) -> (i32, i32) {
    let Some(filter) = filter else { return (0, 0) };
    let client = build_agent_client(opts, mode);
    let window = lookup_or_exit(&client, filter, mode).await;
    let inset = top_inset_from_env(|k| std::env::var(k).ok());
    compensate(&window, platform, inset)
}

async fn lookup_or_exit(client: &AgentClient, filter: &str, mode: OutputMode) -> WindowInfo {
    let response = match client.windows().await {
        Ok(r) => r,
        Err(err) => exit_agent_error(err, mode),
    };
    let needle = filter.to_lowercase();
    let matched = response.windows.into_iter().find(|w| {
        let title_match = w
            .title
            .as_deref()
            .map(|t| t.to_lowercase().contains(&needle))
            .unwrap_or(false);
        let app_match = w.app_name.to_lowercase().contains(&needle);
        title_match || app_match
    });
    match matched {
        Some(w) => w,
        None => print_error(
            mode,
            "WINDOW_NOT_FOUND",
            &format!("No window matching '{filter}'"),
            Some("Run `testanyware agent windows` to list available windows."),
            json!({ "filter": filter }),
            crate::output::exit_code_for("WINDOW_NOT_FOUND"),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window_at(x: f64, y: f64) -> WindowInfo {
        WindowInfo {
            title: Some("Doc".into()),
            window_type: "window".into(),
            size_width: 800.0,
            size_height: 600.0,
            position_x: x,
            position_y: y,
            app_name: "App".into(),
            focused: true,
            elements: None,
        }
    }

    #[test]
    fn macos_subtracts_default_inset_from_y() {
        let w = window_at(100.0, 80.0);
        let (x, y) = compensate(&w, Some(Platform::Macos), None);
        assert_eq!(x, 100);
        assert_eq!(y, 80 - DEFAULT_MACOS_TOP_INSET);
    }

    #[test]
    fn macos_honours_env_override() {
        let w = window_at(100.0, 80.0);
        let (_, y) = compensate(&w, Some(Platform::Macos), Some(0));
        assert_eq!(y, 80, "TESTANYWARE_WINDOW_TOP_INSET=0 disables the inset");
        let (_, y) = compensate(&w, Some(Platform::Macos), Some(64));
        assert_eq!(y, 80 - 64, "non-zero override wins over the default");
    }

    #[test]
    fn linux_passes_through_unchanged() {
        let w = window_at(100.0, 80.0);
        let (x, y) = compensate(&w, Some(Platform::Linux), None);
        assert_eq!((x, y), (100, 80));
    }

    #[test]
    fn windows_passes_through_unchanged() {
        let w = window_at(100.0, 80.0);
        let (x, y) = compensate(&w, Some(Platform::Windows), None);
        assert_eq!((x, y), (100, 80));
    }

    #[test]
    fn nil_platform_passes_through_unchanged() {
        // Swift parity: `WindowOriginCompensation` short-circuits on any
        // non-macOS platform, including the absent case.
        let w = window_at(100.0, 80.0);
        let (x, y) = compensate(&w, None, None);
        assert_eq!((x, y), (100, 80));
    }

    #[test]
    fn fractional_origin_truncates_toward_zero_like_swift_int_init() {
        // Swift: `Int(window.position.x)` truncates toward zero. f64 → i32
        // cast in Rust does the same for in-range values.
        let w = window_at(100.9, 80.5);
        let (x, y) = compensate(&w, Some(Platform::Linux), None);
        assert_eq!((x, y), (100, 80));
    }

    #[test]
    fn env_override_parses_signed_integer() {
        let inset = top_inset_from_env(|k| {
            (k == "TESTANYWARE_WINDOW_TOP_INSET").then(|| "12".to_string())
        });
        assert_eq!(inset, Some(12));
        let inset = top_inset_from_env(|k| {
            (k == "TESTANYWARE_WINDOW_TOP_INSET").then(|| "-8".to_string())
        });
        assert_eq!(inset, Some(-8));
    }

    #[test]
    fn env_override_rejects_non_integer() {
        let inset = top_inset_from_env(|k| {
            (k == "TESTANYWARE_WINDOW_TOP_INSET").then(|| "abc".to_string())
        });
        assert_eq!(inset, None);
        let inset = top_inset_from_env(|k| {
            (k == "TESTANYWARE_WINDOW_TOP_INSET").then(|| "".to_string())
        });
        assert_eq!(inset, None);
    }

    #[test]
    fn env_override_returns_none_when_unset() {
        let inset = top_inset_from_env(|_| None);
        assert_eq!(inset, None);
    }
}
