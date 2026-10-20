//! What the TestAnyware app and the Host CLI agree on.
//!
//! The crate holds types and pure rules only. It does no I/O except the one
//! directory check in [`socket`], and it builds on every host (ADR-0004,
//! ADR-0019). The Host CLI uses it on Linux and Windows hosts too.

pub mod error;
pub mod hello;
pub mod socket;
pub mod status;
pub mod stream;
pub mod version;

pub use error::{AppError, Peer, UnreachableCause, APP_INCOMPATIBLE, APP_UNREACHABLE};
pub use hello::{ClientHello, Directories};
pub use socket::{
    default_socket_path, judge_socket_dir, validate_socket_path, SocketDirError, SocketPathError,
    SOCKET_PATH_MAX_BYTES,
};
pub use status::StatusResponse;
pub use stream::{
    is_plain_vm_id, parse_stream_path, RouteMatch, StreamKind, StreamTarget, STREAM_UPGRADE_TOKEN,
};
pub use version::ProtocolRange;
