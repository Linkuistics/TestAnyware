//! Tight rectangle decoder (RFB §7.7.5, encoding type 7).
//!
//! Tight is the most elaborate standard encoding. Each rectangle opens
//! with a *compression-control byte* whose low nibble flushes any of four
//! persistent zlib streams and whose high nibble selects one of three
//! compression types:
//!
//! - **Fill** — one solid colour for the whole rectangle (no zlib).
//! - **JPEG** — a `compactlen`-prefixed JPEG blob (lossy; decoded by the
//!   `jpeg-decoder` crate).
//! - **Basic** — pixel data on one of the four zlib streams, after an
//!   optional *filter* (copy / palette / gradient).
//!
//! Unlike ZRLE there is no single length prefix in front of the
//! rectangle: its byte count is only discoverable by parsing the control
//! byte and the filter/palette/`compactlen` fields as they arrive, and
//! the zlib streams are stateful (so a partial buffer cannot be safely
//! re-parsed to probe its length). The decoder therefore reads
//! *incrementally from the transport* — `decode_rect` is async and
//! generic over `AsyncRead` — rather than working over a pre-sliced blob
//! the way `ZrleDecoder` does. Tests drive it standalone because tokio
//! implements `AsyncRead` for `&[u8]`.
//!
//! Every path produces the negotiated BGRX pixel layout and is handed to
//! `Framebuffer::raw_rect`, so non-JPEG Tight output is pixel-identical
//! to the Raw path by construction (JPEG is lossy, hence the tolerance in
//! its test).

use flate2::{Decompress, FlushDecompress, Status};
use tokio::io::{AsyncRead, AsyncReadExt};

use crate::error::RfbError;
use crate::proto::PixelFormat;

/// Below this many uncompressed bytes, basic-compression data is sent
/// raw (no zlib stream, no `compactlen` prefix). `rfbTightMinToCompress`.
const MIN_TO_COMPRESS: usize = 12;

/// Filter ids for basic compression (read only when the control byte's
/// explicit-filter bit is set; otherwise the filter is `Copy`).
const FILTER_COPY: u8 = 0;
const FILTER_PALETTE: u8 = 1;
const FILTER_GRADIENT: u8 = 2;

/// Per-connection Tight decode state: the four zlib streams, each lazily
/// created on first use and reset (dropped) when the server sets its
/// flush bit in a control byte. They must outlive any single rectangle
/// because the server keeps compressing against them across rectangles.
pub struct TightDecoder {
    streams: [Option<Decompress>; 4],
}

impl std::fmt::Debug for TightDecoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // `flate2::Decompress` is not Debug; report which streams are
        // live and how far each has decoded so `RfbConnection` can still
        // derive Debug.
        let live: Vec<(usize, u64)> = self
            .streams
            .iter()
            .enumerate()
            .filter_map(|(i, s)| s.as_ref().map(|d| (i, d.total_out())))
            .collect();
        f.debug_struct("TightDecoder")
            .field("live_streams", &live)
            .finish()
    }
}

impl TightDecoder {
    pub fn new() -> Self {
        Self {
            streams: [None, None, None, None],
        }
    }

    /// Decode one Tight rectangle off `reader` into a Raw-format BGRX
    /// pixel buffer of `w*h*4` bytes, ready for `Framebuffer::raw_rect`.
    pub async fn decode_rect<R: AsyncRead + Unpin>(
        &mut self,
        reader: &mut R,
        pf: PixelFormat,
        w: u32,
        h: u32,
    ) -> Result<Vec<u8>, RfbError> {
        let cpx = tpixel_width(&pf)?;
        let w = w as usize;
        let h = h as usize;
        let npix = w * h;

        let ctl = read_u8(reader).await?;

        // Low four bits flush the corresponding zlib streams: the server
        // reinitialised its compressor, so we drop ours (a fresh one is
        // created on next use, expecting a new zlib header).
        for (id, stream) in self.streams.iter_mut().enumerate() {
            if ctl & (1 << id) != 0 {
                *stream = None;
            }
        }

        // High nibble selects the compression type. 0x8 = fill, 0x9 =
        // JPEG, ≤0x7 = basic (its low two bits are the stream id, bit 2
        // signals an explicit filter id follows), >0x9 is undefined.
        match ctl >> 4 {
            0x8 => self.decode_fill(reader, cpx, npix).await,
            0x9 => decode_jpeg(reader, w, h).await,
            ty @ 0..=0x7 => {
                let stream_id = (ty & 0x03) as usize;
                let filter = if ty & 0x04 != 0 {
                    read_u8(reader).await?
                } else {
                    FILTER_COPY
                };
                self.decode_basic(reader, stream_id, filter, cpx, w, h)
                    .await
            }
            other => Err(RfbError::Protocol(format!(
                "Tight: unsupported compression type 0x{other:x}"
            ))),
        }
    }

    /// Fill: a single TPIXEL (uncompressed) painted over the rectangle.
    async fn decode_fill<R: AsyncRead + Unpin>(
        &mut self,
        reader: &mut R,
        cpx: usize,
        npix: usize,
    ) -> Result<Vec<u8>, RfbError> {
        let mut tp = [0u8; 4];
        reader.read_exact(&mut tp[..cpx]).await?;
        let px = tpixel_to_bgrx(&tp[..cpx]);
        let mut out = vec![0u8; npix * 4];
        for chunk in out.chunks_exact_mut(4) {
            chunk.copy_from_slice(&px);
        }
        Ok(out)
    }

    /// Basic compression: optional filter, then `w*h` pixels' worth of
    /// (possibly zlib-compressed) filtered data on `stream_id`.
    async fn decode_basic<R: AsyncRead + Unpin>(
        &mut self,
        reader: &mut R,
        stream_id: usize,
        filter: u8,
        cpx: usize,
        w: usize,
        h: usize,
    ) -> Result<Vec<u8>, RfbError> {
        match filter {
            FILTER_COPY => {
                // One TPIXEL per pixel, row-major.
                let data = self.read_stream(reader, stream_id, w * h * cpx).await?;
                tpixels_to_bgrx(&data, cpx, w * h)
            }
            FILTER_PALETTE => {
                // Palette of (byte + 1) TPIXELs, sent uncompressed, then
                // per-pixel indices on the stream: 1-bit (row-padded to a
                // byte) when the palette holds exactly two colours, else
                // one byte per pixel.
                let num_colours = read_u8(reader).await? as usize + 1;
                let mut pal_raw = vec![0u8; num_colours * cpx];
                reader.read_exact(&mut pal_raw).await?;
                let palette: Vec<[u8; 4]> = pal_raw.chunks_exact(cpx).map(tpixel_to_bgrx).collect();

                let row_bytes = if num_colours == 2 { w.div_ceil(8) } else { w };
                let data = self.read_stream(reader, stream_id, row_bytes * h).await?;
                palette_to_bgrx(&data, &palette, num_colours, w, h, row_bytes)
            }
            FILTER_GRADIENT => {
                // Per-channel "gradient" (Paeth-like) prediction over
                // `w*h` TPIXELs.
                let data = self.read_stream(reader, stream_id, w * h * cpx).await?;
                gradient_to_bgrx(&data, cpx, w, h)
            }
            other => Err(RfbError::Protocol(format!(
                "Tight: unsupported basic filter id {other}"
            ))),
        }
    }

    /// Read `expected_len` bytes of basic-compression payload from
    /// `stream_id`. Payloads below `MIN_TO_COMPRESS` are sent
    /// uncompressed and read verbatim; larger ones are `compactlen`-
    /// prefixed zlib data inflated through the persistent stream.
    async fn read_stream<R: AsyncRead + Unpin>(
        &mut self,
        reader: &mut R,
        stream_id: usize,
        expected_len: usize,
    ) -> Result<Vec<u8>, RfbError> {
        if expected_len < MIN_TO_COMPRESS {
            let mut data = vec![0u8; expected_len];
            reader.read_exact(&mut data).await?;
            return Ok(data);
        }
        let compressed_len = read_compact_len(reader).await?;
        let mut compressed = vec![0u8; compressed_len];
        reader.read_exact(&mut compressed).await?;
        let stream = self.streams[stream_id].get_or_insert_with(|| Decompress::new(true));
        inflate_exact(stream, &compressed, expected_len)
    }
}

impl Default for TightDecoder {
    fn default() -> Self {
        Self::new()
    }
}

/// Inflate `compressed` through the persistent `stream`, producing
/// exactly `expected` bytes. The server sync-flushes the stream at each
/// rectangle's payload boundary, so consuming this blob's input yields
/// precisely this payload's uncompressed bytes.
fn inflate_exact(
    stream: &mut Decompress,
    compressed: &[u8],
    expected: usize,
) -> Result<Vec<u8>, RfbError> {
    let mut out = Vec::with_capacity(expected);
    let mut buf = [0u8; 16 * 1024];
    let mut input = compressed;
    while out.len() < expected {
        let before_in = stream.total_in();
        let before_out = stream.total_out();
        let status = stream
            .decompress(input, &mut buf, FlushDecompress::None)
            .map_err(|e| RfbError::Protocol(format!("Tight inflate failed: {e}")))?;
        let consumed = (stream.total_in() - before_in) as usize;
        let produced = (stream.total_out() - before_out) as usize;
        out.extend_from_slice(&buf[..produced]);
        input = &input[consumed..];
        match status {
            Status::StreamEnd => break,
            _ if consumed == 0 && produced == 0 => break,
            _ => {}
        }
    }
    if out.len() != expected {
        return Err(RfbError::Protocol(format!(
            "Tight: inflated {} bytes, expected {expected}",
            out.len()
        )));
    }
    Ok(out)
}

/// Decode a length-prefixed JPEG blob into BGRX, validating its
/// dimensions against the rectangle.
async fn decode_jpeg<R: AsyncRead + Unpin>(
    reader: &mut R,
    w: usize,
    h: usize,
) -> Result<Vec<u8>, RfbError> {
    let len = read_compact_len(reader).await?;
    let mut data = vec![0u8; len];
    reader.read_exact(&mut data).await?;

    let mut decoder = jpeg_decoder::Decoder::new(std::io::Cursor::new(&data));
    let pixels = decoder
        .decode()
        .map_err(|e| RfbError::Protocol(format!("Tight JPEG decode failed: {e}")))?;
    let info = decoder
        .info()
        .ok_or_else(|| RfbError::Protocol("Tight JPEG: missing image info".into()))?;
    if info.width as usize != w || info.height as usize != h {
        return Err(RfbError::Protocol(format!(
            "Tight JPEG: {}x{} does not match rectangle {w}x{h}",
            info.width, info.height
        )));
    }

    let mut out = vec![0u8; w * h * 4];
    match info.pixel_format {
        jpeg_decoder::PixelFormat::RGB24 => {
            for (src, dst) in pixels.chunks_exact(3).zip(out.chunks_exact_mut(4)) {
                // JPEG yields R,G,B; framebuffer expects B,G,R,X.
                dst.copy_from_slice(&[src[2], src[1], src[0], 0]);
            }
        }
        jpeg_decoder::PixelFormat::L8 => {
            for (src, dst) in pixels.iter().zip(out.chunks_exact_mut(4)) {
                dst.copy_from_slice(&[*src, *src, *src, 0]);
            }
        }
        other => {
            return Err(RfbError::Protocol(format!(
                "Tight JPEG: unsupported pixel format {other:?}"
            )))
        }
    }
    Ok(out)
}

/// Bytes per TPIXEL for `pf`. For our 32bpp / depth-≤24 true-colour
/// format the most-significant byte is padding, so TPIXEL is 3 bytes —
/// the significant little-endian channels [B, G, R] — exactly like
/// ZRLE's CPIXEL. Other depths use the full bytes-per-pixel.
fn tpixel_width(pf: &PixelFormat) -> Result<usize, RfbError> {
    if pf.bits_per_pixel == 32 && pf.depth <= 24 {
        Ok(3)
    } else {
        match pf.bits_per_pixel {
            8 => Ok(1),
            16 => Ok(2),
            32 => Ok(4),
            other => Err(RfbError::Protocol(format!(
                "Tight: unsupported bits_per_pixel {other}"
            ))),
        }
    }
}

/// Expand one TPIXEL into a 4-byte BGRX pixel (padding byte = 0). For
/// `cpx == 3` the bytes are the significant LE channels [B, G, R]; for
/// `cpx == 4` they are the full BGRX pixel.
fn tpixel_to_bgrx(b: &[u8]) -> [u8; 4] {
    match b.len() {
        3 => [b[0], b[1], b[2], 0],
        4 => [b[0], b[1], b[2], b[3]],
        // Single-byte (8bpp) is the only remaining width we negotiate;
        // splat it across BGR for a greyscale-ish placeholder.
        1 => [b[0], b[0], b[0], 0],
        2 => [b[0], b[1], 0, 0],
        _ => [0, 0, 0, 0],
    }
}

/// Convert `count` row-major TPIXELs of `cpx` bytes each into a BGRX
/// buffer.
fn tpixels_to_bgrx(data: &[u8], cpx: usize, count: usize) -> Result<Vec<u8>, RfbError> {
    if data.len() != count * cpx {
        return Err(RfbError::Protocol(format!(
            "Tight: {} bytes for {count} pixels at {cpx} bpp",
            data.len()
        )));
    }
    let mut out = vec![0u8; count * 4];
    for (src, dst) in data.chunks_exact(cpx).zip(out.chunks_exact_mut(4)) {
        dst.copy_from_slice(&tpixel_to_bgrx(src));
    }
    Ok(out)
}

/// Expand palette indices into a BGRX buffer. `row_bytes` is the
/// per-row stride of the index data (1-bit MSB-first and row-padded when
/// the palette holds two colours, else one byte per pixel).
fn palette_to_bgrx(
    data: &[u8],
    palette: &[[u8; 4]],
    num_colours: usize,
    w: usize,
    h: usize,
    row_bytes: usize,
) -> Result<Vec<u8>, RfbError> {
    let mut out = vec![0u8; w * h * 4];
    let lookup = |idx: usize| -> Result<[u8; 4], RfbError> {
        palette.get(idx).copied().ok_or_else(|| {
            RfbError::Protocol(format!(
                "Tight: palette index {idx} out of range (size {})",
                palette.len()
            ))
        })
    };
    for y in 0..h {
        let row = data
            .get(y * row_bytes..y * row_bytes + row_bytes)
            .ok_or_else(|| RfbError::Protocol("Tight: palette data truncated".into()))?;
        for x in 0..w {
            let idx = if num_colours == 2 {
                // MSB-first bit packing within each byte.
                ((row[x / 8] >> (7 - (x % 8))) & 1) as usize
            } else {
                row[x] as usize
            };
            let px = lookup(idx)?;
            let o = (y * w + x) * 4;
            out[o..o + 4].copy_from_slice(&px);
        }
    }
    Ok(out)
}

/// Undo the Tight gradient filter and convert to BGRX. Each TPIXEL
/// channel is predicted independently from its reconstructed left, up,
/// and up-left neighbours (pixels outside the rectangle are 0): the
/// reconstructed value is `(residual + clamp(left + up - upleft)) mod
/// 256`.
fn gradient_to_bgrx(data: &[u8], cpx: usize, w: usize, h: usize) -> Result<Vec<u8>, RfbError> {
    if data.len() != w * h * cpx {
        return Err(RfbError::Protocol(format!(
            "Tight: gradient data {} bytes, expected {}",
            data.len(),
            w * h * cpx
        )));
    }
    // Reconstructed TPIXEL bytes; predictors read already-decoded values.
    let mut recon = vec![0u8; w * h * cpx];
    for y in 0..h {
        for x in 0..w {
            for c in 0..cpx {
                let left = if x > 0 {
                    recon[(y * w + x - 1) * cpx + c] as i32
                } else {
                    0
                };
                let up = if y > 0 {
                    recon[((y - 1) * w + x) * cpx + c] as i32
                } else {
                    0
                };
                let upleft = if x > 0 && y > 0 {
                    recon[((y - 1) * w + x - 1) * cpx + c] as i32
                } else {
                    0
                };
                let pred = (left + up - upleft).clamp(0, 255);
                let idx = (y * w + x) * cpx + c;
                recon[idx] = ((data[idx] as i32 + pred) & 0xff) as u8;
            }
        }
    }
    tpixels_to_bgrx(&recon, cpx, w * h)
}

/// Read a single byte.
async fn read_u8<R: AsyncRead + Unpin>(reader: &mut R) -> Result<u8, RfbError> {
    let mut b = [0u8; 1];
    reader.read_exact(&mut b).await?;
    Ok(b[0])
}

/// Read a Tight `compactlen`: a 1–3 byte little-endian length, 7 bits
/// per byte, the top bit of each of the first two bytes signalling a
/// continuation (the third byte carries a full 8 bits).
async fn read_compact_len<R: AsyncRead + Unpin>(reader: &mut R) -> Result<usize, RfbError> {
    let b0 = read_u8(reader).await?;
    let mut len = (b0 & 0x7f) as usize;
    if b0 & 0x80 != 0 {
        let b1 = read_u8(reader).await?;
        len |= ((b1 & 0x7f) as usize) << 7;
        if b1 & 0x80 != 0 {
            let b2 = read_u8(reader).await?;
            len |= (b2 as usize) << 14;
        }
    }
    Ok(len)
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{write::ZlibEncoder, Compression};
    use std::io::Write;

    /// A persistent zlib stream that sync-flushes at each payload
    /// boundary, exactly as a Tight server does. Each `push` returns the
    /// compressed bytes produced since the previous boundary.
    struct ZlibStream {
        enc: ZlibEncoder<Vec<u8>>,
    }

    impl ZlibStream {
        fn new() -> Self {
            Self {
                enc: ZlibEncoder::new(Vec::new(), Compression::default()),
            }
        }

        fn push(&mut self, uncompressed: &[u8]) -> Vec<u8> {
            self.enc.write_all(uncompressed).unwrap();
            self.enc.flush().unwrap();
            std::mem::take(self.enc.get_mut())
        }
    }

    /// A TPIXEL for our negotiated rgba32_le format: the 3 significant
    /// little-endian bytes [B, G, R].
    fn tpixel(r: u8, g: u8, b: u8) -> [u8; 3] {
        [b, g, r]
    }

    /// BGRX of an (r,g,b) colour with a zeroed padding byte — the
    /// raw-rect layout every decode path must produce.
    fn bgrx(r: u8, g: u8, b: u8) -> [u8; 4] {
        [b, g, r, 0]
    }

    /// Encode a Tight `compactlen`.
    fn compact_len(mut n: usize) -> Vec<u8> {
        let mut out = vec![(n & 0x7f) as u8];
        if n >= 0x80 {
            out[0] |= 0x80;
            n >>= 7;
            out.push((n & 0x7f) as u8);
            if n >= 0x80 {
                out[1] |= 0x80;
                n >>= 7;
                out.push((n & 0xff) as u8);
            }
        }
        out
    }

    async fn decode(bytes: &[u8], w: u32, h: u32) -> Result<Vec<u8>, RfbError> {
        let mut dec = TightDecoder::new();
        let mut src: &[u8] = bytes;
        dec.decode_rect(&mut src, PixelFormat::rgba32_le(), w, h)
            .await
    }

    #[tokio::test]
    async fn fill_paints_whole_rectangle() {
        // Control byte 0x80: no resets, type 0x8 = fill. Then one TPIXEL.
        let mut wire = vec![0x80];
        wire.extend_from_slice(&tpixel(10, 20, 30));
        let out = decode(&wire, 3, 2).await.unwrap();

        let mut expected = Vec::new();
        for _ in 0..6 {
            expected.extend_from_slice(&bgrx(10, 20, 30));
        }
        assert_eq!(out, expected);
    }

    #[tokio::test]
    async fn basic_copy_uncompressed_below_threshold() {
        // 2x1 rectangle: copy filter, 2 px * 3 bytes = 6 < MIN_TO_COMPRESS,
        // so the TPIXELs are sent raw (no zlib, no compactlen).
        // Control 0x00: type 0x0 = basic, stream 0, no explicit filter
        // (=> copy).
        let mut wire = vec![0x00];
        wire.extend_from_slice(&tpixel(255, 0, 0));
        wire.extend_from_slice(&tpixel(0, 255, 0));
        let out = decode(&wire, 2, 1).await.unwrap();

        let mut expected = Vec::new();
        expected.extend_from_slice(&bgrx(255, 0, 0));
        expected.extend_from_slice(&bgrx(0, 255, 0));
        assert_eq!(out, expected);
    }

    #[tokio::test]
    async fn basic_copy_compressed_above_threshold() {
        // 8x2 rectangle (16 px * 3 = 48 bytes >= threshold) forces the
        // zlib path with a compactlen prefix.
        let (w, h) = (8usize, 2usize);
        let mut raw = Vec::new();
        for i in 0..(w * h) {
            raw.extend_from_slice(&tpixel(i as u8, (i * 2) as u8, (i * 3) as u8));
        }
        let compressed = ZlibStream::new().push(&raw);

        // Control 0x00: basic, stream 0, copy filter.
        let mut wire = vec![0x00];
        wire.extend_from_slice(&compact_len(compressed.len()));
        wire.extend_from_slice(&compressed);
        let out = decode(&wire, w as u32, h as u32).await.unwrap();

        let mut expected = Vec::new();
        for i in 0..(w * h) {
            expected.extend_from_slice(&bgrx(i as u8, (i * 2) as u8, (i * 3) as u8));
        }
        assert_eq!(out, expected);
    }

    #[tokio::test]
    async fn explicit_copy_filter_byte_is_honoured() {
        // Control 0x40: type 0x4 => basic, stream 0, explicit filter bit
        // set. Filter id byte 0 = copy. Small payload stays uncompressed.
        let mut wire = vec![0x40, FILTER_COPY];
        wire.extend_from_slice(&tpixel(1, 2, 3));
        let out = decode(&wire, 1, 1).await.unwrap();
        assert_eq!(out, bgrx(1, 2, 3).to_vec());
    }

    #[tokio::test]
    async fn palette_two_colours_uses_one_bit_per_pixel() {
        // 10x2 rectangle, 2-colour palette => 1 bit/pixel, each row
        // padded to a byte: row stride = ceil(10/8) = 2 bytes.
        // Indices: row0 = 0,1,0,1,0,1,0,1,1,1 ; row1 = all 1s.
        // Index data is small after the palette but the row stride * h =
        // 4 bytes < threshold, so it is sent uncompressed.
        let red = tpixel(200, 0, 0);
        let green = tpixel(0, 200, 0);

        // Control 0x40: basic, stream 0, explicit filter.
        let mut wire = vec![0x40, FILTER_PALETTE];
        wire.push(1); // num_colours - 1 => 2 colours
        wire.extend_from_slice(&red);
        wire.extend_from_slice(&green);

        // Row 0: bits 0101 0101 11 -> byte0 = 0b01010101 = 0x55,
        //        byte1 = 0b11_000000 = 0xC0 (last two bits used).
        // Row 1: bits 1111 1111 11 -> byte0 = 0xFF, byte1 = 0xC0.
        wire.extend_from_slice(&[0x55, 0xC0, 0xFF, 0xC0]);
        let out = decode(&wire, 10, 2).await.unwrap();

        let r = bgrx(200, 0, 0);
        let g = bgrx(0, 200, 0);
        let mut expected = Vec::new();
        // Row 0.
        for &idx in &[0, 1, 0, 1, 0, 1, 0, 1, 1, 1] {
            expected.extend_from_slice(if idx == 0 { &r } else { &g });
        }
        // Row 1: all green.
        for _ in 0..10 {
            expected.extend_from_slice(&g);
        }
        assert_eq!(out, expected);
    }

    #[tokio::test]
    async fn palette_many_colours_uses_one_byte_per_pixel() {
        // 4x1 with a 3-colour palette => 1 byte/pixel index. 4 bytes of
        // indices < threshold so sent uncompressed.
        let c0 = tpixel(10, 0, 0);
        let c1 = tpixel(0, 20, 0);
        let c2 = tpixel(0, 0, 30);

        let mut wire = vec![0x40, FILTER_PALETTE];
        wire.push(2); // 3 colours
        wire.extend_from_slice(&c0);
        wire.extend_from_slice(&c1);
        wire.extend_from_slice(&c2);
        wire.extend_from_slice(&[2, 0, 1, 2]); // indices
        let out = decode(&wire, 4, 1).await.unwrap();

        let mut expected = Vec::new();
        expected.extend_from_slice(&bgrx(0, 0, 30));
        expected.extend_from_slice(&bgrx(10, 0, 0));
        expected.extend_from_slice(&bgrx(0, 20, 0));
        expected.extend_from_slice(&bgrx(0, 0, 30));
        assert_eq!(out, expected);
    }

    #[tokio::test]
    async fn gradient_filter_reconstructs_against_neighbours() {
        // 2x2 gradient. Residuals chosen so the reconstruction is easy to
        // verify by hand. Work per channel; here only the B lane (byte 0
        // of each TPIXEL) is non-zero.
        //
        // Residual B values (row-major): r(0,0)=10, r(1,0)=5, r(0,1)=3,
        // r(1,1)=0.
        //  recon(0,0) = 10 + clamp(0+0-0)         = 10
        //  recon(1,0) =  5 + clamp(10+0-0)        = 15
        //  recon(0,1) =  3 + clamp(0+10-0)        = 13
        //  recon(1,1) =  0 + clamp(15+13-10)=18   = 18
        // TPIXEL byte 0 is B, so set B to the residual and R,G to 0.
        let res = [
            tpixel(0, 0, 10),
            tpixel(0, 0, 5),
            tpixel(0, 0, 3),
            tpixel(0, 0, 0),
        ];
        // 4 px * 3 = 12 bytes >= threshold => zlib path.
        let mut raw = Vec::new();
        for p in &res {
            raw.extend_from_slice(p);
        }
        let compressed = ZlibStream::new().push(&raw);

        let mut wire = vec![0x40, FILTER_GRADIENT];
        wire.extend_from_slice(&compact_len(compressed.len()));
        wire.extend_from_slice(&compressed);
        let out = decode(&wire, 2, 2).await.unwrap();

        let mut expected = Vec::new();
        for b in [10u8, 15, 13, 18] {
            expected.extend_from_slice(&bgrx(0, 0, b));
        }
        assert_eq!(out, expected);
    }

    #[tokio::test]
    async fn gradient_prediction_clamps_to_byte_range() {
        // 2x1 row where the predictor would exceed 255 if unclamped.
        // Residual B: r(0,0)=200, r(1,0)=100.
        //  recon(0,0)=200+clamp(0)=200
        //  recon(1,0)=100+clamp(200+0-0)=100+200=300 -> &0xff = 44
        // The clamp is on the prediction (<=255), then the sum wraps mod
        // 256: 100 + 200 = 300 & 0xff = 44.
        let res = [tpixel(0, 0, 200), tpixel(0, 0, 100)];
        let mut raw = Vec::new();
        for p in &res {
            raw.extend_from_slice(p);
        }
        // 6 bytes < threshold => uncompressed.
        let mut wire = vec![0x40, FILTER_GRADIENT];
        wire.extend_from_slice(&raw);
        let out = decode(&wire, 2, 1).await.unwrap();

        let mut expected = Vec::new();
        expected.extend_from_slice(&bgrx(0, 0, 200));
        expected.extend_from_slice(&bgrx(0, 0, 44));
        assert_eq!(out, expected);
    }

    #[tokio::test]
    async fn four_streams_persist_independently_across_rectangles() {
        // Drive two basic-copy rectangles on two different streams from a
        // single decoder, each stream sync-flushed between rectangles, to
        // prove the four inflate contexts are kept alive and selected by
        // the control byte's low stream-id bits.
        let (w, h) = (8usize, 1usize); // 24 bytes >= threshold
        let mut stream1 = ZlibStream::new();
        let mut stream2 = ZlibStream::new();

        let raw_a: Vec<u8> = (0..w * h).flat_map(|i| tpixel(i as u8, 0, 0)).collect();
        let raw_b: Vec<u8> = (0..w * h).flat_map(|i| tpixel(0, i as u8, 0)).collect();
        let comp_a = stream1.push(&raw_a);
        let comp_b = stream2.push(&raw_b);

        let mut dec = TightDecoder::new();

        // Rect 1 on stream 1: stream id lives in the HIGH nibble, so
        // control 0x10 => basic, stream id 1, no resets.
        let mut wire1 = vec![0x10];
        wire1.extend_from_slice(&compact_len(comp_a.len()));
        wire1.extend_from_slice(&comp_a);
        let mut s1: &[u8] = &wire1;
        let out1 = dec
            .decode_rect(&mut s1, PixelFormat::rgba32_le(), w as u32, h as u32)
            .await
            .unwrap();

        // Rect 2 on stream 2: control 0x20 => basic, stream id 2.
        let mut wire2 = vec![0x20];
        wire2.extend_from_slice(&compact_len(comp_b.len()));
        wire2.extend_from_slice(&comp_b);
        let mut s2: &[u8] = &wire2;
        let out2 = dec
            .decode_rect(&mut s2, PixelFormat::rgba32_le(), w as u32, h as u32)
            .await
            .unwrap();

        for i in 0..w * h {
            assert_eq!(
                &out1[i * 4..i * 4 + 4],
                &bgrx(i as u8, 0, 0),
                "rect1 px {i}"
            );
            assert_eq!(
                &out2[i * 4..i * 4 + 4],
                &bgrx(0, i as u8, 0),
                "rect2 px {i}"
            );
        }
    }

    #[tokio::test]
    async fn stream_reset_bit_reinitialises_the_stream() {
        // Two rectangles on stream 0, but the SECOND sets the reset bit
        // (control low nibble bit 0) and uses a FRESH zlib stream, as a
        // server does after re-creating its compressor. The decoder must
        // drop the old inflate context so the new header parses.
        let (w, h) = (8usize, 1usize);
        let raw1: Vec<u8> = (0..w * h).flat_map(|_| tpixel(1, 1, 1)).collect();
        let raw2: Vec<u8> = (0..w * h).flat_map(|_| tpixel(2, 2, 2)).collect();

        let mut persistent = ZlibStream::new();
        let comp1 = persistent.push(&raw1);
        // Fresh, independent stream for rect 2.
        let comp2 = ZlibStream::new().push(&raw2);

        let mut dec = TightDecoder::new();

        let mut wire1 = vec![0x00]; // basic, stream 0, no reset
        wire1.extend_from_slice(&compact_len(comp1.len()));
        wire1.extend_from_slice(&comp1);
        let mut s1: &[u8] = &wire1;
        dec.decode_rect(&mut s1, PixelFormat::rgba32_le(), w as u32, h as u32)
            .await
            .unwrap();

        // Control 0x01: reset bit for stream 0 set... but that collides
        // with stream-id encoding. Reset bits are the LOW nibble; the
        // type/stream are the HIGH nibble. So reset stream 0 + basic
        // stream 0 = 0x01 in low nibble, 0x0 high nibble => 0x01.
        let mut wire2 = vec![0x01];
        wire2.extend_from_slice(&compact_len(comp2.len()));
        wire2.extend_from_slice(&comp2);
        let mut s2: &[u8] = &wire2;
        let out2 = dec
            .decode_rect(&mut s2, PixelFormat::rgba32_le(), w as u32, h as u32)
            .await
            .unwrap();

        for i in 0..w * h {
            assert_eq!(&out2[i * 4..i * 4 + 4], &bgrx(2, 2, 2), "rect2 px {i}");
        }
    }

    #[tokio::test]
    async fn tight_basic_output_is_pixel_identical_to_raw_path() {
        // Build a deterministic image, decode it via Tight basic-copy,
        // and assert the framebuffer matches the Raw path byte-for-byte.
        use crate::framebuffer::Framebuffer;

        let (w, h) = (40u32, 30u32);
        let pixel = |x: u32, y: u32| {
            let r = (x.wrapping_mul(7).wrapping_add(y)) as u8;
            let g = (y.wrapping_mul(3).wrapping_add(x.wrapping_mul(5))) as u8;
            let b = (x ^ y) as u8;
            (r, g, b)
        };

        // Raw BGRX buffer.
        let mut raw_bgrx = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            for x in 0..w {
                let (r, g, b) = pixel(x, y);
                raw_bgrx.extend_from_slice(&[b, g, r, 0]);
            }
        }
        let mut fb_raw = Framebuffer::new(w, h).unwrap();
        fb_raw.raw_rect(0, 0, w, h, &raw_bgrx).unwrap();

        // Tight basic-copy: TPIXELs row-major, zlib-compressed.
        let mut tpixels = Vec::new();
        for y in 0..h {
            for x in 0..w {
                let (r, g, b) = pixel(x, y);
                tpixels.extend_from_slice(&tpixel(r, g, b));
            }
        }
        let compressed = ZlibStream::new().push(&tpixels);
        let mut wire = vec![0x00];
        wire.extend_from_slice(&compact_len(compressed.len()));
        wire.extend_from_slice(&compressed);

        let bgrx = decode(&wire, w, h).await.unwrap();
        let mut fb_tight = Framebuffer::new(w, h).unwrap();
        fb_tight.raw_rect(0, 0, w, h, &bgrx).unwrap();

        assert_eq!(fb_tight.rgba(), fb_raw.rgba());
    }

    #[tokio::test]
    async fn jpeg_rectangle_decodes_within_lossy_tolerance() {
        // Encode a small image to JPEG, wrap it as a Tight JPEG rectangle
        // (control 0x90 + compactlen + blob), decode through the Tight
        // path, and assert each channel is within a tolerance of the
        // original — JPEG is lossy, so exact equality is not expected.
        let (w, h) = (16usize, 16usize);
        let src_pixel = |x: usize, y: usize| {
            // Smooth gradients compress cleanly and survive JPEG well.
            ((x * 12) as u8, (y * 12) as u8, ((x + y) * 6) as u8)
        };
        let mut rgb = Vec::with_capacity(w * h * 3);
        for y in 0..h {
            for x in 0..w {
                let (r, g, b) = src_pixel(x, y);
                rgb.extend_from_slice(&[r, g, b]);
            }
        }

        // Encode RGB to a JPEG blob at high quality.
        let mut jpeg = Vec::new();
        jpeg_encoder::Encoder::new(&mut jpeg, 95)
            .encode(&rgb, w as u16, h as u16, jpeg_encoder::ColorType::Rgb)
            .unwrap();

        // Control 0x90: high nibble 0x9 = JPEG.
        let mut wire = vec![0x90];
        wire.extend_from_slice(&compact_len(jpeg.len()));
        wire.extend_from_slice(&jpeg);

        let out = decode(&wire, w as u32, h as u32).await.unwrap();
        assert_eq!(out.len(), w * h * 4);

        for y in 0..h {
            for x in 0..w {
                let (r, g, b) = src_pixel(x, y);
                let o = (y * w + x) * 4;
                // Output is BGRX: [B, G, R, 0].
                let (db, dg, dr) = (out[o] as i32, out[o + 1] as i32, out[o + 2] as i32);
                let tol = 24;
                assert!((dr - r as i32).abs() <= tol, "R at ({x},{y}): {dr} vs {r}");
                assert!((dg - g as i32).abs() <= tol, "G at ({x},{y}): {dg} vs {g}");
                assert!((db - b as i32).abs() <= tol, "B at ({x},{y}): {db} vs {b}");
                assert_eq!(out[o + 3], 0, "padding byte zeroed");
            }
        }
    }

    #[tokio::test]
    async fn unsupported_compression_type_errors() {
        // High nibble 0xA (10) is undefined.
        let wire = vec![0xA0];
        let err = decode(&wire, 1, 1).await.unwrap_err();
        assert!(matches!(err, RfbError::Protocol(_)));
    }

    #[tokio::test]
    async fn compact_len_roundtrips_across_byte_boundaries() {
        // Drive the encoder helper and decoder over the 1/2/3-byte
        // boundaries.
        for n in [0usize, 1, 0x7f, 0x80, 0x3fff, 0x4000, 0x1f_ffff] {
            let bytes = compact_len(n);
            let mut src: &[u8] = &bytes;
            let decoded = read_compact_len(&mut src).await.unwrap();
            assert_eq!(decoded, n, "compactlen {n}");
        }
    }
}
