//! `testanyware-rfb` — pure-Rust RFB (VNC) protocol client.
//!
//! Implements RFB 3.8 (RFC 6143) at a level sufficient for screen
//! capture and screen-size queries against the project's golden VMs:
//!
//! - protocol-version greeting,
//! - security types `None` and `VNC Authentication` (DES with the
//!   RFB-quirky bit-reversed key),
//! - 32 bpp little-endian RGBA-32 pixel format negotiation,
//! - `Raw`, `CopyRect`, `ZRLE` and `Tight` encodings,
//! - `DesktopSize` and `LastRect` pseudo-encodings.
//!
//! Servers that offer no compressed encoding negotiate down to Raw, so
//! the client drives `screen size`, `screen capture`, the embedded
//! viewer and the input-message extension across all of them.

pub mod auth;
pub mod connection;
pub mod error;
pub mod framebuffer;
pub mod input;
pub mod keymap;
pub mod proto;
pub mod tight;
pub mod zrle;

pub use connection::{RfbConnection, ServerEvent};
pub use error::RfbError;
pub use framebuffer::Framebuffer;
pub use input::InputError;
pub use keymap::{KeymapError, Platform};
pub use proto::PixelFormat;
