//! High-level RFB connection: handshake + framebuffer-update loop.
//!
//! Concrete TCP plumbing lives behind a generic `AsyncRead + AsyncWrite`
//! bound so test fixtures can pump synthetic byte streams through the
//! state machine without binding to a port.

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

use crate::auth::vnc_authenticate;
use crate::error::RfbError;
use crate::framebuffer::Framebuffer;
use crate::proto::{
    client_msg, encoding, sec_type, server_msg, PixelFormat, PROTOCOL_VERSION_3_8,
};

/// Outcome of `next_message`.
#[derive(Debug)]
pub enum ServerEvent {
    /// One framebuffer update was received and applied to the
    /// internal framebuffer. The contained count is the number of
    /// rectangles processed (zero for a no-op update).
    FramebufferUpdated { rectangles: u32 },
    /// Server rang the bell.
    Bell,
    /// Server sent a colour-map update (we never request indexed
    /// colour, so this is unexpected; reported and ignored).
    ColourMapEntries,
    /// Server sent us its clipboard contents. Payload discarded.
    ServerCutText,
}

/// Owned RFB connection. Generic over the underlying transport so
/// tests can drive the state machine without a TCP socket.
#[derive(Debug)]
pub struct RfbConnection<T: AsyncRead + AsyncWrite + Unpin> {
    transport: T,
    framebuffer: Framebuffer,
    pixel_format: PixelFormat,
}

impl RfbConnection<BufReader<TcpStream>> {
    /// Connect over TCP, complete the RFB 3.8 handshake (with VNC
    /// password auth if provided), negotiate our preferred pixel
    /// format and the Raw + CopyRect encodings, and return a connection
    /// ready to receive framebuffer updates.
    pub async fn connect(
        host: &str,
        port: u16,
        password: Option<&[u8]>,
    ) -> Result<Self, RfbError> {
        let stream = TcpStream::connect((host, port)).await?;
        // Disable Nagle so our small handshake messages flush promptly.
        stream.set_nodelay(true).ok();
        let transport = BufReader::new(stream);
        Self::handshake(transport, password).await
    }
}

impl<T: AsyncRead + AsyncWrite + Unpin> RfbConnection<T> {
    /// Handshake driver, exposed for tests that supply a synthetic
    /// transport.
    pub async fn handshake(mut transport: T, password: Option<&[u8]>) -> Result<Self, RfbError> {
        // 1. Protocol version.
        let mut version = [0u8; 12];
        transport.read_exact(&mut version).await?;
        if &version != PROTOCOL_VERSION_3_8 {
            // Servers older than 3.8 are rare on the platforms we
            // target; reject loudly rather than silently downgrade.
            return Err(RfbError::UnsupportedProtocolVersion(version));
        }
        transport.write_all(PROTOCOL_VERSION_3_8).await?;
        transport.flush().await?;

        // 2. Security types (3.8 framing).
        let mut count_buf = [0u8; 1];
        transport.read_exact(&mut count_buf).await?;
        let count = count_buf[0];
        if count == 0 {
            // Server rejected the connection; the next field is a
            // 4-byte length-prefixed reason string.
            let mut len_buf = [0u8; 4];
            transport.read_exact(&mut len_buf).await?;
            let len = u32::from_be_bytes(len_buf) as usize;
            let mut reason = vec![0u8; len];
            transport.read_exact(&mut reason).await?;
            return Err(RfbError::SecurityNegotiationFailed(
                String::from_utf8_lossy(&reason).into_owned(),
            ));
        }
        let mut offered = vec![0u8; count as usize];
        transport.read_exact(&mut offered).await?;
        let chosen = pick_security_type(&offered, password.is_some())?;
        transport.write_all(&[chosen]).await?;
        transport.flush().await?;

        // 3. Optional VNC auth round-trip.
        if chosen == sec_type::VNC_AUTH {
            let pw = password.ok_or(RfbError::PasswordRequired)?;
            let mut challenge = [0u8; 16];
            transport.read_exact(&mut challenge).await?;
            let response = vnc_authenticate(pw, &challenge);
            transport.write_all(&response).await?;
            transport.flush().await?;
        }

        // 4. SecurityResult (always present in 3.8, regardless of type).
        let mut result_buf = [0u8; 4];
        transport.read_exact(&mut result_buf).await?;
        let result = u32::from_be_bytes(result_buf);
        if result != 0 {
            // RFB 3.8 also has a length-prefixed reason after a
            // failure, but consuming it requires reading further; we
            // surface the status and let the caller close.
            return Err(RfbError::AuthFailed(result));
        }

        // 5. ClientInit — request shared session.
        transport.write_all(&[1]).await?;
        transport.flush().await?;

        // 6. ServerInit.
        let mut init_header = [0u8; 24];
        transport.read_exact(&mut init_header).await?;
        let width = u16::from_be_bytes([init_header[0], init_header[1]]) as u32;
        let height = u16::from_be_bytes([init_header[2], init_header[3]]) as u32;
        let mut pf_bytes = [0u8; 16];
        pf_bytes.copy_from_slice(&init_header[4..20]);
        let server_pf = PixelFormat::decode(&pf_bytes);
        let name_len = u32::from_be_bytes([
            init_header[20],
            init_header[21],
            init_header[22],
            init_header[23],
        ]) as usize;
        let mut name = vec![0u8; name_len];
        transport.read_exact(&mut name).await?;

        if width == 0 || height == 0 {
            return Err(RfbError::InvalidFramebufferSize { width, height });
        }

        let framebuffer = Framebuffer::new(width, height)?;
        let mut conn = Self {
            transport,
            framebuffer,
            pixel_format: server_pf,
        };

        // 7. SetPixelFormat — request our preferred RGBA32-LE layout.
        conn.set_pixel_format(PixelFormat::rgba32_le()).await?;

        // 8. SetEncodings — Raw + CopyRect, plus the DesktopSize and
        // LastRect pseudo-encodings to be future-friendly. Most servers
        // negotiate down to Raw if Tight isn't offered.
        conn.set_encodings(&[
            encoding::COPY_RECT,
            encoding::RAW,
            encoding::PSEUDO_DESKTOP_SIZE,
            encoding::PSEUDO_LAST_RECT,
        ])
        .await?;

        Ok(conn)
    }

    pub fn framebuffer(&self) -> &Framebuffer {
        &self.framebuffer
    }

    pub fn framebuffer_size(&self) -> (u32, u32) {
        (self.framebuffer.width(), self.framebuffer.height())
    }

    /// Send a SetPixelFormat message and update our local view.
    pub async fn set_pixel_format(&mut self, pf: PixelFormat) -> Result<(), RfbError> {
        let mut msg = [0u8; 20];
        msg[0] = client_msg::SET_PIXEL_FORMAT;
        // bytes 1..4 padding
        msg[4..20].copy_from_slice(&pf.encode());
        self.transport.write_all(&msg).await?;
        self.transport.flush().await?;
        self.pixel_format = pf;
        Ok(())
    }

    /// Send a SetEncodings message.
    pub async fn set_encodings(&mut self, encodings: &[i32]) -> Result<(), RfbError> {
        let n = encodings.len();
        let mut msg = Vec::with_capacity(4 + n * 4);
        msg.push(client_msg::SET_ENCODINGS);
        msg.push(0); // padding
        msg.extend_from_slice(&(n as u16).to_be_bytes());
        for &enc in encodings {
            msg.extend_from_slice(&enc.to_be_bytes());
        }
        self.transport.write_all(&msg).await?;
        self.transport.flush().await?;
        Ok(())
    }

    /// Request a framebuffer update covering `(x,y,w,h)`. Set
    /// `incremental = false` to ask for a full re-send of the region;
    /// `true` requests only changes since the last update.
    pub async fn request_framebuffer_update(
        &mut self,
        incremental: bool,
        x: u16,
        y: u16,
        w: u16,
        h: u16,
    ) -> Result<(), RfbError> {
        let mut msg = [0u8; 10];
        msg[0] = client_msg::FRAMEBUFFER_UPDATE_REQUEST;
        msg[1] = if incremental { 1 } else { 0 };
        msg[2..4].copy_from_slice(&x.to_be_bytes());
        msg[4..6].copy_from_slice(&y.to_be_bytes());
        msg[6..8].copy_from_slice(&w.to_be_bytes());
        msg[8..10].copy_from_slice(&h.to_be_bytes());
        self.transport.write_all(&msg).await?;
        self.transport.flush().await?;
        Ok(())
    }

    /// Send a `KeyEvent` (RFB §7.5.4). `down = true` is press,
    /// `down = false` is release. The keysym is sent unmodified — no
    /// ARD remapping or platform translation happens here. Callers
    /// should resolve names via `keymap::key_for_name` first.
    pub async fn key_event(&mut self, keysym: u32, down: bool) -> Result<(), RfbError> {
        let mut msg = [0u8; 8];
        msg[0] = client_msg::KEY_EVENT;
        msg[1] = if down { 1 } else { 0 };
        // bytes 2..4 padding
        msg[4..8].copy_from_slice(&keysym.to_be_bytes());
        self.transport.write_all(&msg).await?;
        self.transport.flush().await?;
        Ok(())
    }

    /// Send a `PointerEvent` (RFB §7.5.5). `button_mask` is the
    /// bit-packed state of currently-held buttons (bit 0 = left,
    /// bit 1 = middle, bit 2 = right; bits 3..6 encode wheel pulses
    /// as transient down+up edges). `(x, y)` are framebuffer pixels.
    pub async fn pointer_event(
        &mut self,
        button_mask: u8,
        x: u16,
        y: u16,
    ) -> Result<(), RfbError> {
        let mut msg = [0u8; 6];
        msg[0] = client_msg::POINTER_EVENT;
        msg[1] = button_mask;
        msg[2..4].copy_from_slice(&x.to_be_bytes());
        msg[4..6].copy_from_slice(&y.to_be_bytes());
        self.transport.write_all(&msg).await?;
        self.transport.flush().await?;
        Ok(())
    }

    /// Read one server message and apply it to internal state.
    pub async fn next_message(&mut self) -> Result<ServerEvent, RfbError> {
        let mut tag = [0u8; 1];
        self.transport.read_exact(&mut tag).await?;
        match tag[0] {
            server_msg::FRAMEBUFFER_UPDATE => self.read_framebuffer_update().await,
            server_msg::BELL => Ok(ServerEvent::Bell),
            server_msg::SET_COLOUR_MAP_ENTRIES => {
                // 1 byte padding, 2 bytes first-colour, 2 bytes count,
                // then count*6 bytes of (R,G,B u16 each). We don't use
                // colour-map mode but must drain the bytes.
                let mut header = [0u8; 5];
                self.transport.read_exact(&mut header).await?;
                let count = u16::from_be_bytes([header[3], header[4]]) as usize;
                let mut entries = vec![0u8; count * 6];
                self.transport.read_exact(&mut entries).await?;
                Ok(ServerEvent::ColourMapEntries)
            }
            server_msg::SERVER_CUT_TEXT => {
                let mut header = [0u8; 7];
                self.transport.read_exact(&mut header).await?;
                let len = u32::from_be_bytes([header[3], header[4], header[5], header[6]]);
                let mut payload = vec![0u8; len as usize];
                self.transport.read_exact(&mut payload).await?;
                Ok(ServerEvent::ServerCutText)
            }
            other => Err(RfbError::UnexpectedMessageType(other)),
        }
    }

    async fn read_framebuffer_update(&mut self) -> Result<ServerEvent, RfbError> {
        let mut header = [0u8; 3];
        self.transport.read_exact(&mut header).await?;
        let n_rects = u16::from_be_bytes([header[1], header[2]]);
        let mut applied = 0u32;
        for _ in 0..n_rects {
            let mut rect_header = [0u8; 12];
            self.transport.read_exact(&mut rect_header).await?;
            let x = u16::from_be_bytes([rect_header[0], rect_header[1]]) as u32;
            let y = u16::from_be_bytes([rect_header[2], rect_header[3]]) as u32;
            let w = u16::from_be_bytes([rect_header[4], rect_header[5]]) as u32;
            let h = u16::from_be_bytes([rect_header[6], rect_header[7]]) as u32;
            let enc = i32::from_be_bytes([
                rect_header[8],
                rect_header[9],
                rect_header[10],
                rect_header[11],
            ]);
            match enc {
                encoding::RAW => {
                    let bpp = self.pixel_format.bits_per_pixel as usize / 8;
                    let len = (w as usize) * (h as usize) * bpp;
                    let mut buf = vec![0u8; len];
                    self.transport.read_exact(&mut buf).await?;
                    self.framebuffer.raw_rect(x, y, w, h, &buf)?;
                    applied += 1;
                }
                encoding::COPY_RECT => {
                    let mut src = [0u8; 4];
                    self.transport.read_exact(&mut src).await?;
                    let src_x = u16::from_be_bytes([src[0], src[1]]) as u32;
                    let src_y = u16::from_be_bytes([src[2], src[3]]) as u32;
                    self.framebuffer.copy_rect(x, y, src_x, src_y, w, h)?;
                    applied += 1;
                }
                encoding::PSEUDO_DESKTOP_SIZE => {
                    // Allocate a fresh framebuffer at the new size.
                    self.framebuffer = Framebuffer::new(w, h)?;
                }
                encoding::PSEUDO_LAST_RECT => {
                    // Marks "no more rectangles in this update" when
                    // num-rects was sentinel 0xFFFF. We honour it by
                    // breaking out early.
                    break;
                }
                other => return Err(RfbError::UnsupportedEncoding(other)),
            }
        }
        Ok(ServerEvent::FramebufferUpdated {
            rectangles: applied,
        })
    }
}

/// Pick a security type to satisfy. Prefers VNC-auth when a password is
/// available, falls back to None otherwise.
fn pick_security_type(offered: &[u8], have_password: bool) -> Result<u8, RfbError> {
    if have_password && offered.contains(&sec_type::VNC_AUTH) {
        return Ok(sec_type::VNC_AUTH);
    }
    if offered.contains(&sec_type::NONE) {
        return Ok(sec_type::NONE);
    }
    if offered.contains(&sec_type::VNC_AUTH) {
        // Password not supplied but server requires it.
        return Err(RfbError::PasswordRequired);
    }
    Err(RfbError::NoMutualSecurityType(offered.to_vec()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pick_none_when_offered_and_no_password() {
        assert_eq!(pick_security_type(&[1], false).unwrap(), sec_type::NONE);
    }

    #[test]
    fn pick_vnc_auth_when_password_present() {
        assert_eq!(pick_security_type(&[1, 2], true).unwrap(), sec_type::VNC_AUTH);
    }

    #[test]
    fn fall_back_to_none_when_password_present_but_vnc_not_offered() {
        assert_eq!(pick_security_type(&[1], true).unwrap(), sec_type::NONE);
    }

    #[test]
    fn require_password_when_only_vnc_offered() {
        assert!(matches!(
            pick_security_type(&[2], false),
            Err(RfbError::PasswordRequired)
        ));
    }

    #[test]
    fn no_mutual_when_unknown_type() {
        assert!(matches!(
            pick_security_type(&[42], false),
            Err(RfbError::NoMutualSecurityType(_))
        ));
    }
}
