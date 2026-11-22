//! A rectangle whose header is larger than the framebuffer carries only the part inside it.
//!
//! tart's VNC server on macOS 27.2.1 does this after a resize. The connection starts at
//! 1280x720, and a `DesktopSize` change sets 1024x768. A redraw then arrives as a ZRLE
//! rectangle with the header `0,0 1280x720`, but its tiles cover only `0,0 1024x720`. A second
//! rectangle covers `0,720 1024x48`. A base build saw this at Setup Assistant on 2026-11-22.

use std::io::Write;

use flate2::{write::ZlibEncoder, Compression};
use tokio::io::{duplex, AsyncWriteExt, BufReader};

use testanyware_rfb::{proto::PixelFormat, RfbConnection, ServerEvent};

const ZRLE: i32 = 16;
const DESKTOP_SIZE: i32 = -223;

/// The no-auth handshake of a `width`×`height` desktop.
fn handshake_script(width: u16, height: u16) -> Vec<u8> {
    let mut script = Vec::new();
    script.extend_from_slice(b"RFB 003.008\n");
    script.extend_from_slice(&[1, 1]);
    script.extend_from_slice(&0u32.to_be_bytes());
    script.extend_from_slice(&width.to_be_bytes());
    script.extend_from_slice(&height.to_be_bytes());
    script.extend_from_slice(&PixelFormat::rgba32_le().encode());
    script.extend_from_slice(&2u32.to_be_bytes());
    script.extend_from_slice(b"vm");
    script
}

fn rect_header(msg: &mut Vec<u8>, x: u16, y: u16, w: u16, h: u16, encoding: i32) {
    for field in [x, y, w, h] {
        msg.extend_from_slice(&field.to_be_bytes());
    }
    msg.extend_from_slice(&encoding.to_be_bytes());
}

/// The colour of the tile at (`column`, `row`), distinct for each tile so that a tile at the
/// wrong place shows.
fn tile_colour(column: usize, row: usize) -> [u8; 3] {
    [column as u8 * 8, row as u8 * 16, 0x40]
}

/// A tile stream for a `w`×`h` area: one solid tile for each 64×64 tile. Its tile rows take
/// the colours of rows `first_row` and on.
fn solid_tiles(w: usize, h: usize, first_row: usize) -> Vec<u8> {
    let mut tiles = Vec::new();
    for ty in (0..h).step_by(64) {
        for tx in (0..w).step_by(64) {
            tiles.push(1); // solid
            let [r, g, b] = tile_colour(tx / 64, first_row + ty / 64);
            tiles.extend_from_slice(&[b, g, r]); // CPIXEL: B, G, R
        }
    }
    tiles
}

/// One ZRLE payload of the persistent stream: its length, then its bytes up to a sync flush.
fn zrle_payload(msg: &mut Vec<u8>, stream: &mut ZlibEncoder<Vec<u8>>, tiles: &[u8]) {
    stream.write_all(tiles).unwrap();
    stream.flush().unwrap();
    let compressed = std::mem::take(stream.get_mut());
    msg.extend_from_slice(&(compressed.len() as u32).to_be_bytes());
    msg.extend_from_slice(&compressed);
}

#[tokio::test]
async fn a_stale_zrle_rectangle_after_a_resize_decodes_the_part_inside_the_framebuffer() {
    let mut script = handshake_script(1280, 720);

    // Update 1: the resize.
    script.extend_from_slice(&[0, 0]);
    script.extend_from_slice(&1u16.to_be_bytes());
    rect_header(&mut script, 0, 0, 1024, 768, DESKTOP_SIZE);

    // Update 2: the stale header, then the rest of the new framebuffer, on one zlib stream.
    let mut stream = ZlibEncoder::new(Vec::new(), Compression::default());
    script.extend_from_slice(&[0, 0]);
    script.extend_from_slice(&2u16.to_be_bytes());
    rect_header(&mut script, 0, 0, 1280, 720, ZRLE);
    zrle_payload(&mut script, &mut stream, &solid_tiles(1024, 720, 0));
    rect_header(&mut script, 0, 720, 1024, 48, ZRLE);
    zrle_payload(&mut script, &mut stream, &solid_tiles(1024, 48, 12));

    let (client, mut server) = duplex(1 << 20);
    server.write_all(&script).await.unwrap();
    let mut conn = RfbConnection::handshake(BufReader::new(client), None)
        .await
        .unwrap();

    conn.next_message().await.unwrap();
    assert_eq!(conn.framebuffer_size(), (1024, 768));

    let event = conn.next_message().await.unwrap();
    assert!(
        matches!(event, ServerEvent::FramebufferUpdated { rectangles: 2 }),
        "{event:?}"
    );

    let fb = conn.framebuffer();
    let rgb_at = |x: usize, y: usize| {
        let i = (y * 1024 + x) * 4;
        [fb.rgba()[i], fb.rgba()[i + 1], fb.rgba()[i + 2]]
    };
    assert_eq!(rgb_at(0, 0), tile_colour(0, 0));
    assert_eq!(rgb_at(1023, 0), tile_colour(15, 0));
    assert_eq!(rgb_at(1023, 719), tile_colour(15, 11));
    assert_eq!(rgb_at(0, 720), tile_colour(0, 12));
    assert_eq!(rgb_at(1023, 767), tile_colour(15, 12));
}
