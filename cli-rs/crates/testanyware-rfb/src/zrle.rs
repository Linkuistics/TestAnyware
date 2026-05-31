//! ZRLE rectangle decoder (RFB §7.7.6, encoding type 16).
//!
//! ZRLE compresses a rectangle with a *single zlib stream that persists
//! for the whole connection*. After inflation the rectangle is split
//! into 64×64 tiles (row-major; edge tiles are smaller), each carrying a
//! sub-encoding byte. We decode every tile into the negotiated BGRX
//! pixel layout — identical to what a `Raw` rectangle carries — and hand
//! the assembled buffer to `Framebuffer::raw_rect`, so ZRLE output is
//! pixel-identical to the Raw path by construction.

use flate2::{Decompress, FlushDecompress, Status};

use crate::error::RfbError;
use crate::proto::PixelFormat;

/// Side length of a ZRLE tile.
const TILE: usize = 64;

/// Per-connection ZRLE decode state. Owns the single zlib stream that
/// persists for the whole RFB connection (it must outlive any single
/// rectangle decode — the server never resets it between rectangles).
pub struct ZrleDecoder {
    inflate: Decompress,
}

impl std::fmt::Debug for ZrleDecoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // `flate2::Decompress` is not Debug; summarise the stream
        // position instead so `RfbConnection` can still derive Debug.
        f.debug_struct("ZrleDecoder")
            .field("total_in", &self.inflate.total_in())
            .field("total_out", &self.inflate.total_out())
            .finish()
    }
}

impl ZrleDecoder {
    pub fn new() -> Self {
        // `true` selects the zlib wrapper (header + adler32), which is
        // what ZRLE's stream uses.
        Self {
            inflate: Decompress::new(true),
        }
    }

    /// Decode one ZRLE rectangle (its raw, length-prefix-stripped
    /// `compressed` blob) into a Raw-format pixel buffer of `w*h*4`
    /// bytes in the negotiated BGRX layout, ready for
    /// `Framebuffer::raw_rect`.
    pub fn decode_rect(
        &mut self,
        pf: PixelFormat,
        w: u32,
        h: u32,
        compressed: &[u8],
    ) -> Result<Vec<u8>, RfbError> {
        let inflated = self.inflate_rect(compressed)?;
        tiles_to_bgrx(&inflated, w as usize, h as usize, cpixel_width(&pf)?)
    }

    /// Feed one rectangle's compressed blob into the persistent zlib
    /// stream and return every uncompressed byte it yields. The server
    /// sync-flushes the stream at each rectangle boundary, so consuming
    /// all of this blob's input produces exactly this rectangle's tile
    /// bytes.
    fn inflate_rect(&mut self, compressed: &[u8]) -> Result<Vec<u8>, RfbError> {
        let mut out = Vec::with_capacity(compressed.len() * 4);
        let mut buf = [0u8; 16 * 1024];
        let mut input = compressed;
        loop {
            let before_in = self.inflate.total_in();
            let before_out = self.inflate.total_out();
            let status = self
                .inflate
                .decompress(input, &mut buf, FlushDecompress::None)
                .map_err(|e| RfbError::Protocol(format!("ZRLE inflate failed: {e}")))?;
            let consumed = (self.inflate.total_in() - before_in) as usize;
            let produced = (self.inflate.total_out() - before_out) as usize;
            out.extend_from_slice(&buf[..produced]);
            input = &input[consumed..];
            match status {
                Status::StreamEnd => break,
                // No progress with input still available and output
                // space free means the rectangle's data is exhausted
                // (sync-flush boundary) — stop without consuming the
                // next rectangle's bytes.
                _ if consumed == 0 && produced == 0 => break,
                _ => {}
            }
        }
        Ok(out)
    }
}

/// Bytes per CPIXEL for `pf`. For our 32bpp / depth-≤24 true-colour
/// format the most-significant byte is padding, so CPIXEL is 3 bytes
/// (the significant little-endian bytes [B, G, R]) — the classic ZRLE
/// width trap. Other depths use the full bytes-per-pixel.
fn cpixel_width(pf: &PixelFormat) -> Result<usize, RfbError> {
    if pf.bits_per_pixel == 32 && pf.depth <= 24 {
        Ok(3)
    } else {
        match pf.bits_per_pixel {
            8 => Ok(1),
            16 => Ok(2),
            32 => Ok(4),
            other => Err(RfbError::Protocol(format!(
                "ZRLE: unsupported bits_per_pixel {other}"
            ))),
        }
    }
}

/// Split the inflated rectangle into 64×64 tiles and decode each into
/// the `w*h*4` BGRX output buffer.
fn tiles_to_bgrx(inflated: &[u8], w: usize, h: usize, cpx: usize) -> Result<Vec<u8>, RfbError> {
    let mut out = vec![0u8; w * h * 4];
    let mut cur = Cursor::new(inflated);
    let mut ty = 0;
    while ty < h {
        let th = (h - ty).min(TILE);
        let mut tx = 0;
        while tx < w {
            let tw = (w - tx).min(TILE);
            let mut sink = TileSink {
                out: &mut out,
                rect_w: w,
                ox: tx,
                oy: ty,
                tw,
                th,
            };
            decode_tile(&mut cur, &mut sink, cpx)?;
            tx += TILE;
        }
        ty += TILE;
    }
    if cur.remaining() != 0 {
        return Err(RfbError::Protocol(format!(
            "ZRLE: {} trailing bytes after {w}x{h} rectangle",
            cur.remaining()
        )));
    }
    Ok(out)
}

/// Decode one tile's sub-encoding into `sink`.
fn decode_tile(cur: &mut Cursor, sink: &mut TileSink, cpx: usize) -> Result<(), RfbError> {
    let sub = cur.byte()?;
    match sub {
        0 => {
            // Raw: one CPIXEL per pixel, row-major.
            for p in 0..sink.total() {
                let px = cur.pixel(cpx)?;
                sink.put_flat(p, px);
            }
            Ok(())
        }
        1 => {
            // Solid: one CPIXEL fills the whole tile.
            let px = cur.pixel(cpx)?;
            for p in 0..sink.total() {
                sink.put_flat(p, px);
            }
            Ok(())
        }
        2..=16 => {
            // Packed palette: `sub` CPIXELs, then bit-packed indices,
            // each tile row restarting on a byte boundary.
            let palette = read_palette(cur, sub as usize, cpx)?;
            let bpi = bits_per_index(sub as usize);
            let mask = (1u8 << bpi) - 1;
            let row_bytes = (sink.tw * bpi).div_ceil(8);
            for row in 0..sink.th {
                let bytes = cur.take(row_bytes)?;
                for col in 0..sink.tw {
                    let bit_off = col * bpi;
                    // MSB-first within each byte.
                    let shift = 8 - (bit_off % 8) - bpi;
                    let idx = ((bytes[bit_off / 8] >> shift) & mask) as usize;
                    sink.put(col, row, palette_lookup(&palette, idx)?);
                }
            }
            Ok(())
        }
        128 => {
            // Plain RLE: flat runs of (CPIXEL, run-length) over the
            // tile's pixels, wrapping across rows.
            let mut filled = 0;
            while filled < sink.total() {
                let px = cur.pixel(cpx)?;
                let run = cur.run_length()?;
                sink.fill_run(filled, run, px)?;
                filled += run;
            }
            Ok(())
        }
        130..=255 => {
            // Palette RLE: palette of (sub - 128) CPIXELs, then runs.
            // Each run starts with an index byte; its low 7 bits are the
            // palette index, and its top bit signals that a run-length
            // follows (otherwise the run is a single pixel).
            let palette = read_palette(cur, sub as usize - 128, cpx)?;
            let mut filled = 0;
            while filled < sink.total() {
                let index_byte = cur.byte()?;
                let px = palette_lookup(&palette, (index_byte & 0x7f) as usize)?;
                let run = if index_byte & 0x80 != 0 {
                    cur.run_length()?
                } else {
                    1
                };
                sink.fill_run(filled, run, px)?;
                filled += run;
            }
            Ok(())
        }
        other => Err(RfbError::Protocol(format!(
            "ZRLE: unsupported tile sub-encoding {other}"
        ))),
    }
}

/// A 64×64-or-smaller tile at origin `(ox, oy)` writing into the
/// rectangle's `rect_w`-wide BGRX buffer. Bundles the placement so the
/// per-sub-encoding decoders stay parameter-light.
struct TileSink<'a> {
    out: &'a mut [u8],
    rect_w: usize,
    ox: usize,
    oy: usize,
    tw: usize,
    th: usize,
}

impl TileSink<'_> {
    fn total(&self) -> usize {
        self.tw * self.th
    }

    /// Place a pixel by flat tile index (row-major within `tw`).
    fn put_flat(&mut self, p: usize, px: [u8; 4]) {
        self.put(p % self.tw, p / self.tw, px);
    }

    /// Place a pixel at tile-local `(col, row)`.
    fn put(&mut self, col: usize, row: usize, px: [u8; 4]) {
        let i = ((self.oy + row) * self.rect_w + self.ox + col) * 4;
        self.out[i..i + 4].copy_from_slice(&px);
    }

    /// Fill `count` pixels of `px` starting at flat tile index `start`.
    fn fill_run(&mut self, start: usize, count: usize, px: [u8; 4]) -> Result<(), RfbError> {
        if start + count > self.total() {
            return Err(RfbError::Protocol("ZRLE: RLE run overflows tile".into()));
        }
        for p in start..start + count {
            self.put_flat(p, px);
        }
        Ok(())
    }
}

/// Bits needed to index a packed palette of `n` colours (2→1, 3..4→2,
/// 5..16→4).
fn bits_per_index(n: usize) -> usize {
    if n <= 2 {
        1
    } else if n <= 4 {
        2
    } else {
        4
    }
}

/// Read `n` CPIXELs into a palette of BGRX pixels.
fn read_palette(cur: &mut Cursor, n: usize, cpx: usize) -> Result<Vec<[u8; 4]>, RfbError> {
    (0..n).map(|_| cur.pixel(cpx)).collect()
}

fn palette_lookup(palette: &[[u8; 4]], idx: usize) -> Result<[u8; 4], RfbError> {
    palette.get(idx).copied().ok_or_else(|| {
        RfbError::Protocol(format!(
            "ZRLE: palette index {idx} out of range (size {})",
            palette.len()
        ))
    })
}

/// A bounds-checked forward reader over the inflated tile stream.
struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }

    fn byte(&mut self) -> Result<u8, RfbError> {
        let b = *self.data.get(self.pos).ok_or_else(underrun)?;
        self.pos += 1;
        Ok(b)
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], RfbError> {
        let end = self.pos.checked_add(n).ok_or_else(underrun)?;
        let slice = self.data.get(self.pos..end).ok_or_else(underrun)?;
        self.pos = end;
        Ok(slice)
    }

    /// Read an RLE run length: one greater than the sum of a chain of
    /// bytes in which every byte but the last is 255.
    fn run_length(&mut self) -> Result<usize, RfbError> {
        let mut len = 1usize;
        loop {
            let b = self.byte()?;
            len += b as usize;
            if b != 255 {
                return Ok(len);
            }
        }
    }

    /// Read one CPIXEL of `cpx` bytes and expand it to a 4-byte BGRX
    /// pixel (padding byte = 0). For `cpx == 3` the bytes are the
    /// significant LE channels [B, G, R]; for `cpx == 4` they are the
    /// full BGRX pixel.
    fn pixel(&mut self, cpx: usize) -> Result<[u8; 4], RfbError> {
        let b = self.take(cpx)?;
        Ok(match cpx {
            3 => [b[0], b[1], b[2], 0],
            4 => [b[0], b[1], b[2], b[3]],
            _ => {
                return Err(RfbError::Protocol(format!(
                    "ZRLE: unsupported CPIXEL width {cpx}"
                )))
            }
        })
    }
}

fn underrun() -> RfbError {
    RfbError::Protocol("ZRLE: tile stream ended mid-pixel".into())
}

impl Default for ZrleDecoder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{write::ZlibEncoder, Compression};
    use std::io::Write;

    /// A persistent zlib stream that sync-flushes at each rectangle
    /// boundary, exactly as an RFB server does. Each `push` returns the
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

    /// Compress a single rectangle's tile stream as a self-contained
    /// (sync-flushed) blob from a fresh stream.
    fn zlib_one(uncompressed: &[u8]) -> Vec<u8> {
        ZlibStream::new().push(uncompressed)
    }

    /// A CPIXEL for our negotiated rgba32_le format: the 3 significant
    /// little-endian bytes [B, G, R].
    fn cpixel(r: u8, g: u8, b: u8) -> [u8; 3] {
        [b, g, r]
    }

    #[test]
    fn raw_tile_smaller_than_64_decodes_to_bgrx() {
        // One 2x1 rectangle, single raw tile: red then green.
        let mut tile = vec![0u8]; // sub-encoding 0 = raw
        tile.extend_from_slice(&cpixel(255, 0, 0));
        tile.extend_from_slice(&cpixel(0, 255, 0));
        let compressed = zlib_one(&tile);

        let mut dec = ZrleDecoder::new();
        let bgrx = dec
            .decode_rect(PixelFormat::rgba32_le(), 2, 1, &compressed)
            .unwrap();

        // Raw-format BGRX, padding byte = 0: red=[0,0,255,0], green=[0,255,0,0].
        assert_eq!(bgrx, vec![0, 0, 255, 0, 0, 255, 0, 0]);
    }

    #[test]
    fn solid_tile_fills_whole_tile_with_one_colour() {
        // 3x2 rectangle, single solid tile of blue.
        let mut tile = vec![1u8]; // sub-encoding 1 = solid
        tile.extend_from_slice(&cpixel(0, 0, 255)); // blue
        let compressed = zlib_one(&tile);

        let mut dec = ZrleDecoder::new();
        let bgrx = dec
            .decode_rect(PixelFormat::rgba32_le(), 3, 2, &compressed)
            .unwrap();

        // Every one of the 6 pixels is blue BGRX = [255,0,0,0].
        let mut expected = Vec::new();
        for _ in 0..6 {
            expected.extend_from_slice(&[255, 0, 0, 0]);
        }
        assert_eq!(bgrx, expected);
    }

    #[test]
    fn packed_palette_tile_respects_per_row_byte_padding() {
        // palette size 2 => 1 bit/index. 3x2 tile: each 3-bit row pads
        // out to a full byte, so wrong padding handling corrupts row 1.
        // palette: index 0 = red, index 1 = green.
        let red = cpixel(255, 0, 0);
        let green = cpixel(0, 255, 0);
        // sub-encoding 2 = packed palette of size 2, then one byte per row.
        let mut tile = vec![2u8];
        tile.extend_from_slice(&red);
        tile.extend_from_slice(&green);
        // Row 0 indices [0,1,0] -> 0b010 << 5 = 0x40.
        tile.push(0x40);
        // Row 1 indices [1,1,1] -> 0b111 << 5 = 0xE0.
        tile.push(0xE0);
        let compressed = zlib_one(&tile);

        let mut dec = ZrleDecoder::new();
        let bgrx = dec
            .decode_rect(PixelFormat::rgba32_le(), 3, 2, &compressed)
            .unwrap();

        let r = [0, 0, 255, 0];
        let g = [0, 255, 0, 0];
        let mut expected = Vec::new();
        for px in [r, g, r, g, g, g] {
            expected.extend_from_slice(&px);
        }
        assert_eq!(bgrx, expected);
    }

    #[test]
    fn plain_rle_runs_flatten_across_tile_rows() {
        // 2x2 tile: red run of length 3 then green run of length 1,
        // which must wrap from row 0 into row 1.
        let mut tile = vec![128u8]; // sub-encoding 128 = plain RLE
        tile.extend_from_slice(&cpixel(255, 0, 0)); // red
        tile.push(0x02); // run length 3 (= 2 + 1)
        tile.extend_from_slice(&cpixel(0, 255, 0)); // green
        tile.push(0x00); // run length 1 (= 0 + 1)
        let compressed = zlib_one(&tile);

        let mut dec = ZrleDecoder::new();
        let bgrx = dec
            .decode_rect(PixelFormat::rgba32_le(), 2, 2, &compressed)
            .unwrap();

        let r = [0, 0, 255, 0];
        let g = [0, 255, 0, 0];
        let mut expected = Vec::new();
        for px in [r, r, r, g] {
            expected.extend_from_slice(&px);
        }
        assert_eq!(bgrx, expected);
    }

    #[test]
    fn rle_run_length_sums_255_chained_bytes() {
        // One 64x8 tile (512 px). Run of A length 300 = 255 + 44 + 1
        // (bytes [255, 44]) exercises the 255-summation; run of B fills
        // the remaining 212 = 211 + 1 (byte [211]).
        let mut tile = vec![128u8];
        tile.extend_from_slice(&cpixel(1, 2, 3)); // A
        tile.push(255);
        tile.push(44);
        tile.extend_from_slice(&cpixel(9, 8, 7)); // B
        tile.push(211);
        let compressed = zlib_one(&tile);

        let mut dec = ZrleDecoder::new();
        let bgrx = dec
            .decode_rect(PixelFormat::rgba32_le(), 64, 8, &compressed)
            .unwrap();

        let a = [3u8, 2, 1, 0]; // BGRX of (R=1,G=2,B=3)
        let b = [7u8, 8, 9, 0]; // BGRX of (R=9,G=8,B=7)
        assert_eq!(bgrx.len(), 512 * 4);
        for (i, px) in bgrx.chunks_exact(4).enumerate() {
            assert_eq!(px, if i < 300 { a } else { b }, "pixel {i}");
        }
    }

    #[test]
    fn palette_rle_handles_runs_and_single_pixels() {
        // 2x2 tile, palette size 2 => sub-encoding 130. Index byte with
        // top bit set starts a run (length byte follows); without it, a
        // single pixel.
        let mut tile = vec![130u8]; // palette RLE, size 2
        tile.extend_from_slice(&cpixel(255, 0, 0)); // palette[0] red
        tile.extend_from_slice(&cpixel(0, 255, 0)); // palette[1] green
        tile.push(0x80); // index 0, MSB set => run
        tile.push(0x02); // run length 3
        tile.push(0x01); // index 1, MSB clear => single pixel
        let compressed = zlib_one(&tile);

        let mut dec = ZrleDecoder::new();
        let bgrx = dec
            .decode_rect(PixelFormat::rgba32_le(), 2, 2, &compressed)
            .unwrap();

        let r = [0, 0, 255, 0];
        let g = [0, 255, 0, 0];
        let mut expected = Vec::new();
        for px in [r, r, r, g] {
            expected.extend_from_slice(&px);
        }
        assert_eq!(bgrx, expected);
    }

    #[test]
    fn multiple_tiles_are_placed_at_correct_origins() {
        // 100x70 rect spans 4 tiles: (64x64, 36x64, 64x6, 36x6),
        // emitted row-major. Give each a distinct solid colour and
        // check a sample pixel in each quadrant.
        let colours = [(10, 0, 0), (0, 20, 0), (0, 0, 30), (40, 50, 60)];
        let mut stream = Vec::new();
        for (r, g, b) in colours {
            stream.push(1u8); // solid
            stream.extend_from_slice(&cpixel(r, g, b));
        }
        let compressed = zlib_one(&stream);

        let mut dec = ZrleDecoder::new();
        let bgrx = dec
            .decode_rect(PixelFormat::rgba32_le(), 100, 70, &compressed)
            .unwrap();

        let at = |x: usize, y: usize| {
            let i = (y * 100 + x) * 4;
            [bgrx[i], bgrx[i + 1], bgrx[i + 2], bgrx[i + 3]]
        };
        // BGRX of each colour (padding 0).
        assert_eq!(at(0, 0), [0, 0, 10, 0], "top-left tile");
        assert_eq!(at(99, 0), [0, 20, 0, 0], "top-right tile");
        assert_eq!(at(0, 69), [30, 0, 0, 0], "bottom-left tile");
        assert_eq!(at(99, 69), [60, 50, 40, 0], "bottom-right tile");
        // A pixel straddling the vertical tile seam stays in its tile.
        assert_eq!(at(63, 0), [0, 0, 10, 0], "last col of left tile");
        assert_eq!(at(64, 0), [0, 20, 0, 0], "first col of right tile");
    }

    #[test]
    fn persistent_zlib_stream_decodes_consecutive_rectangles() {
        // Two rectangles share one zlib stream (sync-flushed between
        // them), as a real server sends them. The decoder must keep the
        // inflate context alive across both.
        let mut stream = ZlibStream::new();
        let rect1 = {
            let mut t = vec![1u8];
            t.extend_from_slice(&cpixel(11, 22, 33));
            t
        };
        let rect2 = {
            let mut t = vec![1u8];
            t.extend_from_slice(&cpixel(44, 55, 66));
            t
        };
        let blob1 = stream.push(&rect1);
        let blob2 = stream.push(&rect2);

        let mut dec = ZrleDecoder::new();
        let out1 = dec
            .decode_rect(PixelFormat::rgba32_le(), 1, 1, &blob1)
            .unwrap();
        let out2 = dec
            .decode_rect(PixelFormat::rgba32_le(), 1, 1, &blob2)
            .unwrap();

        assert_eq!(out1, vec![33, 22, 11, 0]);
        assert_eq!(out2, vec![66, 55, 44, 0]);
    }

    #[test]
    fn zrle_output_is_pixel_identical_to_raw_path() {
        // Build a deterministic 70x65 image, push it through both the
        // Raw path and a ZRLE raw-tile path, and assert the resulting
        // framebuffers are byte-for-byte identical.
        use crate::framebuffer::Framebuffer;

        let (w, h) = (70u32, 65u32);
        let pixel = |x: u32, y: u32| {
            let r = (x.wrapping_mul(7).wrapping_add(y)) as u8;
            let g = (y.wrapping_mul(3).wrapping_add(x.wrapping_mul(5))) as u8;
            let b = (x ^ y) as u8;
            (r, g, b)
        };

        // Raw BGRX buffer for the whole rect.
        let mut raw = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            for x in 0..w {
                let (r, g, b) = pixel(x, y);
                raw.extend_from_slice(&[b, g, r, 0]);
            }
        }
        let mut fb_raw = Framebuffer::new(w, h).unwrap();
        fb_raw.raw_rect(0, 0, w, h, &raw).unwrap();

        // ZRLE: emit each 64x64 tile as a raw sub-encoding tile.
        let mut stream = Vec::new();
        let mut ty = 0;
        while ty < h {
            let th = (h - ty).min(64);
            let mut tx = 0;
            while tx < w {
                let tw = (w - tx).min(64);
                stream.push(0u8); // raw tile
                for row in 0..th {
                    for col in 0..tw {
                        let (r, g, b) = pixel(tx + col, ty + row);
                        stream.extend_from_slice(&cpixel(r, g, b));
                    }
                }
                tx += 64;
            }
            ty += 64;
        }
        let compressed = zlib_one(&stream);

        let mut dec = ZrleDecoder::new();
        let bgrx = dec
            .decode_rect(PixelFormat::rgba32_le(), w, h, &compressed)
            .unwrap();
        let mut fb_zrle = Framebuffer::new(w, h).unwrap();
        fb_zrle.raw_rect(0, 0, w, h, &bgrx).unwrap();

        assert_eq!(fb_zrle.rgba(), fb_raw.rgba());
    }
}
