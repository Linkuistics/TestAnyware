//! Wire-format tests for KeyEvent / PointerEvent and the high-level
//! input helpers.
//!
//! The transport captures everything the client writes, then we slice
//! past the handshake bytes (12 + 1 + 1 + 20 + 24 = 58) to inspect the
//! input messages alone.

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

use testanyware_rfb::{proto::PixelFormat, Platform, RfbConnection};

#[derive(Debug)]
struct ScriptedTransport {
    server_to_client: io::Cursor<Vec<u8>>,
    client_to_server: Vec<u8>,
}

impl ScriptedTransport {
    fn new(server_script: Vec<u8>) -> Self {
        Self {
            server_to_client: io::Cursor::new(server_script),
            client_to_server: Vec::new(),
        }
    }
    fn writes(&self) -> &[u8] {
        &self.client_to_server
    }
}

impl AsyncRead for ScriptedTransport {
    fn poll_read(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let pos = self.server_to_client.position() as usize;
        let inner = self.server_to_client.get_ref();
        if pos >= inner.len() {
            return Poll::Ready(Ok(()));
        }
        let n = (inner.len() - pos).min(buf.remaining());
        buf.put_slice(&inner[pos..pos + n]);
        self.server_to_client.set_position((pos + n) as u64);
        Poll::Ready(Ok(()))
    }
}

impl AsyncWrite for ScriptedTransport {
    fn poll_write(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.client_to_server.extend_from_slice(buf);
        Poll::Ready(Ok(buf.len()))
    }
    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

fn server_no_auth_script(width: u16, height: u16, name: &[u8]) -> Vec<u8> {
    let mut script = Vec::new();
    script.extend_from_slice(b"RFB 003.008\n");
    script.push(1);
    script.push(1);
    script.extend_from_slice(&0u32.to_be_bytes());
    script.extend_from_slice(&width.to_be_bytes());
    script.extend_from_slice(&height.to_be_bytes());
    script.extend_from_slice(&PixelFormat::rgba32_le().encode());
    script.extend_from_slice(&(name.len() as u32).to_be_bytes());
    script.extend_from_slice(name);
    script
}

/// Number of bytes the client writes during the no-auth handshake:
/// 12 (proto greeting), 1 (chosen security type), 1 (ClientInit shared),
/// 20 (SetPixelFormat), and SetEncodings (4-byte header plus 5 codes of
/// 4 bytes each = 24). The 5 codes are ZRLE, CopyRect, Raw, DesktopSize,
/// LastRect.
///
/// Verify by reading the actual code in `connection.rs`.
const HANDSHAKE_WRITE_LEN: usize = 12 + 1 + 1 + 20 + 4 + 5 * 4;

#[tokio::test]
async fn key_event_press_then_release_lays_out_bytes() {
    let script = server_no_auth_script(800, 600, b"vm");
    let mut transport = ScriptedTransport::new(script);
    let mut conn = RfbConnection::handshake(&mut transport, None).await.unwrap();
    conn.key_event(0xff0d, true).await.unwrap(); // Return down
    conn.key_event(0xff0d, false).await.unwrap(); // Return up

    let after_handshake = &transport.writes()[HANDSHAKE_WRITE_LEN..];
    assert_eq!(after_handshake.len(), 16, "two KeyEvents = 16 bytes");

    // First message: type 4, down=1, pad pad, keysym 0xff0d.
    assert_eq!(after_handshake[0], 4);
    assert_eq!(after_handshake[1], 1);
    assert_eq!(&after_handshake[4..8], &0xff0du32.to_be_bytes());
    // Second message: type 4, down=0, pad pad, keysym 0xff0d.
    assert_eq!(after_handshake[8], 4);
    assert_eq!(after_handshake[9], 0);
    assert_eq!(&after_handshake[12..16], &0xff0du32.to_be_bytes());
}

#[tokio::test]
async fn pointer_event_layout_matches_rfc_6143() {
    let script = server_no_auth_script(800, 600, b"vm");
    let mut transport = ScriptedTransport::new(script);
    let mut conn = RfbConnection::handshake(&mut transport, None).await.unwrap();
    conn.pointer_event(0b001, 100, 200).await.unwrap();

    let msg = &transport.writes()[HANDSHAKE_WRITE_LEN..];
    assert_eq!(msg.len(), 6);
    assert_eq!(msg[0], 5, "PointerEvent tag");
    assert_eq!(msg[1], 0b001, "button mask = left");
    assert_eq!(&msg[2..4], &100u16.to_be_bytes());
    assert_eq!(&msg[4..6], &200u16.to_be_bytes());
}

#[tokio::test]
async fn click_emits_down_then_up_zero_mask() {
    let script = server_no_auth_script(800, 600, b"vm");
    let mut transport = ScriptedTransport::new(script);
    let mut conn = RfbConnection::handshake(&mut transport, None).await.unwrap();
    conn.click(50, 60, "left", 1).await.unwrap();

    let msg = &transport.writes()[HANDSHAKE_WRITE_LEN..];
    assert_eq!(msg.len(), 12);
    assert_eq!(msg[0], 5);
    assert_eq!(msg[1], 0b001, "down: left");
    assert_eq!(msg[6], 5);
    assert_eq!(msg[7], 0, "up: no buttons held");
}

#[tokio::test]
async fn click_count_two_emits_four_pointer_events() {
    let script = server_no_auth_script(800, 600, b"vm");
    let mut transport = ScriptedTransport::new(script);
    let mut conn = RfbConnection::handshake(&mut transport, None).await.unwrap();
    conn.click(0, 0, "right", 2).await.unwrap();

    let msg = &transport.writes()[HANDSHAKE_WRITE_LEN..];
    // 4 PointerEvents * 6 bytes each.
    assert_eq!(msg.len(), 24);
    // Each "down" should have right-button bit set (mask 0b100 = 4).
    assert_eq!(msg[1], 0b100);
    assert_eq!(msg[7], 0);
    assert_eq!(msg[13], 0b100);
    assert_eq!(msg[19], 0);
}

#[tokio::test]
async fn type_text_uppercase_brackets_with_shift() {
    let script = server_no_auth_script(800, 600, b"vm");
    let mut transport = ScriptedTransport::new(script);
    let mut conn = RfbConnection::handshake(&mut transport, None).await.unwrap();
    conn.type_text("A").await.unwrap();

    let msg = &transport.writes()[HANDSHAKE_WRITE_LEN..];
    // Expected sequence:
    //   shift down (8) + 'a' down (8) + 'a' up (8) + shift up (8) = 32 bytes
    assert_eq!(msg.len(), 32);
    // shift down
    assert_eq!(msg[0], 4);
    assert_eq!(msg[1], 1);
    assert_eq!(&msg[4..8], &0xffe1u32.to_be_bytes());
    // 'a' down
    assert_eq!(msg[8], 4);
    assert_eq!(msg[9], 1);
    assert_eq!(&msg[12..16], &(b'a' as u32).to_be_bytes());
    // 'a' up
    assert_eq!(msg[16], 4);
    assert_eq!(msg[17], 0);
    // shift up
    assert_eq!(msg[24], 4);
    assert_eq!(msg[25], 0);
    assert_eq!(&msg[28..32], &0xffe1u32.to_be_bytes());
}

#[tokio::test]
async fn type_text_lowercase_letter_no_shift() {
    let script = server_no_auth_script(800, 600, b"vm");
    let mut transport = ScriptedTransport::new(script);
    let mut conn = RfbConnection::handshake(&mut transport, None).await.unwrap();
    conn.type_text("a").await.unwrap();

    let msg = &transport.writes()[HANDSHAKE_WRITE_LEN..];
    assert_eq!(msg.len(), 16, "down + up only");
    assert_eq!(&msg[4..8], &(b'a' as u32).to_be_bytes());
    assert_eq!(&msg[12..16], &(b'a' as u32).to_be_bytes());
}

#[tokio::test]
async fn type_text_shifted_symbol_uses_base_with_shift() {
    let script = server_no_auth_script(800, 600, b"vm");
    let mut transport = ScriptedTransport::new(script);
    let mut conn = RfbConnection::handshake(&mut transport, None).await.unwrap();
    conn.type_text("!").await.unwrap();

    let msg = &transport.writes()[HANDSHAKE_WRITE_LEN..];
    // shift down + '1' down + '1' up + shift up = 4 events = 32 bytes
    assert_eq!(msg.len(), 32);
    assert_eq!(&msg[4..8], &0xffe1u32.to_be_bytes(), "shift down");
    assert_eq!(&msg[12..16], &(b'1' as u32).to_be_bytes(), "base char '1'");
}

#[tokio::test]
async fn press_key_with_macos_cmd_modifier_uses_xk_alt_l() {
    let script = server_no_auth_script(800, 600, b"vm");
    let mut transport = ScriptedTransport::new(script);
    let mut conn = RfbConnection::handshake(&mut transport, None).await.unwrap();
    conn.press_key("a", &["cmd"], Platform::Macos).await.unwrap();

    let msg = &transport.writes()[HANDSHAKE_WRITE_LEN..];
    // cmd down (8) + 'a' down (8) + 'a' up (8) + cmd up (8) = 32 bytes
    assert_eq!(msg.len(), 32);
    // First event: modifier down — must be XK_Alt_L (0xffe9) per memory.
    assert_eq!(msg[0], 4);
    assert_eq!(msg[1], 1);
    assert_eq!(&msg[4..8], &0xffe9u32.to_be_bytes());
    // Last event: modifier up — same keysym.
    assert_eq!(msg[24], 4);
    assert_eq!(msg[25], 0);
    assert_eq!(&msg[28..32], &0xffe9u32.to_be_bytes());
}

#[tokio::test]
async fn press_key_unknown_returns_input_error() {
    let script = server_no_auth_script(800, 600, b"vm");
    let transport = ScriptedTransport::new(script);
    let mut conn = RfbConnection::handshake(transport, None).await.unwrap();
    let err = conn.press_key("ImaginaryKey", &[], Platform::Linux).await;
    assert!(err.is_err());
}

#[tokio::test]
async fn drag_emits_down_then_n_moves_then_up() {
    let script = server_no_auth_script(800, 600, b"vm");
    let mut transport = ScriptedTransport::new(script);
    let mut conn = RfbConnection::handshake(&mut transport, None).await.unwrap();
    conn.drag(0, 0, 100, 100, "left", 4).await.unwrap();

    let msg = &transport.writes()[HANDSHAKE_WRITE_LEN..];
    // 1 down + 4 moves + 1 up = 6 events = 36 bytes
    assert_eq!(msg.len(), 36);
    // First event: button down at (0,0) with mask 0b001.
    assert_eq!(msg[1], 0b001);
    assert_eq!(&msg[2..4], &0u16.to_be_bytes());
    // Last event: button up at (100,100) with mask 0.
    assert_eq!(msg[31], 0);
    assert_eq!(&msg[32..34], &100u16.to_be_bytes());
    assert_eq!(&msg[34..36], &100u16.to_be_bytes());
}

#[tokio::test]
async fn scroll_dy_negative_emits_wheel_up_pulses() {
    let script = server_no_auth_script(800, 600, b"vm");
    let mut transport = ScriptedTransport::new(script);
    let mut conn = RfbConnection::handshake(&mut transport, None).await.unwrap();
    conn.scroll(50, 60, 0, -2).await.unwrap();

    let msg = &transport.writes()[HANDSHAKE_WRITE_LEN..];
    // 2 pulses, each = down + up = 4 PointerEvents = 24 bytes.
    assert_eq!(msg.len(), 24);
    // Wheel up = bit 3 → mask 0b1000 = 8.
    assert_eq!(msg[1], 0b1000);
    assert_eq!(msg[7], 0);
    assert_eq!(msg[13], 0b1000);
    assert_eq!(msg[19], 0);
}
