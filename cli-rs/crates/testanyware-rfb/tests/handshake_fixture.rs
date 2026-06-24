//! Drive the handshake state machine with a synthetic in-memory
//! transport. Exercises the byte patterns documented in RFC 6143
//! without binding to a TCP port.

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

use testanyware_rfb::{proto::PixelFormat, RfbConnection, ServerEvent};

/// Bidirectional in-memory transport: a "server-side" script feeds the
/// client's reads, and the client's writes are captured into a buffer
/// the test can inspect.
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

    fn client_writes(&self) -> &[u8] {
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
            // No more bytes scripted; signal EOF.
            return Poll::Ready(Ok(()));
        }
        let remaining = &inner[pos..];
        let n = remaining.len().min(buf.remaining());
        buf.put_slice(&remaining[..n]);
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

/// Build a canonical "no-auth" server script ending after ServerInit.
/// `width` and `height` are placed in the ServerInit header.
fn server_no_auth_script(width: u16, height: u16, name: &[u8]) -> Vec<u8> {
    let mut script = Vec::new();
    // 1. Protocol version greeting.
    script.extend_from_slice(b"RFB 003.008\n");
    // 2. Security types: count=1, type=1 (None).
    script.push(1);
    script.push(1);
    // 3. SecurityResult OK.
    script.extend_from_slice(&0u32.to_be_bytes());
    // 4. ServerInit.
    script.extend_from_slice(&width.to_be_bytes());
    script.extend_from_slice(&height.to_be_bytes());
    script.extend_from_slice(&PixelFormat::rgba32_le().encode());
    script.extend_from_slice(&(name.len() as u32).to_be_bytes());
    script.extend_from_slice(name);
    script
}

#[tokio::test]
async fn handshake_completes_with_no_auth() {
    let script = server_no_auth_script(1920, 1080, b"testanyware-vm");
    let transport = ScriptedTransport::new(script);
    let conn = RfbConnection::handshake(transport, None)
        .await
        .expect("handshake should succeed");
    assert_eq!(conn.framebuffer_size(), (1920, 1080));
}

#[tokio::test]
async fn client_writes_protocol_version_then_chosen_security_then_setpf_setencodings() {
    let script = server_no_auth_script(800, 600, b"vm");
    let mut transport = ScriptedTransport::new(script);
    // Drive the handshake and discard the resulting connection (we
    // care about what the client sent, not what it parsed).
    let _ = RfbConnection::handshake(&mut transport, None).await.unwrap();

    let writes = transport.client_writes();

    // First 12 bytes: protocol version mirror.
    assert_eq!(&writes[0..12], b"RFB 003.008\n");

    // Next 1 byte: chosen security type (None = 1).
    assert_eq!(writes[12], 1);

    // Next 1 byte: ClientInit shared flag (we always request shared).
    assert_eq!(writes[13], 1);

    // Next 20 bytes: SetPixelFormat (1 type + 3 padding + 16 pf).
    assert_eq!(writes[14], 0, "SetPixelFormat tag");
    assert_eq!(&writes[18..34], &PixelFormat::rgba32_le().encode());

    // Next: SetEncodings (1 tag + 1 pad + 2 count + N*4).
    assert_eq!(writes[34], 2, "SetEncodings tag");
    let n = u16::from_be_bytes([writes[36], writes[37]]);
    assert_eq!(n, 6, "ZRLE + Tight + CopyRect + Raw + DesktopSize + LastRect");
    // ZRLE (16) must be advertised first (lossless, preferred for OCR),
    // with Tight (7) second.
    let first = i32::from_be_bytes([writes[38], writes[39], writes[40], writes[41]]);
    assert_eq!(first, 16, "ZRLE advertised first");
    let second = i32::from_be_bytes([writes[42], writes[43], writes[44], writes[45]]);
    assert_eq!(second, 7, "Tight advertised second");
}

#[tokio::test]
async fn handshake_rejects_unexpected_protocol_version() {
    // Server sends 3.7 instead of 3.8.
    let mut script = b"RFB 003.007\n".to_vec();
    script.push(1);
    script.push(1);
    script.extend_from_slice(&0u32.to_be_bytes());
    let transport = ScriptedTransport::new(script);
    let err = RfbConnection::handshake(transport, None)
        .await
        .expect_err("3.7 should be rejected");
    assert!(matches!(
        err,
        testanyware_rfb::RfbError::UnsupportedProtocolVersion(_)
    ));
}

#[tokio::test]
async fn handshake_propagates_security_negotiation_failure() {
    let mut script = b"RFB 003.008\n".to_vec();
    // count = 0 → followed by 4-byte length-prefixed reason.
    script.push(0);
    let reason = b"too many users";
    script.extend_from_slice(&(reason.len() as u32).to_be_bytes());
    script.extend_from_slice(reason);
    let transport = ScriptedTransport::new(script);
    let err = RfbConnection::handshake(transport, None)
        .await
        .expect_err("count=0 must surface as a negotiation failure");
    match err {
        testanyware_rfb::RfbError::SecurityNegotiationFailed(s) => {
            assert!(s.contains("too many users"));
        }
        other => panic!("expected SecurityNegotiationFailed, got {other:?}"),
    }
}

#[tokio::test]
async fn full_update_decoded_into_framebuffer() {
    // 2x1 framebuffer, no auth, then a single update with one Raw rect
    // covering both pixels: pixel0 = red, pixel1 = green.
    let mut script = server_no_auth_script(2, 1, b"vm");
    // FramebufferUpdate: tag 0, pad, num_rects=1, rect at (0,0,2,1)
    // encoded as Raw.
    script.push(0); // tag
    script.push(0); // pad
    script.extend_from_slice(&1u16.to_be_bytes()); // num_rects
    script.extend_from_slice(&0u16.to_be_bytes()); // x
    script.extend_from_slice(&0u16.to_be_bytes()); // y
    script.extend_from_slice(&2u16.to_be_bytes()); // w
    script.extend_from_slice(&1u16.to_be_bytes()); // h
    script.extend_from_slice(&0i32.to_be_bytes()); // encoding = Raw
    // Two BGRX pixels: red, green.
    script.extend_from_slice(&[0, 0, 255, 0, 0, 255, 0, 0]);

    let transport = ScriptedTransport::new(script);
    let mut conn = RfbConnection::handshake(transport, None).await.unwrap();
    let event = conn.next_message().await.unwrap();
    match event {
        ServerEvent::FramebufferUpdated { rectangles } => assert_eq!(rectangles, 1),
        other => panic!("expected FramebufferUpdated, got {other:?}"),
    }
    let fb = conn.framebuffer();
    let rgba = fb.rgba();
    assert_eq!(&rgba[0..4], &[255, 0, 0, 0xFF], "pixel 0 RGBA = red");
    assert_eq!(&rgba[4..8], &[0, 255, 0, 0xFF], "pixel 1 RGBA = green");
}

#[tokio::test]
async fn tight_fill_rect_decoded_through_connection_path() {
    // Exercise the Tight arm end-to-end: a 2x2 framebuffer filled by a
    // single Tight "fill" rectangle (control 0x80 + one TPIXEL), proving
    // the connection reads Tight's variable-length payload straight off
    // the transport and lands it in the framebuffer.
    let mut script = server_no_auth_script(2, 2, b"vm");
    script.push(0); // FramebufferUpdate tag
    script.push(0); // pad
    script.extend_from_slice(&1u16.to_be_bytes()); // num_rects
    script.extend_from_slice(&0u16.to_be_bytes()); // x
    script.extend_from_slice(&0u16.to_be_bytes()); // y
    script.extend_from_slice(&2u16.to_be_bytes()); // w
    script.extend_from_slice(&2u16.to_be_bytes()); // h
    script.extend_from_slice(&7i32.to_be_bytes()); // encoding = Tight
    script.push(0x80); // control byte: fill, no resets
    // TPIXEL for our rgba32_le format: 3 significant LE bytes [B, G, R].
    // Blue colour (R=0, G=0, B=255) => [255, 0, 0].
    script.extend_from_slice(&[255, 0, 0]);

    let transport = ScriptedTransport::new(script);
    let mut conn = RfbConnection::handshake(transport, None).await.unwrap();
    let event = conn.next_message().await.unwrap();
    match event {
        ServerEvent::FramebufferUpdated { rectangles } => assert_eq!(rectangles, 1),
        other => panic!("expected FramebufferUpdated, got {other:?}"),
    }
    // All four pixels are blue: RGBA = [0, 0, 255, 0xFF].
    let fb = conn.framebuffer();
    for px in fb.rgba().chunks_exact(4) {
        assert_eq!(px, &[0, 0, 255, 0xFF]);
    }
}

/// Build a no-auth script ending with a single full-frame Raw update of the
/// given `width × height`, where `bgrx` supplies the row-major BGRX pixels.
fn server_script_with_full_raw(width: u16, height: u16, bgrx: &[u8]) -> Vec<u8> {
    let mut script = server_no_auth_script(width, height, b"vm");
    script.push(0); // FramebufferUpdate tag
    script.push(0); // pad
    script.extend_from_slice(&1u16.to_be_bytes()); // num_rects
    script.extend_from_slice(&0u16.to_be_bytes()); // x
    script.extend_from_slice(&0u16.to_be_bytes()); // y
    script.extend_from_slice(&width.to_be_bytes()); // w
    script.extend_from_slice(&height.to_be_bytes()); // h
    script.extend_from_slice(&0i32.to_be_bytes()); // encoding = Raw
    script.extend_from_slice(bgrx);
    script
}

#[tokio::test]
async fn logical_target_downsamples_framebuffer_read_and_size() {
    // Physical 2x2 (R = 10,20,30,40) with a logical 1x1 target → scale 2;
    // the single logical pixel box-averages to (100 + 2) / 4 = 25, while the
    // physical accessors still expose the raw 2x2 Retina frame (the
    // `--physical` capture/record path, ADR-0016 D2b).
    let bgrx = [
        0, 0, 10, 0, // (0,0)
        0, 0, 20, 0, // (1,0)
        0, 0, 30, 0, // (0,1)
        0, 0, 40, 0, // (1,1)
    ];
    let transport = ScriptedTransport::new(server_script_with_full_raw(2, 2, &bgrx));
    let mut conn = RfbConnection::handshake(transport, None).await.unwrap();
    conn.next_message().await.unwrap();
    conn.set_logical_target(1, 1);

    assert_eq!(conn.scale(), 2);
    assert_eq!(conn.framebuffer_size(), (1, 1), "logical size");
    assert_eq!(conn.framebuffer().rgba(), &[25, 0, 0, 0xFF], "logical pixel");

    assert_eq!(conn.physical_framebuffer_size(), (2, 2), "physical size");
    assert_eq!(
        conn.physical_framebuffer().rgba().len(),
        2 * 2 * 4,
        "physical frame untouched"
    );
}

#[tokio::test]
async fn no_logical_target_is_byte_identical_passthrough() {
    // The default (non-HiDPI) path: no target → scale 1 → the logical view is
    // the physical frame, verbatim. This is the regression guard.
    let bgrx = [0, 0, 255, 0, 0, 255, 0, 0]; // red, green
    let transport = ScriptedTransport::new(server_script_with_full_raw(2, 1, &bgrx));
    let mut conn = RfbConnection::handshake(transport, None).await.unwrap();
    conn.next_message().await.unwrap();

    assert_eq!(conn.scale(), 1);
    assert_eq!(conn.framebuffer_size(), (2, 1));
    assert_eq!(conn.framebuffer().rgba(), conn.physical_framebuffer().rgba());
}

#[tokio::test]
async fn logical_target_no_ops_when_host_yields_1x() {
    // HiDPI requested (target set) but the host produced a 1× framebuffer:
    // physical == logical → scale 1 → graceful no-op (ADR-0016 consequence:
    // the opt-in must never be silently wrong on a 1× host).
    let transport = ScriptedTransport::new(server_no_auth_script(2, 2, b"vm"));
    let mut conn = RfbConnection::handshake(transport, None).await.unwrap();
    conn.set_logical_target(2, 2);

    assert_eq!(conn.scale(), 1, "requested but host gave 1×");
    assert_eq!(conn.framebuffer_size(), (2, 2));
}
