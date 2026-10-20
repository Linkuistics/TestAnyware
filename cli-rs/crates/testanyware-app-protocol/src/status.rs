//! The body of `GET /v1/status`.

use serde::{Deserialize, Serialize};

use crate::hello::Directories;
use crate::version::ProtocolRange;

/// The app's state. The fields `build`, `protocol_range` and `directories` keep their
/// names in every protocol version, because `doctor` reads them to report a mismatch.
/// Later leaves add fields. A reader must ignore the fields it does not know.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusResponse {
    pub build: String,
    pub protocol_range: ProtocolRange,
    pub directories: Directories,
}
