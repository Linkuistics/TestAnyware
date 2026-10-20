//! The guest stream routes: what the Host CLI asks for and what the app serves.
//!
//! The parser and the builder live together, so a test can prove that they agree. The parser
//! never decodes a VM id. A strict character set rejects `..` and `%2F` before any path exists.

use std::net::{IpAddr, Ipv6Addr};

use percent_encoding::{percent_decode_str, utf8_percent_encode, AsciiSet, CONTROLS};

/// The `Upgrade` token of a guest stream request and of its `101` response.
pub const STREAM_UPGRADE_TOKEN: &str = "testanyware-stream";

/// A VM id is at most this many bytes.
const VM_ID_MAX_BYTES: usize = 128;

/// Bytes that a path segment of an endpoint must encode. Brackets and colons stay legal in
/// HTTP paths in practice, but an IPv6 literal is safest with its brackets encoded.
const ENDPOINT_ENCODE: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'%')
    .add(b'/')
    .add(b'<')
    .add(b'>')
    .add(b'?')
    .add(b'[')
    .add(b']')
    .add(b'`')
    .add(b'{')
    .add(b'}');

/// Which guest protocol a stream carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamKind {
    Agent,
    Vnc,
}

impl StreamKind {
    pub fn as_str(self) -> &'static str {
        match self {
            StreamKind::Agent => "agent",
            StreamKind::Vnc => "vnc",
        }
    }

    fn parse(segment: &str) -> Option<Self> {
        match segment {
            "agent" => Some(StreamKind::Agent),
            "vnc" => Some(StreamKind::Vnc),
            _ => None,
        }
    }
}

/// Where a stream goes: the endpoint of a VM in the state directory, or an endpoint that the
/// request names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamTarget {
    Vm { id: String },
    Endpoint { host: String, port: u16 },
}

impl StreamTarget {
    /// The request path for a stream of `kind` to this target.
    pub fn path(&self, kind: StreamKind) -> String {
        match self {
            StreamTarget::Vm { id } => format!("/v1/vms/{id}/{}", kind.as_str()),
            StreamTarget::Endpoint { host, port } => {
                let authority = match host.parse::<Ipv6Addr>() {
                    Ok(_) => format!("[{host}]:{port}"),
                    Err(_) => format!("{host}:{port}"),
                };
                let segment = utf8_percent_encode(&authority, ENDPOINT_ENCODE);
                format!("/v1/endpoints/{segment}/{}", kind.as_str())
            }
        }
    }
}

/// What a request path is, as the stream grammar sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteMatch {
    /// The path is no guest stream route.
    NotAStreamRoute,
    Stream(StreamTarget, StreamKind),
    /// A guest stream route whose VM id is not a plain id.
    BadVmId(String),
    /// A guest stream route whose endpoint is not `host:port`.
    BadEndpoint {
        value: String,
        reason: String,
    },
}

/// Match `path` against the four guest stream routes.
pub fn parse_stream_path(path: &str) -> RouteMatch {
    let Some(rest) = path.strip_prefix("/v1/") else {
        return RouteMatch::NotAStreamRoute;
    };
    let segments: Vec<&str> = rest.split('/').collect();
    match segments.as_slice() {
        ["vms", id, kind] => match StreamKind::parse(kind) {
            None => RouteMatch::NotAStreamRoute,
            Some(kind) if is_plain_vm_id(id) => RouteMatch::Stream(
                StreamTarget::Vm {
                    id: (*id).to_string(),
                },
                kind,
            ),
            Some(_) => RouteMatch::BadVmId((*id).to_string()),
        },
        ["endpoints", target, kind] => match StreamKind::parse(kind) {
            None => RouteMatch::NotAStreamRoute,
            Some(kind) => {
                let value = percent_decode_str(target).decode_utf8_lossy().into_owned();
                match parse_endpoint(&value) {
                    Ok((host, port)) => {
                        RouteMatch::Stream(StreamTarget::Endpoint { host, port }, kind)
                    }
                    Err(reason) => RouteMatch::BadEndpoint { value, reason },
                }
            }
        },
        _ => RouteMatch::NotAStreamRoute,
    }
}

/// A plain VM id: ASCII letters, digits, `.`, `-` and `_`, and no leading dot. It joins a
/// directory path without escaping it.
pub fn is_plain_vm_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= VM_ID_MAX_BYTES
        && !id.starts_with('.')
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
}

/// Split `host:port`. An IPv6 literal has brackets. The port is required, because the Host CLI
/// resolves any default before it asks.
fn parse_endpoint(value: &str) -> Result<(String, u16), String> {
    let (host, port) = if let Some(rest) = value.strip_prefix('[') {
        let (host, port) = rest
            .split_once("]:")
            .ok_or_else(|| "an IPv6 address needs the form [address]:port".to_string())?;
        if host.parse::<Ipv6Addr>().is_err() {
            return Err(format!("'{host}' is not an IPv6 address"));
        }
        (host, port)
    } else {
        let (host, port) = value
            .rsplit_once(':')
            .ok_or_else(|| "the endpoint needs the form host:port".to_string())?;
        // `::1:80` would read as host `::1` and port 80, and hide the mistake.
        if host.contains(':') {
            return Err("an IPv6 address needs the form [address]:port".to_string());
        }
        if host.parse::<IpAddr>().is_err() && !is_hostname(host) {
            return Err(format!("'{host}' is not a host name or an address"));
        }
        (host, port)
    };
    let port: u16 = port
        .parse()
        .map_err(|_| format!("invalid port '{port}'; expected 1..=65535"))?;
    if port == 0 {
        return Err("port must be 1..=65535".to_string());
    }
    Ok((host.to_string(), port))
}

fn is_hostname(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= 253
        && host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stream(path: &str) -> (StreamTarget, StreamKind) {
        match parse_stream_path(path) {
            RouteMatch::Stream(target, kind) => (target, kind),
            other => panic!("{path} gave {other:?}"),
        }
    }

    #[test]
    fn the_four_routes_parse() {
        assert_eq!(
            stream("/v1/vms/testanyware-ab12cd34/agent"),
            (
                StreamTarget::Vm {
                    id: "testanyware-ab12cd34".into()
                },
                StreamKind::Agent
            )
        );
        assert_eq!(
            stream("/v1/vms/x/vnc"),
            (StreamTarget::Vm { id: "x".into() }, StreamKind::Vnc)
        );
        assert_eq!(
            stream("/v1/endpoints/192.168.64.5:8648/agent"),
            (
                StreamTarget::Endpoint {
                    host: "192.168.64.5".into(),
                    port: 8648
                },
                StreamKind::Agent
            )
        );
        assert_eq!(
            stream("/v1/endpoints/guest.local:5900/vnc"),
            (
                StreamTarget::Endpoint {
                    host: "guest.local".into(),
                    port: 5900
                },
                StreamKind::Vnc
            )
        );
    }

    #[test]
    fn the_builder_and_the_parser_agree() {
        let targets = [
            StreamTarget::Vm {
                id: "testanyware-0a1b2c3d".into(),
            },
            StreamTarget::Endpoint {
                host: "192.168.64.5".into(),
                port: 8648,
            },
            StreamTarget::Endpoint {
                host: "guest-1.example.com".into(),
                port: 1,
            },
            StreamTarget::Endpoint {
                host: "::1".into(),
                port: 5900,
            },
            StreamTarget::Endpoint {
                host: "fe80::1".into(),
                port: 65535,
            },
        ];
        for target in targets {
            for kind in [StreamKind::Agent, StreamKind::Vnc] {
                let path = target.path(kind);
                assert_eq!(stream(&path), (target.clone(), kind), "path {path}");
            }
        }
    }

    #[test]
    fn an_ipv6_literal_encodes_its_brackets() {
        let path = StreamTarget::Endpoint {
            host: "::1".into(),
            port: 80,
        }
        .path(StreamKind::Agent);
        assert_eq!(path, "/v1/endpoints/%5B::1%5D:80/agent");
    }

    #[test]
    fn other_paths_are_no_stream_route() {
        for path in [
            "/v1/status",
            "/v1/vms",
            "/v1/vms/x/stop",
            "/v1/vms/x/agent/extra",
            "/v1/vms/x/screen",
            "/v1/endpoints/h:1/other",
            "/v2/vms/x/agent",
            "/v1/vms//agent/",
            "",
        ] {
            assert_eq!(
                parse_stream_path(path),
                RouteMatch::NotAStreamRoute,
                "{path}"
            );
        }
    }

    #[test]
    fn a_vm_id_that_could_escape_the_directory_is_refused() {
        for id in [
            "..",
            ".hidden",
            "a%2Fb",
            "%2E%2E%2Fevil",
            "a b",
            "é",
            "a:b",
            "",
        ] {
            let path = format!("/v1/vms/{id}/agent");
            let matched = parse_stream_path(&path);
            // An empty id leaves `//` in the path, which is no route at all.
            assert!(
                matches!(
                    matched,
                    RouteMatch::BadVmId(_) | RouteMatch::NotAStreamRoute
                ),
                "{path} gave {matched:?}"
            );
            assert!(!matches!(matched, RouteMatch::Stream(..)), "{path}");
        }
        assert_eq!(
            parse_stream_path("/v1/vms/../agent"),
            RouteMatch::BadVmId("..".into())
        );
        assert!(!is_plain_vm_id(&"a".repeat(VM_ID_MAX_BYTES + 1)));
    }

    #[test]
    fn a_bad_endpoint_names_its_reason() {
        for (target, needle) in [
            ("host", "host:port"),
            ("host:0", "1..=65535"),
            ("host:99999", "invalid port"),
            ("host:abc", "invalid port"),
            (":80", "not a host name"),
            ("a%2Fb:80", "not a host name"),
            ("%2Ftmp%2Fsock:80", "not a host name"),
            ("%5B::1%5D", "[address]:port"),
            ("%5Bnot-ip%5D:80", "not an IPv6"),
        ] {
            let path = format!("/v1/endpoints/{target}/agent");
            match parse_stream_path(&path) {
                RouteMatch::BadEndpoint { reason, .. } => {
                    assert!(reason.contains(needle), "{target}: {reason}")
                }
                other => panic!("{target} gave {other:?}"),
            }
        }
    }

    #[test]
    fn an_unbracketed_ipv6_literal_is_refused() {
        match parse_stream_path("/v1/endpoints/::1:80/agent") {
            RouteMatch::BadEndpoint { reason, .. } => assert!(reason.contains("[address]:port")),
            other => panic!("gave {other:?}"),
        }
    }
}
