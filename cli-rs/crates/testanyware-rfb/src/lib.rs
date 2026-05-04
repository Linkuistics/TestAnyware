//! `testanyware-rfb` — pure-Rust RFB (VNC) protocol client.
//!
//! Implements RFB 3.8 (RFC 6143) at a level sufficient for screen
//! capture and screen-size queries against the project's golden VMs:
//!
//! - protocol-version greeting,
//! - security types `None` and `VNC Authentication` (DES with the
//!   RFB-quirky bit-reversed key),
//! - 32 bpp little-endian RGBA-32 pixel format negotiation,
//! - `Raw` and `CopyRect` encodings,
//! - `DesktopSize` and `LastRect` pseudo-encodings.
//!
//! ZRLE / Tight encodings are out of scope for this foundation. They
//! land in a follow-up task; most golden VMs negotiate down to Raw if
//! Tight is not offered, so the foundation is sufficient to drive
//! `screen size`, `screen capture`, and (later) the embedded viewer
//! and the input-message extension.

pub mod auth;
pub mod connection;
pub mod error;
pub mod framebuffer;
pub mod input;
pub mod keymap;
pub mod proto;

pub use connection::{RfbConnection, ServerEvent};
pub use error::RfbError;
pub use framebuffer::Framebuffer;
pub use input::InputError;
pub use keymap::{KeymapError, Platform};
pub use proto::PixelFormat;
