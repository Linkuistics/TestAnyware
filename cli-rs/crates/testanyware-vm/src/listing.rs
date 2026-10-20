//! The `vm list` view: the rows, the filter and the limit.
//!
//! The Host CLI builds the view on a Linux or Windows host. The TestAnyware app builds it on a
//! macOS host and sends it as JSON, so the one view serves both paths and the Host CLI prints
//! it the same way (spec: "The protocol").

use serde::{Deserialize, Serialize};

use crate::lifecycle::VmListing;

/// One row of `vm list`: a golden image or a running clone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListItem {
    pub kind: String,
    pub name: String,
    pub platform: String,
    pub backend: String,
    pub pid: Option<i32>,
    pub vnc: Option<String>,
    pub agent: Option<String>,
}

/// The rows that `vm list` shows, with the counts that its `--json` output states.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListView {
    pub items: Vec<ListItem>,
    pub returned: usize,
    pub total: usize,
    pub truncated: bool,
}

/// Flatten a `VmListing` into the unified row form: the goldens, then the running clones.
pub fn listing_items(listing: &VmListing) -> Vec<ListItem> {
    let mut out = Vec::new();
    for g in &listing.goldens {
        out.push(ListItem {
            kind: "golden".into(),
            name: g.name.clone(),
            platform: g.platform.clone(),
            backend: g.backend.into(),
            pid: None,
            vnc: None,
            agent: None,
        });
    }
    for r in &listing.running {
        out.push(ListItem {
            kind: "running".into(),
            name: r.id.clone(),
            platform: r.platform.clone(),
            backend: r.backend.into(),
            pid: r.pid,
            vnc: r.vnc.clone(),
            agent: r.agent.clone(),
        });
    }
    out
}

/// Parse `--filter` into `(field, value)` pairs. Errors on a pair lacking `=`.
pub fn parse_filter(raw: &str) -> Result<Vec<(String, String)>, String> {
    if raw.is_empty() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for pair in raw.split(',') {
        let (k, v) = pair
            .split_once('=')
            .ok_or_else(|| format!("'{pair}' is not a field=value pair"))?;
        out.push((k.trim().to_string(), v.trim().to_string()));
    }
    Ok(out)
}

/// True if `item` satisfies every filter. Unknown fields never match.
pub fn filter_matches(item: &ListItem, filters: &[(String, String)]) -> bool {
    filters.iter().all(|(field, value)| match field.as_str() {
        "kind" => item.kind == *value,
        "platform" => item.platform == *value,
        "backend" => item.backend == *value,
        "name" => item.name == *value,
        _ => false,
    })
}

/// Apply the §9.4 list limit. Returns `(shown, returned, total, truncated)`.
pub fn apply_limit<T>(items: Vec<T>, limit: usize, all: bool) -> (Vec<T>, usize, usize, bool) {
    let total = items.len();
    if all || total <= limit {
        return (items, total, total, false);
    }
    let shown: Vec<T> = items.into_iter().take(limit).collect();
    let returned = shown.len();
    (shown, returned, total, true)
}

/// The view of `listing` under the options of `vm list`.
pub fn build_view(
    listing: &VmListing,
    filters: &[(String, String)],
    limit: usize,
    all: bool,
) -> ListView {
    let items: Vec<ListItem> = listing_items(listing)
        .into_iter()
        .filter(|item| filter_matches(item, filters))
        .collect();
    let (items, returned, total, truncated) = apply_limit(items, limit, all);
    ListView {
        items,
        returned,
        total,
        truncated,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lifecycle::RunningEntry;
    use crate::qemu::GoldenImage;

    fn item(platform: &str) -> ListItem {
        ListItem {
            kind: "running".into(),
            name: "testanyware-aa".into(),
            platform: platform.into(),
            backend: "qemu".into(),
            pid: Some(7),
            vnc: None,
            agent: None,
        }
    }

    #[test]
    fn parse_filter_reads_comma_separated_pairs() {
        let f = parse_filter("platform=windows,backend=qemu").unwrap();
        assert_eq!(
            f,
            vec![
                ("platform".to_string(), "windows".to_string()),
                ("backend".to_string(), "qemu".to_string()),
            ]
        );
    }

    #[test]
    fn parse_filter_rejects_a_pair_without_equals() {
        assert!(parse_filter("platform").is_err());
        assert!(parse_filter("platform=windows,oops").is_err());
    }

    #[test]
    fn parse_filter_empty_string_is_no_filters() {
        assert_eq!(parse_filter("").unwrap(), Vec::<(String, String)>::new());
    }

    #[test]
    fn filter_matches_compares_known_fields() {
        let item = item("windows");
        assert!(filter_matches(&item, &[("platform".into(), "windows".into())]));
        assert!(!filter_matches(&item, &[("platform".into(), "linux".into())]));
        assert!(filter_matches(&item, &[("kind".into(), "running".into())]));
        // Unknown fields never match — surfaces a typo as an empty result.
        assert!(!filter_matches(&item, &[("colour".into(), "blue".into())]));
    }

    #[test]
    fn apply_limit_truncates_and_flags() {
        let items: Vec<u8> = (0..150).collect();
        let (shown, returned, total, truncated) = apply_limit(items.clone(), 100, false);
        assert_eq!(returned, 100);
        assert_eq!(total, 150);
        assert!(truncated);
        assert_eq!(shown.len(), 100);

        let (shown_all, returned_all, total_all, truncated_all) = apply_limit(items, 100, true);
        assert_eq!(returned_all, 150);
        assert_eq!(total_all, 150);
        assert!(!truncated_all);
        assert_eq!(shown_all.len(), 150);
    }

    fn listing() -> VmListing {
        VmListing {
            goldens: vec![GoldenImage {
                name: "testanyware-golden-windows-11".into(),
                platform: "windows".into(),
                backend: "qemu",
            }],
            running: vec![RunningEntry {
                id: "testanyware-aa".into(),
                platform: "linux".into(),
                backend: "qemu",
                pid: Some(7),
                vnc: Some("127.0.0.1:5900".into()),
                agent: Some("127.0.0.1:8648".into()),
            }],
        }
    }

    #[test]
    fn the_view_lists_goldens_then_running_clones() {
        let view = build_view(&listing(), &[], 100, false);
        let rows: Vec<(&str, &str)> = view
            .items
            .iter()
            .map(|i| (i.kind.as_str(), i.name.as_str()))
            .collect();
        assert_eq!(
            rows,
            [
                ("golden", "testanyware-golden-windows-11"),
                ("running", "testanyware-aa"),
            ]
        );
        assert_eq!((view.returned, view.total, view.truncated), (2, 2, false));
        assert_eq!(view.items[1].pid, Some(7));
    }

    #[test]
    fn the_view_counts_the_rows_after_the_filter_and_before_the_limit() {
        let filters = parse_filter("backend=qemu").unwrap();
        let view = build_view(&listing(), &filters, 1, false);
        assert_eq!((view.returned, view.total, view.truncated), (1, 2, true));
        assert_eq!(view.items[0].kind, "golden");
        let none = build_view(&listing(), &parse_filter("platform=macos").unwrap(), 1, false);
        assert_eq!((none.returned, none.total, none.truncated), (0, 0, false));
    }

    #[test]
    fn the_view_round_trips_through_json() {
        let view = build_view(&listing(), &[], 100, false);
        let json = serde_json::to_string(&view).unwrap();
        assert_eq!(serde_json::from_str::<ListView>(&json).unwrap(), view);
    }
}
