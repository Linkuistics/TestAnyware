//! What a client states in the headers of each request.

use std::path::PathBuf;

use percent_encoding::{percent_decode_str, utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use serde::{Deserialize, Serialize};

use crate::version::ProtocolRange;

/// Header of a request and of a response: the sender's protocol range, as `MIN-MAX`.
pub const PROTOCOL_HEADER: &str = "x-testanyware-protocol";
/// Header of a request and of a response: the sender's build version.
pub const BUILD_HEADER: &str = "x-testanyware-build";
/// Header of a request: the state directory the Host CLI resolved.
pub const STATE_DIR_HEADER: &str = "x-testanyware-state-dir";
/// Header of a request: the data directory the Host CLI resolved.
pub const DATA_DIR_HEADER: &str = "x-testanyware-data-dir";

/// Characters that stay literal in a directory header. Every other byte is percent-encoded,
/// so a path with spaces or non-ASCII characters is a valid header value.
const PATH_LITERAL: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'/')
    .remove(b'.')
    .remove(b'-')
    .remove(b'_');

/// The state and data directories of one side.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Directories {
    pub state_dir: PathBuf,
    pub data_dir: PathBuf,
}

/// The identity a client states in its request headers. A field is `None` when the
/// header is missing or malformed.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ClientHello {
    pub protocol_range: Option<ProtocolRange>,
    pub build: Option<String>,
    pub directories: Option<Directories>,
}

impl ClientHello {
    pub fn new(
        protocol_range: ProtocolRange,
        build: impl Into<String>,
        directories: Directories,
    ) -> Self {
        Self {
            protocol_range: Some(protocol_range),
            build: Some(build.into()),
            directories: Some(directories),
        }
    }

    /// Read the hello from request headers. `get` returns the value of a header by its
    /// lower-case name.
    pub fn from_lookup<'a>(get: impl Fn(&str) -> Option<&'a str>) -> Self {
        let decode = |name: &str| {
            get(name).map(|v| PathBuf::from(percent_decode_str(v).decode_utf8_lossy().into_owned()))
        };
        let directories = match (decode(STATE_DIR_HEADER), decode(DATA_DIR_HEADER)) {
            (Some(state_dir), Some(data_dir)) => Some(Directories {
                state_dir,
                data_dir,
            }),
            _ => None,
        };
        Self {
            protocol_range: get(PROTOCOL_HEADER).and_then(|v| v.parse().ok()),
            build: get(BUILD_HEADER).map(str::to_string),
            directories,
        }
    }

    /// The request headers that state this hello. A `None` field gives no header.
    pub fn to_headers(&self) -> Vec<(&'static str, String)> {
        let mut headers = Vec::new();
        if let Some(range) = self.protocol_range {
            headers.push((PROTOCOL_HEADER, range.to_string()));
        }
        if let Some(build) = &self.build {
            headers.push((BUILD_HEADER, build.clone()));
        }
        if let Some(dirs) = &self.directories {
            headers.push((STATE_DIR_HEADER, encode_path(&dirs.state_dir)));
            headers.push((DATA_DIR_HEADER, encode_path(&dirs.data_dir)));
        }
        headers
    }
}

fn encode_path(path: &std::path::Path) -> String {
    utf8_percent_encode(&path.to_string_lossy(), PATH_LITERAL).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lookup(headers: &[(&'static str, String)]) -> ClientHello {
        ClientHello::from_lookup(|name| {
            headers
                .iter()
                .find(|(k, _)| *k == name)
                .map(|(_, v)| v.as_str())
        })
    }

    #[test]
    fn hello_round_trips_through_headers() {
        let hello = ClientHello::new(
            ProtocolRange::new(1, 3),
            "0.4.2",
            Directories {
                state_dir: "/s/state".into(),
                data_dir: "/d/data".into(),
            },
        );
        assert_eq!(lookup(&hello.to_headers()), hello);
    }

    #[test]
    fn directories_with_spaces_and_non_ascii_round_trip() {
        let hello = ClientHello::new(
            ProtocolRange::new(1, 1),
            "x",
            Directories {
                state_dir: "/Users/Zoë Q/Library/Application Support".into(),
                data_dir: "/d".into(),
            },
        );
        let headers = hello.to_headers();
        assert!(headers.iter().all(|(_, v)| v.is_ascii()));
        assert_eq!(lookup(&headers), hello);
    }

    #[test]
    fn missing_headers_give_none() {
        assert_eq!(lookup(&[]), ClientHello::default());
    }

    #[test]
    fn one_directory_header_alone_gives_no_directories() {
        let hello = lookup(&[(STATE_DIR_HEADER, "/s".to_string())]);
        assert_eq!(hello.directories, None);
    }

    #[test]
    fn malformed_range_gives_none() {
        let hello = lookup(&[(PROTOCOL_HEADER, "3-1".to_string())]);
        assert_eq!(hello.protocol_range, None);
    }
}
