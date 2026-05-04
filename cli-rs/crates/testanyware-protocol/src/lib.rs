//! Wire-format types for the TestAnyware in-VM agent.
//!
//! This crate is the Rust counterpart of `TestAnywareAgentProtocol` (Swift,
//! `cli/Sources/TestAnywareAgentProtocol/`). It carries no I/O, no HTTP, no
//! filesystem access — only serde definitions and pure logic such as the
//! macOS AX role mapper.
//!
//! ## Wire format invariants
//!
//! These properties are enforced by the cross-language contract fixtures
//! (`cli-rs/tests/fixtures/protocol/`):
//!
//! - All keys are `camelCase`.
//! - `CGPoint`/`CGSize`/`CGRect` are flattened into per-axis keys —
//!   `positionX`, `positionY`, `sizeWidth`, `sizeHeight`, `boundsX`,
//!   `boundsY`, `boundsWidth`, `boundsHeight` — not nested objects.
//! - Optional fields are omitted from the encoded form when `None`.
//! - `UnifiedRole` is a string enum with kebab-case raw values for
//!   multi-word variants (e.g. `menu-item`, `combo-box`).

pub mod agent_formatter;
pub mod agent_requests;
pub mod agent_responses;
pub mod element_info;
pub mod role_mapper;
pub mod unified_role;
pub mod window_info;

pub use agent_formatter::AgentFormatter;
pub use agent_requests::{
    DownloadRequest, DownloadResponse, ElementQuery, ExecRequest, ExecResult, HealthResponse,
    SnapshotRequest, UploadRequest,
};
pub use agent_responses::{ActionResponse, ErrorResponse, InspectResponse, SnapshotResponse};
pub use element_info::ElementInfo;
pub use role_mapper::RoleMapper;
pub use unified_role::UnifiedRole;
pub use window_info::WindowInfo;
