//! The TestAnyware app, without its window.
//!
//! The library holds the socket, its safety checks and the protocol. It has no AppKit code.
//! The status item and the login item come with the bundle leaf. The executable in test mode
//! runs this library and nothing else (spec: "The TestAnyware app").

#[cfg(unix)]
pub mod access;
#[cfg(unix)]
pub mod gate;
#[cfg(unix)]
pub mod listener;
#[cfg(unix)]
pub mod server;

#[cfg(unix)]
pub use server::{serve, AppContext};
