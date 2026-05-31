use std::io;

use thiserror::Error;

/// Errors produced by the RFB client.
#[derive(Debug, Error)]
pub enum RfbError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("server speaks an unsupported protocol version: {0:?}")]
    UnsupportedProtocolVersion([u8; 12]),

    #[error("server offered no security types; reason: {0}")]
    SecurityNegotiationFailed(String),

    #[error("server does not offer a security type we can satisfy (offered: {0:?})")]
    NoMutualSecurityType(Vec<u8>),

    #[error("VNC password authentication failed (server returned status {0})")]
    AuthFailed(u32),

    #[error("password required for VNC authentication, but none was supplied")]
    PasswordRequired,

    #[error("framebuffer dimensions are invalid: {width}x{height}")]
    InvalidFramebufferSize { width: u32, height: u32 },

    #[error("server sent unexpected message type {0}")]
    UnexpectedMessageType(u8),

    #[error("server used an encoding we have not negotiated: {0}")]
    UnsupportedEncoding(i32),

    #[error("protocol violation: {0}")]
    Protocol(String),

    #[error(
        "invalid encoding override {value:?} in TESTANYWARE_RFB_ENCODING; \
         expected one of: zrle, tight, raw"
    )]
    InvalidEncodingOverride { value: String },
}
