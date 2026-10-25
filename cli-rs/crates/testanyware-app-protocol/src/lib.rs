//! What the TestAnyware app and the Host CLI agree on.
//!
//! The crate holds types and pure rules only. It does no I/O except the one
//! directory check in [`socket`], and it builds on every host (ADR-0004,
//! ADR-0019). The Host CLI uses it on Linux and Windows hosts too.

pub mod bundle;
pub mod error;
pub mod hello;
pub mod list;
pub mod skew;
pub mod slots;
pub mod socket;
pub mod start;
pub mod status;
pub mod stop_delete;
pub mod stream;
pub mod version;

pub use bundle::{bundle_build_at, bundle_build_from_plist};
pub use error::{
    AppError, Peer, UnreachableCause, APP_INCOMPATIBLE, APP_NOT_INSTALLED, APP_UNREACHABLE,
    CONFIG_INVALID, LOCAL_NETWORK_REMEDY, SETUP_REMEDY,
};
pub use hello::{ClientHello, Directories, ServerHello};
pub use list::{ListRequest, LIST_PATH};
pub use skew::{decide, needs_version, older_than_bundle, Decision, Notice, Remedy};
pub use slots::{
    HolderKind, HolderState, RequestedBy, SlotContention, SlotHolder, SlotWaiter, SlotsView,
    WaiterKind, SLOTS_PATH,
};
pub use socket::{
    default_socket_path, judge_socket_dir, validate_socket_path, SocketDirError, SocketPathError,
    SOCKET_PATH_MAX_BYTES,
};
pub use start::{
    EventLines, HostPort, StartEvent, StartRequest, StartResult, TpmReport, START_CONTENT_TYPE,
};
pub use status::{
    AdoptedVm, AdoptionStatus, ConfigurationState, ConfigurationStatus, GrantState,
    LocalNetworkStatus, LoginItemState, NotAdoptedVm, ProbeAnswer, StatusResponse,
};
pub use stop_delete::{
    delete_path, parse_end_path, stop_path, DeleteRequest, DeleteResult, EndRoute, StopResult,
    GOLDEN_NAME_MAX_BYTES,
};
pub use stream::{
    is_plain_vm_id, parse_stream_path, RouteMatch, StreamKind, StreamTarget, STREAM_UPGRADE_TOKEN,
};
pub use version::{ProtocolRange, SLOTS_VERSION};
