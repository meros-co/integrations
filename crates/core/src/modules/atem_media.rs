//! The data the switcher's file transfer carries, and the conversions a
//! consumer's files need on the way in and out.
//!
//! - Frames (stills and clip frames) travel as the switcher's own picture
//!   format: per pair of pixels two big-endian 32-bit words, each 10-bit
//!   alpha in bits 20 to 29, 10-bit chroma (Cb in the first word, Cr in the
//!   second) in bits 10 to 19 and 10-bit luma in bits 0 to 9. Blackmagic's
//!   ATEM Switchers SDK manual (section 4.2.1, `bmdSwitcherPixelFormat10BitYUVA`)
//!   gives the component order, a0 Cb y0 a1 Cr y1; Sofie
//!   `lib/converters/rgbaToYuv422.ts` and `yuv422ToRgba.ts` give the bit
//!   packing and the video-range scaling (luma and alpha 64 to 940, chroma
//!   centered on 512), with BT.709 from 720 lines up and BT.601 below.
//! - A frame on the wire is run-length encoded in 8-byte blocks: a block of
//!   `FE` bytes, then a big-endian u64 count, then the block repeated
//!   (Sofie `lib/converters/rle.ts`). Literal blocks never look like the
//!   marker: a pixel word's top two bits are always clear.
//! - Clip audio is 48 kHz 24-bit stereo, as a WAV file in (the ATEM user
//!   manual: the media pool takes WAV); on the wire big-endian samples, and
//!   on switchers from the Production Studio 4K on with every 4-byte word
//!   reversed (Sofie `lib/converters/wavAudio.ts`).
//!
//! Consumers pass PNG files (the ATEM user manual lists PNG among the media
//! pool's still formats), raw 8-bit RGBA, or the switcher's own uncompressed
//! words. PNG is read and written here with the DEFLATE of `miniz_oxide`.
//! Written from those documents and MIT-licensed references, never from GPL
//! code.

/// The 8-byte block that starts a run.
pub(super) const RLE_MARKER: u64 = 0xFEFE_FEFE_FEFE_FEFE;

/// A frame's width and height in a video mode (by its wire value, as in
/// `atem_settings::VIDEO_MODES`). SD sizes are Sofie's (`lib/videoMode.ts`):
/// 640 by 480 for 525-line modes and 720 by 576 for 625-line ones.
pub(super) fn frame_size(mode: u8) -> Option<(u32, u32)> {
    Some(match mode {
        0 | 2 => (640, 480),
        1 | 3 => (720, 576),
        4 | 5 => (1280, 720),
        6..=13 | 26 | 27 => (1920, 1080),
        14..=19 => (3840, 2160),
        20..=25 => (7680, 4320),
        _ => return None,
    })
}

/// Luma and chroma weights for a frame height: BT.709 for HD and up,
/// BT.601 for SD.
fn weights(height: u32) -> (f32, f32) {
    if height >= 720 {
        (0.2126, 0.0722)
    } else {
        (0.299, 0.114)
    }
}

fn level10(v: f32) -> u32 {
    v.round().clamp(0.0, 1023.0) as u32
}

/// 8-bit RGBA, row after row, to the switcher's 10-bit YUVA words. The two
/// pixels of a pair share the chroma of their average.
pub(super) fn rgba_to_yuva(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let (kr, kb) = weights(height);
    let kg = 1.0 - kr - kb;
    let pixels = (width * height) as usize;
    let mut out = Vec::with_capacity(pixels * 4);
    for pair in rgba[..pixels * 4].as_chunks::<8>().0 {
        let luma = |p: &[u8]| (kr * p[0] as f32 + kg * p[1] as f32 + kb * p[2] as f32) / 255.0;
        let (y1, y2) = (luma(&pair[..4]), luma(&pair[4..]));
        let r = (pair[0] as f32 + pair[4] as f32) / 510.0;
        let b = (pair[2] as f32 + pair[6] as f32) / 510.0;
        let y = (y1 + y2) / 2.0;
        let cb = level10(512.0 + 448.0 * (b - y) / (1.0 - kb));
        let cr = level10(512.0 + 448.0 * (r - y) / (1.0 - kr));
        let alpha = |a: u8| level10(64.0 + 876.0 * a as f32 / 255.0);
        let word = |a: u32, c: u32, y: f32| (a << 20) | (c << 10) | level10(64.0 + 876.0 * y);
        out.extend_from_slice(&word(alpha(pair[3]), cb, y1).to_be_bytes());
        out.extend_from_slice(&word(alpha(pair[7]), cr, y2).to_be_bytes());
    }
    out
}

fn byte(v: f32) -> u8 {
    (v * 255.0).round().clamp(0.0, 255.0) as u8
}

/// The switcher's 10-bit YUVA words to 8-bit RGBA.
pub(super) fn yuva_to_rgba(width: u32, height: u32, yuva: &[u8]) -> Vec<u8> {
    let (kr, kb) = weights(height);
    let kg = 1.0 - kr - kb;
    let pixels = (width * height) as usize;
    let mut out = Vec::with_capacity(pixels * 4);
    for pair in yuva[..pixels * 4].as_chunks::<8>().0 {
        let w1 = u32::from_be_bytes([pair[0], pair[1], pair[2], pair[3]]);
        let w2 = u32::from_be_bytes([pair[4], pair[5], pair[6], pair[7]]);
        let cb = (((w1 >> 10) & 0x3FF) as f32 - 512.0) / 448.0;
        let cr = (((w2 >> 10) & 0x3FF) as f32 - 512.0) / 448.0;
        for w in [w1, w2] {
            let y = ((w & 0x3FF) as f32 - 64.0) / 876.0;
            let r = y + cr * (1.0 - kr);
            let b = y + cb * (1.0 - kb);
            let g = (y - kr * r - kb * b) / kg;
            let a = (((w >> 20) & 0x3FF) as f32 - 64.0) / 876.0;
            out.extend_from_slice(&[byte(r), byte(g), byte(b), byte(a)]);
        }
    }
    out
}

/// Whether raw words could be the switcher's frame: every word's top two
/// bits clear, so no block can be taken for the run marker.
pub(super) fn plausible_yuva(data: &[u8]) -> bool {
    data.as_chunks::<4>().0.iter().all(|w| w[0] & 0xC0 == 0)
}

fn block(data: &[u8], at: usize) -> u64 {
    u64::from_be_bytes(data[at..at + 8].try_into().unwrap())
}

/// Run-length encode whole 8-byte blocks; runs of four or more become the
/// marker, a count and the block.
pub(super) fn rle_encode(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    let blocks = data.len() / 8;
    let mut i = 0;
    while i < blocks {
        let b = block(data, i * 8);
        let mut run = 1;
        while i + run < blocks && block(data, (i + run) * 8) == b {
            run += 1;
        }
        if run >= 4 {
            out.extend_from_slice(&RLE_MARKER.to_be_bytes());
            out.extend_from_slice(&(run as u64).to_be_bytes());
            out.extend_from_slice(&b.to_be_bytes());
        } else {
            out.extend_from_slice(&data[i * 8..(i + run) * 8]);
        }
        i += run;
    }
    out
}

/// Expand run-length encoded blocks to exactly `size` bytes.
pub(super) fn rle_decode(data: &[u8], size: usize) -> Result<Vec<u8>, String> {
    if !data.len().is_multiple_of(8) {
        return Err(format!("{} bytes is not whole 8-byte blocks", data.len()));
    }
    let mut out = Vec::with_capacity(size);
    let mut at = 0;
    while at < data.len() {
        let b = block(data, at);
        if b == RLE_MARKER {
            if at + 24 > data.len() {
                return Err("a run is cut short".into());
            }
            let count = block(data, at + 8) as usize;
            if count > (size - out.len()) / 8 {
                return Err("a run goes past the frame".into());
            }
            let value = data[at + 16..at + 24].to_vec();
            for _ in 0..count {
                out.extend_from_slice(&value);
            }
            at += 24;
        } else {
            if out.len() + 8 > size {
                return Err("more data than the frame holds".into());
            }
            out.extend_from_slice(&data[at..at + 8]);
            at += 8;
        }
    }
    if out.len() != size {
        return Err(format!("{} bytes where the frame is {size}", out.len()));
    }
    Ok(out)
}

/// Where the chunk starting at `from` ends, at most `size` bytes on: never
/// inside a run (marker, count, block), as Sofie
/// `dataTransfer/dataTransferUploadBuffer.ts:154-170` keeps it. `size` is a
/// multiple of 8.
pub(super) fn chunk_end(data: &[u8], from: usize, size: usize, rle: bool) -> usize {
    let end = from + size;
    if end >= data.len() {
        return data.len();
    }
    if rle && end >= from + 16 {
        if block(data, end - 8) == RLE_MARKER {
            return end - 8;
        }
        if block(data, end - 16) == RLE_MARKER {
            return end - 16;
        }
    }
    end
}

/// CRC-32 (ISO 3309, as PNG uses it).
fn crc32(parts: &[&[u8]]) -> u32 {
    let mut table = [0u32; 256];
    for (n, entry) in table.iter_mut().enumerate() {
        let mut c = n as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
        }
        *entry = c;
    }
    let mut c = 0xFFFF_FFFFu32;
    for part in parts {
        for &b in *part {
            c = table[((c ^ b as u32) & 0xFF) as usize] ^ (c >> 8);
        }
    }
    c ^ 0xFFFF_FFFF
}

pub(super) const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// The widest and tallest picture read: an 8K frame fits.
const PNG_MAX_SIDE: u32 = 8192;

fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let p = a as i16 + b as i16 - c as i16;
    let (pa, pb, pc) = (
        (p - a as i16).abs(),
        (p - b as i16).abs(),
        (p - c as i16).abs(),
    );
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

/// Undo the PNG row filters of one (sub)image in place: `rows` of
/// `stride` bytes, each after its filter type byte.
fn unfilter(raw: &[u8], rows: usize, stride: usize, bpp: usize) -> Result<Vec<u8>, String> {
    let mut out = vec![0u8; rows * stride];
    for y in 0..rows {
        let line = &raw[y * (stride + 1)..(y + 1) * (stride + 1)];
        let filter = line[0];
        for x in 0..stride {
            let a = if x >= bpp {
                out[y * stride + x - bpp]
            } else {
                0
            };
            let b = if y > 0 { out[(y - 1) * stride + x] } else { 0 };
            let c = if x >= bpp && y > 0 {
                out[(y - 1) * stride + x - bpp]
            } else {
                0
            };
            let v = line[1 + x];
            out[y * stride + x] = match filter {
                0 => v,
                1 => v.wrapping_add(a),
                2 => v.wrapping_add(b),
                3 => v.wrapping_add(((a as u16 + b as u16) / 2) as u8),
                4 => v.wrapping_add(paeth(a, b, c)),
                f => return Err(format!("unknown PNG row filter {f}")),
            };
        }
    }
    Ok(out)
}

struct PngHeader {
    width: u32,
    height: u32,
    depth: u8,
    color: u8,
    palette: Vec<[u8; 3]>,
    /// Palette alpha (tRNS for a palette image).
    palette_alpha: Vec<u8>,
    /// The sample that is transparent (tRNS for gray or RGB), at the
    /// image's own depth.
    key: Option<[u16; 3]>,
}

impl PngHeader {
    fn channels(&self) -> usize {
        match self.color {
            0 | 3 => 1,
            2 => 3,
            4 => 2,
            _ => 4,
        }
    }

    /// Sample `i` of a row unpacked, at the image's depth.
    fn sample(&self, row: &[u8], i: usize) -> u16 {
        match self.depth {
            16 => u16::from_be_bytes([row[i * 2], row[i * 2 + 1]]),
            8 => row[i] as u16,
            d => {
                let bit = i * d as usize;
                let shift = 8 - d as usize - bit % 8;
                ((row[bit / 8] >> shift) & ((1u8 << d) - 1)) as u16
            }
        }
    }

    fn to8(&self, v: u16) -> u8 {
        match self.depth {
            16 => (v >> 8) as u8,
            8 => v as u8,
            d => (v as u32 * 255 / ((1u32 << d) - 1)) as u8,
        }
    }

    /// One pixel of a row as RGBA.
    fn pixel(&self, row: &[u8], x: usize) -> Result<[u8; 4], String> {
        let n = self.channels();
        let s = |c: usize| self.sample(row, x * n + c);
        Ok(match self.color {
            0 => {
                let v = s(0);
                let a = if self.key.is_some_and(|k| k[0] == v) {
                    0
                } else {
                    255
                };
                let g = self.to8(v);
                [g, g, g, a]
            }
            2 => {
                let (r, g, b) = (s(0), s(1), s(2));
                let a = if self.key == Some([r, g, b]) { 0 } else { 255 };
                [self.to8(r), self.to8(g), self.to8(b), a]
            }
            3 => {
                let i = s(0) as usize;
                let [r, g, b] = *self
                    .palette
                    .get(i)
                    .ok_or_else(|| format!("palette index {i} past the palette"))?;
                [r, g, b, self.palette_alpha.get(i).copied().unwrap_or(255)]
            }
            4 => {
                let g = self.to8(s(0));
                [g, g, g, self.to8(s(1))]
            }
            _ => [
                self.to8(s(0)),
                self.to8(s(1)),
                self.to8(s(2)),
                self.to8(s(3)),
            ],
        })
    }
}

/// Read a PNG (any color type and depth the standard allows, interlaced
/// or not) as 8-bit RGBA, with its width and height.
pub(super) fn png_decode(data: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    if !data.starts_with(&PNG_SIGNATURE) {
        return Err("not a PNG file".into());
    }
    let mut at = 8;
    let mut header: Option<PngHeader> = None;
    let mut interlaced = false;
    let mut compressed = Vec::new();
    loop {
        if at + 12 > data.len() {
            return Err("the PNG ends before its IEND".into());
        }
        let len = u32::from_be_bytes(data[at..at + 4].try_into().unwrap()) as usize;
        let kind = &data[at + 4..at + 8];
        if at + 12 + len > data.len() {
            return Err("a PNG chunk runs past the file".into());
        }
        let body = &data[at + 8..at + 8 + len];
        let crc = u32::from_be_bytes(data[at + 8 + len..at + 12 + len].try_into().unwrap());
        if crc32(&[kind, body]) != crc {
            return Err(format!(
                "the PNG's {} chunk fails its CRC",
                String::from_utf8_lossy(kind)
            ));
        }
        at += 12 + len;
        match kind {
            b"IHDR" => {
                if body.len() != 13 {
                    return Err("a PNG IHDR is 13 bytes".into());
                }
                let width = u32::from_be_bytes(body[0..4].try_into().unwrap());
                let height = u32::from_be_bytes(body[4..8].try_into().unwrap());
                let (depth, color) = (body[8], body[9]);
                let valid = match color {
                    0 => [1, 2, 4, 8, 16].contains(&depth),
                    3 => [1, 2, 4, 8].contains(&depth),
                    2 | 4 | 6 => [8, 16].contains(&depth),
                    _ => false,
                };
                if !valid || body[10] != 0 || body[11] != 0 || body[12] > 1 {
                    return Err(format!(
                        "PNG color type {color} at depth {depth} is not a valid PNG"
                    ));
                }
                if width == 0 || height == 0 || width > PNG_MAX_SIDE || height > PNG_MAX_SIDE {
                    return Err(format!("a {width} by {height} PNG is too large or empty"));
                }
                interlaced = body[12] == 1;
                header = Some(PngHeader {
                    width,
                    height,
                    depth,
                    color,
                    palette: Vec::new(),
                    palette_alpha: Vec::new(),
                    key: None,
                });
            }
            b"PLTE" => {
                let h = header.as_mut().ok_or("PLTE before IHDR")?;
                h.palette = body
                    .as_chunks::<3>()
                    .0
                    .iter()
                    .map(|c| [c[0], c[1], c[2]])
                    .collect();
            }
            b"tRNS" => {
                let h = header.as_mut().ok_or("tRNS before IHDR")?;
                let word = |i: usize| -> u16 {
                    body.get(i * 2..i * 2 + 2)
                        .map(|w| u16::from_be_bytes([w[0], w[1]]))
                        .unwrap_or(0)
                };
                match h.color {
                    3 => h.palette_alpha = body.to_vec(),
                    0 => h.key = Some([word(0), 0, 0]),
                    2 => h.key = Some([word(0), word(1), word(2)]),
                    _ => {}
                }
            }
            b"IDAT" => compressed.extend_from_slice(body),
            b"IEND" => break,
            _ => {}
        }
    }
    let h = header.ok_or("the PNG has no IHDR")?;
    if h.color == 3 && h.palette.is_empty() {
        return Err("a palette PNG without its palette".into());
    }
    let bits = h.channels() * h.depth as usize;
    let bpp = bits.div_ceil(8);
    let passes: Vec<(usize, usize, usize, usize)> = if interlaced {
        vec![
            (0, 0, 8, 8),
            (4, 0, 8, 8),
            (0, 4, 4, 8),
            (2, 0, 4, 4),
            (0, 2, 2, 4),
            (1, 0, 2, 2),
            (0, 1, 1, 2),
        ]
    } else {
        vec![(0, 0, 1, 1)]
    };
    let (w, ht) = (h.width as usize, h.height as usize);
    let sub = |(x0, y0, dx, dy): (usize, usize, usize, usize)| {
        let sw = (w + dx - 1 - x0.min(w)) / dx;
        let sh = (ht + dy - 1 - y0.min(ht)) / dy;
        if x0 >= w || y0 >= ht {
            (0, 0)
        } else {
            (sw, sh)
        }
    };
    let expected: usize = passes
        .iter()
        .map(|p| {
            let (sw, sh) = sub(*p);
            if sw == 0 || sh == 0 {
                0
            } else {
                sh * (1 + (sw * bits).div_ceil(8))
            }
        })
        .sum();
    let raw = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&compressed, expected)
        .map_err(|e| format!("the PNG's image data does not inflate: {e:?}"))?;
    if raw.len() != expected {
        return Err(format!(
            "the PNG's image data is {} bytes, not {expected}",
            raw.len()
        ));
    }
    let mut rgba = vec![0u8; w * ht * 4];
    let mut at = 0;
    for p in passes {
        let (sw, sh) = sub(p);
        if sw == 0 || sh == 0 {
            continue;
        }
        let stride = (sw * bits).div_ceil(8);
        let len = sh * (stride + 1);
        let rows = unfilter(&raw[at..at + len], sh, stride, bpp)?;
        at += len;
        let (x0, y0, dx, dy) = p;
        for sy in 0..sh {
            let row = &rows[sy * stride..(sy + 1) * stride];
            for sx in 0..sw {
                let (x, y) = (x0 + sx * dx, y0 + sy * dy);
                let px = h.pixel(row, sx)?;
                rgba[(y * w + x) * 4..(y * w + x) * 4 + 4].copy_from_slice(&px);
            }
        }
    }
    Ok((h.width, h.height, rgba))
}

fn png_chunk(out: &mut Vec<u8>, kind: &[u8; 4], body: &[u8]) {
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(body);
    out.extend_from_slice(&crc32(&[kind, body]).to_be_bytes());
}

/// Write 8-bit RGBA as a PNG: each row filtered with Sub, DEFLATE at a
/// fast level.
pub(super) fn png_encode(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let stride = width as usize * 4;
    let mut raw = Vec::with_capacity((stride + 1) * height as usize);
    for row in rgba.chunks_exact(stride).take(height as usize) {
        raw.push(1);
        for x in 0..stride {
            let left = if x >= 4 { row[x - 4] } else { 0 };
            raw.push(row[x].wrapping_sub(left));
        }
    }
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    let mut out = PNG_SIGNATURE.to_vec();
    png_chunk(&mut out, b"IHDR", &ihdr);
    png_chunk(
        &mut out,
        b"IDAT",
        &miniz_oxide::deflate::compress_to_vec_zlib(&raw, 3),
    );
    png_chunk(&mut out, b"IEND", &[]);
    out
}

/// A 48 kHz 24-bit stereo PCM WAV file as the switcher takes clip audio:
/// big-endian samples, and with `reversed_words` every 4-byte word reversed
/// (padded with one silent sample pair when the pairs are odd in number, so
/// the words are whole).
pub(super) fn wav_to_clip_audio(data: &[u8], reversed_words: bool) -> Result<Vec<u8>, String> {
    if data.len() < 12 || &data[0..4] != b"RIFF" || &data[8..12] != b"WAVE" {
        return Err("not a WAV file".into());
    }
    let mut at = 12;
    let mut format: Option<(u16, u16, u32, u16)> = None;
    let mut samples: Option<&[u8]> = None;
    while at + 8 <= data.len() {
        let kind = &data[at..at + 4];
        let len = u32::from_le_bytes(data[at + 4..at + 8].try_into().unwrap()) as usize;
        let body = &data[at + 8..(at + 8 + len).min(data.len())];
        match kind {
            b"fmt " if body.len() >= 16 => {
                let mut tag = u16::from_le_bytes([body[0], body[1]]);
                // WAVE_FORMAT_EXTENSIBLE names the real format in its
                // sub-format GUID's first two bytes.
                if tag == 0xFFFE && body.len() >= 26 {
                    tag = u16::from_le_bytes([body[24], body[25]]);
                }
                format = Some((
                    tag,
                    u16::from_le_bytes([body[2], body[3]]),
                    u32::from_le_bytes(body[4..8].try_into().unwrap()),
                    u16::from_le_bytes([body[14], body[15]]),
                ));
            }
            b"data" => samples = Some(body),
            _ => {}
        }
        at += 8 + len + (len & 1);
    }
    let (tag, channels, rate, bits) = format.ok_or("the WAV file has no fmt chunk")?;
    if tag != 1 || channels != 2 || rate != 48_000 || bits != 24 {
        return Err(format!(
            "the WAV file is {channels} channels of {bits}-bit format {tag} at {rate} Hz; \
             clip audio is 48 kHz 24-bit stereo PCM"
        ));
    }
    let samples = samples.ok_or("the WAV file has no data chunk")?;
    let mut out: Vec<u8> = samples
        .as_chunks::<3>()
        .0
        .iter()
        .flat_map(|s| [s[2], s[1], s[0]])
        .collect();
    out.truncate(out.len() / 6 * 6);
    if out.is_empty() {
        return Err("the WAV file holds no samples".into());
    }
    if reversed_words {
        if !out.len().is_multiple_of(4) {
            out.extend_from_slice(&[0; 6]);
        }
        for word in out.as_chunks_mut::<4>().0 {
            word.reverse();
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_sizes_follow_the_video_mode() {
        assert_eq!(frame_size(0), Some((640, 480)));
        assert_eq!(frame_size(1), Some((720, 576)));
        assert_eq!(frame_size(4), Some((1280, 720)));
        assert_eq!(frame_size(12), Some((1920, 1080)));
        assert_eq!(frame_size(27), Some((1920, 1080)));
        assert_eq!(frame_size(18), Some((3840, 2160)));
        assert_eq!(frame_size(24), Some((7680, 4320)));
        assert_eq!(frame_size(28), None);
    }

    #[test]
    fn colors_convert_to_video_levels_and_back() {
        // White opaque, black transparent: luma 940 and 64, chroma 512,
        // alpha 940 and 64.
        let rgba = [255, 255, 255, 255, 0, 0, 0, 0];
        let yuva = rgba_to_yuva(2, 1, &rgba);
        let w1 = u32::from_be_bytes(yuva[0..4].try_into().unwrap());
        let w2 = u32::from_be_bytes(yuva[4..8].try_into().unwrap());
        assert_eq!(w1 & 0x3FF, 940);
        assert_eq!((w1 >> 20) & 0x3FF, 940);
        assert_eq!(w2 & 0x3FF, 64);
        assert_eq!((w2 >> 20) & 0x3FF, 64);
        // The pair's average is mid gray: no chroma.
        assert_eq!((w1 >> 10) & 0x3FF, 512);
        assert_eq!((w2 >> 10) & 0x3FF, 512);
        assert!(plausible_yuva(&yuva));

        // A flat color survives the round trip within a level.
        for (height, color) in [(720, [200u8, 40, 90, 128]), (576, [10, 220, 30, 255])] {
            let rgba: Vec<u8> = color
                .iter()
                .copied()
                .cycle()
                .take(2 * height as usize * 4)
                .collect();
            let back = yuva_to_rgba(2, height, &rgba_to_yuva(2, height, &rgba));
            for (a, b) in back.iter().zip(&rgba) {
                assert!((*a as i16 - *b as i16).abs() <= 1, "{back:?} from {rgba:?}");
            }
        }
    }

    #[test]
    fn runs_of_four_blocks_or_more_are_encoded() {
        let a = [1u8, 2, 3, 4, 5, 6, 7, 8];
        let b = [9u8; 8];
        let mut data = Vec::new();
        for _ in 0..3 {
            data.extend_from_slice(&a);
        }
        for _ in 0..5 {
            data.extend_from_slice(&b);
        }
        data.extend_from_slice(&a);
        let rle = rle_encode(&data);
        let mut expected = Vec::new();
        for _ in 0..3 {
            expected.extend_from_slice(&a);
        }
        expected.extend_from_slice(&RLE_MARKER.to_be_bytes());
        expected.extend_from_slice(&5u64.to_be_bytes());
        expected.extend_from_slice(&b);
        expected.extend_from_slice(&a);
        assert_eq!(rle, expected);
        assert_eq!(rle_decode(&rle, data.len()).unwrap(), data);
        assert!(rle_decode(&rle, data.len() - 8).is_err());
        assert!(rle_decode(&rle[..rle.len() - 12], data.len()).is_err());
    }

    #[test]
    fn chunks_never_split_a_run() {
        let mut data = vec![0u8; 16];
        data.extend_from_slice(&RLE_MARKER.to_be_bytes());
        data.extend_from_slice(&7u64.to_be_bytes());
        data.extend_from_slice(&[1; 8]);
        data.extend_from_slice(&[2; 16]);
        // Marker at 16: a 24-byte chunk ends before it, a 32-byte one too.
        assert_eq!(chunk_end(&data, 0, 24, true), 16);
        assert_eq!(chunk_end(&data, 0, 32, true), 16);
        assert_eq!(chunk_end(&data, 0, 40, true), 40);
        assert_eq!(chunk_end(&data, 0, 1000, true), data.len());
        assert_eq!(chunk_end(&data, 0, 24, false), 24);
    }

    #[test]
    fn png_round_trips() {
        let rgba: Vec<u8> = (0..6 * 3 * 4).map(|i| (i * 7) as u8).collect();
        let png = png_encode(6, 3, &rgba);
        assert!(png.starts_with(&PNG_SIGNATURE));
        assert_eq!(png_decode(&png).unwrap(), (6, 3, rgba));
    }

    fn png(ihdr: [u8; 13], extra: &[(&[u8; 4], Vec<u8>)], raw: &[u8]) -> Vec<u8> {
        let mut out = PNG_SIGNATURE.to_vec();
        png_chunk(&mut out, b"IHDR", &ihdr);
        for (kind, body) in extra {
            png_chunk(&mut out, kind, body);
        }
        png_chunk(
            &mut out,
            b"IDAT",
            &miniz_oxide::deflate::compress_to_vec_zlib(raw, 6),
        );
        png_chunk(&mut out, b"IEND", &[]);
        out
    }

    fn ihdr(w: u32, h: u32, depth: u8, color: u8, interlace: u8) -> [u8; 13] {
        let mut b = [0u8; 13];
        b[0..4].copy_from_slice(&w.to_be_bytes());
        b[4..8].copy_from_slice(&h.to_be_bytes());
        b[8] = depth;
        b[9] = color;
        b[12] = interlace;
        b
    }

    #[test]
    fn png_palettes_gray_depths_filters_and_interlacing_are_read() {
        // A 2-bit palette image, two rows, Up filter on the second row, the
        // second color half transparent.
        let file = png(
            ihdr(3, 2, 2, 3, 0),
            &[
                (b"PLTE", vec![255, 0, 0, 0, 255, 0, 0, 0, 255]),
                (b"tRNS", vec![255, 128]),
            ],
            &[0, 0b0001_1000, 2, 0b0100_0000],
        );
        let (w, h, rgba) = png_decode(&file).unwrap();
        assert_eq!((w, h), (3, 2));
        assert_eq!(
            rgba,
            [
                255, 0, 0, 255, 0, 255, 0, 128, 0, 0, 255, 255, //
                0, 255, 0, 128, 0, 255, 0, 128, 0, 0, 255, 255,
            ]
        );

        // 16-bit gray with alpha, Paeth and Average filters.
        let file = png(
            ihdr(1, 2, 16, 4, 0),
            &[],
            &[4, 0x80, 0x00, 0xFF, 0xFF, 3, 0x00, 0x00, 0x00, 0x00],
        );
        let (_, _, rgba) = png_decode(&file).unwrap();
        // Average of nothing to the left and the row above: halves.
        assert_eq!(rgba, [128, 128, 128, 255, 64, 64, 64, 127]);

        // An interlaced 3x3 RGB image: seven passes, some empty.
        let mut raw = Vec::new();
        let px = |x: u8, y: u8| [x * 10, y * 10, 7];
        for (x0, y0, dx, dy) in [
            (0, 0, 8, 8),
            (0, 4, 4, 8),
            (2, 0, 4, 4),
            (0, 2, 2, 4),
            (1, 0, 2, 2),
            (0, 1, 1, 2),
        ] {
            let xs: Vec<u8> = (x0..3).step_by(dx).collect();
            for y in (y0..3).step_by(dy) {
                raw.push(0);
                for &x in &xs {
                    raw.extend_from_slice(&px(x, y));
                }
            }
        }
        let file = png(ihdr(3, 3, 8, 2, 1), &[], &raw);
        let (_, _, rgba) = png_decode(&file).unwrap();
        for y in 0..3u8 {
            for x in 0..3u8 {
                let at = (y as usize * 3 + x as usize) * 4;
                let [r, g, b] = px(x, y);
                assert_eq!(rgba[at..at + 4], [r, g, b, 255], "pixel {x},{y}");
            }
        }

        // A damaged chunk is refused.
        let mut bad = png_encode(2, 2, &[0; 16]);
        bad[20] ^= 1;
        assert!(png_decode(&bad).unwrap_err().contains("CRC"));
        assert!(png_decode(b"GIF89a").is_err());
    }

    fn wav(channels: u16, rate: u32, bits: u16, data: &[u8]) -> Vec<u8> {
        let mut fmt = Vec::new();
        fmt.extend_from_slice(&1u16.to_le_bytes());
        fmt.extend_from_slice(&channels.to_le_bytes());
        fmt.extend_from_slice(&rate.to_le_bytes());
        fmt.extend_from_slice(&(rate * 6).to_le_bytes());
        fmt.extend_from_slice(&6u16.to_le_bytes());
        fmt.extend_from_slice(&bits.to_le_bytes());
        let mut out = b"RIFF\0\0\0\0WAVE".to_vec();
        out.extend_from_slice(b"fmt ");
        out.extend_from_slice(&(fmt.len() as u32).to_le_bytes());
        out.extend_from_slice(&fmt);
        out.extend_from_slice(b"LIST\x01\0\0\0x\0");
        out.extend_from_slice(b"data");
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(data);
        out
    }

    #[test]
    fn wav_audio_becomes_big_endian_and_word_reversed() {
        let pair = [1, 2, 3, 4, 5, 6];
        let file = wav(2, 48_000, 24, &pair);
        assert_eq!(wav_to_clip_audio(&file, false).unwrap(), [3, 2, 1, 6, 5, 4]);
        // Odd pairs gain a silent pair, then every word is reversed.
        assert_eq!(
            wav_to_clip_audio(&file, true).unwrap(),
            [6, 1, 2, 3, 0, 0, 4, 5, 0, 0, 0, 0]
        );
        assert!(wav_to_clip_audio(&wav(1, 48_000, 24, &pair), false).is_err());
        assert!(wav_to_clip_audio(&wav(2, 44_100, 24, &pair), false).is_err());
        assert!(wav_to_clip_audio(&wav(2, 48_000, 16, &pair), false).is_err());
        assert!(wav_to_clip_audio(b"not a wav", false).is_err());
    }
}
