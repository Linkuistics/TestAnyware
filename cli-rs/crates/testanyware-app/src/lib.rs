//! The TestAnyware app, without its window.
//!
//! The library holds the socket, its safety checks, the protocol, the guest streams and the VM listing. The server
//! code has no AppKit code. Only the `shell` module, on macOS, touches AppKit: it shows the status
//! item and the login switch that the `menu` module describes (spec: "The TestAnyware app").
//! The executable in test mode runs the server and nothing else.

#[cfg(unix)]
pub mod access;
#[cfg(unix)]
pub mod dial;
#[cfg(unix)]
pub mod gate;
#[cfg(unix)]
pub mod grant;
#[cfg(unix)]
pub mod listener;
#[cfg(unix)]
pub mod menu;
#[cfg(unix)]
pub mod server;
#[cfg(target_os = "macos")]
pub mod shell;
#[cfg(unix)]
pub mod streams;
#[cfg(unix)]
pub mod vms;

#[cfg(unix)]
pub use server::{serve, AppContext};
