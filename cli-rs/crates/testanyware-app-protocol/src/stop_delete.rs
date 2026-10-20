//! `POST /v1/vms/{id}/stop` and `POST /v1/goldens/{name}/delete`: `vm stop` and `vm delete` in
//! the app (spec: "The protocol").
//!
//! The parser and the builder of the two paths live together, so a test can prove that they
//! agree. The VM id follows the grammar of the guest stream routes and is never decoded. A golden
//! name is percent-encoded in the path, because a golden may carry any character that a file
//! name allows. The app decodes it and then refuses a name that could leave the golden directory.

use percent_encoding::{percent_decode_str, utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use serde::{Deserialize, Serialize};

use crate::stream::is_plain_vm_id;

/// Bytes of a golden name that stay as they are in the path.
const GOLDEN_NAME_KEEP: &AsciiSet = &NON_ALPHANUMERIC.remove(b'-').remove(b'.').remove(b'_');

/// The path of the request that stops the VM `id`. The id must be a plain id.
pub fn stop_path(id: &str) -> String {
    format!("/v1/vms/{id}/stop")
}

/// The path of the request that deletes the golden image `name`.
pub fn delete_path(name: &str) -> String {
    format!(
        "/v1/goldens/{}/delete",
        utf8_percent_encode(name, GOLDEN_NAME_KEEP)
    )
}

/// The body of the request that deletes a golden: the one option of `vm delete` that changes what
/// the command does. `--dry-run` never reaches the app.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeleteRequest {
    pub force: bool,
}

/// What `vm stop` prints on success: `{"id": ..., "stopped": true}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StopResult {
    pub id: String,
    pub stopped: bool,
}

/// What `vm delete` prints on success: `{"name": ..., "deleted": true}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeleteResult {
    pub name: String,
    pub deleted: bool,
}

/// What a path names, among the two routes that end a VM or a golden.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EndRoute {
    Stop {
        id: String,
    },
    /// The segment is no plain VM id, so it names no VM. The app has not looked at the disk.
    BadVmId(String),
    Delete {
        name: String,
    },
    /// The decoded segment could not be the name of a golden image.
    BadGoldenName(String),
    /// The path is neither route.
    Neither,
}

/// Match `path` against the stop route and the delete route.
pub fn parse_end_path(path: &str) -> EndRoute {
    let Some(rest) = path.strip_prefix("/v1/") else {
        return EndRoute::Neither;
    };
    let segments: Vec<&str> = rest.split('/').collect();
    match segments.as_slice() {
        ["vms", id, "stop"] if is_plain_vm_id(id) => EndRoute::Stop {
            id: (*id).to_string(),
        },
        ["vms", id, "stop"] => EndRoute::BadVmId((*id).to_string()),
        ["goldens", name, "delete"] => {
            let name = percent_decode_str(name).decode_utf8_lossy().into_owned();
            if is_golden_file_name(&name) {
                EndRoute::Delete { name }
            } else {
                EndRoute::BadGoldenName(name)
            }
        }
        _ => EndRoute::Neither,
    }
}

/// The most bytes of a golden name. A file name has 255 bytes at most, and a golden adds a suffix,
/// so a longer name can name no golden. The Host CLI answers `GOLDEN_NOT_FOUND` for it without a
/// request, because the path of such a request would exceed what an HTTP request line allows.
pub const GOLDEN_NAME_MAX_BYTES: usize = 255;

/// A name that joins the golden directory without leaving it: it has no separator and no NUL,
/// it is no dot name, and it fits in a file name.
fn is_golden_file_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= GOLDEN_NAME_MAX_BYTES
        && name != "."
        && name != ".."
        && !name.contains(['/', '\0'])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stop_path_round_trips() {
        assert_eq!(
            parse_end_path(&stop_path("testanyware-ab12.cd_3")),
            EndRoute::Stop {
                id: "testanyware-ab12.cd_3".into()
            }
        );
    }

    #[test]
    fn an_id_is_never_decoded() {
        for id in ["..", ".hidden", "a%2Fb", "a%2fb", "", "a b", "../x"] {
            let route = parse_end_path(&format!("/v1/vms/{id}/stop"));
            match id {
                // An empty segment and a path with a slash do not even reach the id rule.
                "" => assert_eq!(route, EndRoute::BadVmId(String::new())),
                "../x" => assert_eq!(route, EndRoute::Neither),
                _ => assert_eq!(route, EndRoute::BadVmId(id.into()), "{id}"),
            }
        }
    }

    #[test]
    fn a_golden_name_round_trips_whatever_it_holds() {
        for name in [
            "testanyware-golden-windows-11",
            "my golden",
            "a:b",
            "100%",
            "ünï",
            "a?b#c",
        ] {
            assert_eq!(
                parse_end_path(&delete_path(name)),
                EndRoute::Delete { name: name.into() },
                "{name}"
            );
        }
    }

    #[test]
    fn a_name_that_could_leave_the_golden_directory_is_refused() {
        for (segment, name) in [
            ("%2E%2E", ".."),
            ("%2E", "."),
            ("..", ".."),
            ("a%2Fb", "a/b"),
            ("a%2fb", "a/b"),
            ("%2Fetc", "/etc"),
            ("a%00b", "a\0b"),
            ("", ""),
        ] {
            assert_eq!(
                parse_end_path(&format!("/v1/goldens/{segment}/delete")),
                EndRoute::BadGoldenName(name.into()),
                "{segment}"
            );
        }
    }

    #[test]
    fn a_name_that_starts_with_a_dot_is_a_golden_name() {
        for name in [".hidden", "...", ".x.y"] {
            assert_eq!(
                parse_end_path(&delete_path(name)),
                EndRoute::Delete { name: name.into() },
                "{name}"
            );
        }
    }

    #[test]
    fn a_name_longer_than_a_file_name_names_no_golden() {
        let longest = "a".repeat(GOLDEN_NAME_MAX_BYTES);
        assert_eq!(
            parse_end_path(&delete_path(&longest)),
            EndRoute::Delete { name: longest }
        );
        let too_long = "a".repeat(GOLDEN_NAME_MAX_BYTES + 1);
        assert_eq!(
            parse_end_path(&delete_path(&too_long)),
            EndRoute::BadGoldenName(too_long)
        );
    }

    #[test]
    fn other_paths_are_neither_route() {
        for path in [
            "/v1/vms",
            "/v1/vms/x",
            "/v1/vms/x/agent",
            "/v1/vms/x/stop/more",
            "/v1/goldens/x",
            "/v1/goldens/x/stop",
            "/v1/goldens/x/delete/more",
            "/v2/vms/x/stop",
            "/vms/x/stop",
        ] {
            assert_eq!(parse_end_path(path), EndRoute::Neither, "{path}");
        }
    }

    #[test]
    fn the_delete_request_and_the_results_have_the_members_that_the_commands_print() {
        assert_eq!(
            serde_json::to_value(DeleteRequest { force: true }).unwrap(),
            serde_json::json!({ "force": true })
        );
        assert!(serde_json::from_str::<DeleteRequest>("{}").is_err());
        assert_eq!(
            serde_json::to_value(StopResult {
                id: "a".into(),
                stopped: true
            })
            .unwrap(),
            serde_json::json!({ "id": "a", "stopped": true })
        );
        assert_eq!(
            serde_json::to_value(DeleteResult {
                name: "g".into(),
                deleted: true
            })
            .unwrap(),
            serde_json::json!({ "name": "g", "deleted": true })
        );
    }
}
