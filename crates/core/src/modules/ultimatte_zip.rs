//! The zip archives Ultimatte Smart Remote 4 and Software Control write with
//! Archive All and read with Restore (PKWARE APPNOTE 6.3: local headers, the
//! central directory and its end record; stored and deflated members only).
//!
//! The vendor app is strict about the archive's layout (bmdwire's
//! ultimattewire, MIT): directory entries `images/`, `quickfiles/` and
//! `presets/` of their own, Unix directory attributes on them, and the
//! members in the app's order. The writer takes the members in the order
//! given; the reader goes by the central directory, so an archive written
//! with data descriptors (sizes after the data) reads too.

use miniz_oxide::deflate::compress_to_vec;
use miniz_oxide::inflate::decompress_to_vec_with_limit;

const LOCAL: u32 = 0x0403_4b50;
const CENTRAL: u32 = 0x0201_4b50;
const END: u32 = 0x0605_4b50;
/// Made by Unix, APPNOTE 2.0, so the external attributes are Unix modes.
const MADE_BY: u16 = (3 << 8) | 20;
const NEEDED: u16 = 20;
const DIR_MODE: u32 = 0o40755;
const FILE_MODE: u32 = 0o100644;

/// One member to write: a directory (name ending in `/`, no data) or a file.
pub(crate) struct Member<'a> {
    pub name: &'a str,
    pub data: &'a [u8],
}

pub(crate) fn crc32(data: &[u8]) -> u32 {
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
    for &b in data {
        c = table[((c ^ b as u32) & 0xFF) as usize] ^ (c >> 8);
    }
    c ^ 0xFFFF_FFFF
}

/// MS-DOS time and date fields for a moment in Unix milliseconds (UTC).
pub(crate) fn dos_time(unix_millis: u64) -> (u16, u16) {
    let secs = unix_millis / 1000;
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    // Civil date from days since 1970-01-01 (H. Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    let year = year.clamp(1980, 2107);
    let time = (((rem / 3600) << 11) | (((rem % 3600) / 60) << 5) | ((rem % 60) / 2)) as u16;
    let date = (((year - 1980) << 9) | (month << 5) | day) as u16;
    (time, date)
}

/// Writes the members, in order, as one archive. Files are deflated unless
/// that would make them larger; directories are stored, empty.
pub(crate) fn write(members: &[Member], unix_millis: u64) -> Vec<u8> {
    let (time, date) = dos_time(unix_millis);
    let mut out = Vec::new();
    let mut central = Vec::new();
    for m in members {
        let dir = m.name.ends_with('/');
        let crc = crc32(m.data);
        let deflated = if dir || m.data.is_empty() {
            None
        } else {
            Some(compress_to_vec(m.data, 6)).filter(|d| d.len() < m.data.len())
        };
        let (method, body): (u16, &[u8]) = match &deflated {
            Some(d) => (8, d),
            None => (0, m.data),
        };
        let offset = out.len() as u32;
        let name = m.name.as_bytes();
        out.extend_from_slice(&LOCAL.to_le_bytes());
        out.extend_from_slice(&NEEDED.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes()); // flags
        out.extend_from_slice(&method.to_le_bytes());
        out.extend_from_slice(&time.to_le_bytes());
        out.extend_from_slice(&date.to_le_bytes());
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&(body.len() as u32).to_le_bytes());
        out.extend_from_slice(&(m.data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(name.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes()); // extra
        out.extend_from_slice(name);
        out.extend_from_slice(body);

        let mode = if dir { DIR_MODE } else { FILE_MODE };
        central.extend_from_slice(&CENTRAL.to_le_bytes());
        central.extend_from_slice(&MADE_BY.to_le_bytes());
        central.extend_from_slice(&NEEDED.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&method.to_le_bytes());
        central.extend_from_slice(&time.to_le_bytes());
        central.extend_from_slice(&date.to_le_bytes());
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&(body.len() as u32).to_le_bytes());
        central.extend_from_slice(&(m.data.len() as u32).to_le_bytes());
        central.extend_from_slice(&(name.len() as u16).to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes()); // extra
        central.extend_from_slice(&0u16.to_le_bytes()); // comment
        central.extend_from_slice(&0u16.to_le_bytes()); // disk
        central.extend_from_slice(&0u16.to_le_bytes()); // internal attributes
                                                        // MS-DOS directory bit beside the Unix mode, as Python's zipfile sets.
        let external = (mode << 16) | if dir { 0x10 } else { 0 };
        central.extend_from_slice(&external.to_le_bytes());
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name);
    }
    let start = out.len() as u32;
    let count = members.len() as u16;
    out.extend_from_slice(&central);
    out.extend_from_slice(&END.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&(central.len() as u32).to_le_bytes());
    out.extend_from_slice(&start.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

fn u16_at(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

/// Every file member of an archive (directories left out), in central
/// directory order, each read whole and its CRC checked. `limit` bounds the
/// total of the members' uncompressed sizes.
pub(crate) fn read(zip: &[u8], limit: usize) -> Result<Vec<(String, Vec<u8>)>, String> {
    let bad = |what: &str| format!("not a zip archive the unit's app writes: {what}");
    // The end record is the last 22 bytes, unless a comment follows it.
    let floor = zip.len().saturating_sub(22 + 0xFFFF);
    let end = (floor..=zip.len().saturating_sub(22))
        .rev()
        .find(|&at| u32_at(zip, at) == Some(END))
        .ok_or_else(|| bad("no end of central directory"))?;
    let count = u16_at(zip, end + 10).ok_or_else(|| bad("short end record"))? as usize;
    let mut at = u32_at(zip, end + 16).ok_or_else(|| bad("short end record"))? as usize;
    if count == 0xFFFF || at == 0xFFFF_FFFF {
        return Err(bad("zip64 is not read"));
    }
    let mut members = Vec::new();
    let mut total = 0usize;
    for _ in 0..count {
        if u32_at(zip, at) != Some(CENTRAL) {
            return Err(bad("broken central directory"));
        }
        let field = |off: usize| u16_at(zip, at + off).ok_or_else(|| bad("short entry"));
        let flags = field(8)?;
        let method = field(10)?;
        let crc = u32_at(zip, at + 16).ok_or_else(|| bad("short entry"))?;
        let packed = u32_at(zip, at + 20).ok_or_else(|| bad("short entry"))? as usize;
        let size = u32_at(zip, at + 24).ok_or_else(|| bad("short entry"))? as usize;
        let name_len = field(28)? as usize;
        let extra_len = field(30)? as usize;
        let comment_len = field(32)? as usize;
        let local = u32_at(zip, at + 42).ok_or_else(|| bad("short entry"))? as usize;
        let name = zip
            .get(at + 46..at + 46 + name_len)
            .ok_or_else(|| bad("short entry"))?;
        let name = String::from_utf8_lossy(name).into_owned();
        at += 46 + name_len + extra_len + comment_len;
        if name.ends_with('/') {
            continue;
        }
        if flags & 1 != 0 {
            return Err(format!("'{name}' is encrypted"));
        }
        if packed == 0xFFFF_FFFF || size == 0xFFFF_FFFF {
            return Err(bad("zip64 is not read"));
        }
        total = total.saturating_add(size);
        if total > limit {
            return Err(format!("the archive holds more than {limit} bytes"));
        }
        if u32_at(zip, local) != Some(LOCAL) {
            return Err(bad("broken local header"));
        }
        let skip = u16_at(zip, local + 26).ok_or_else(|| bad("short local header"))? as usize
            + u16_at(zip, local + 28).ok_or_else(|| bad("short local header"))? as usize;
        let start = local + 30 + skip;
        let body = zip
            .get(start..start + packed)
            .ok_or_else(|| format!("'{name}' is cut short"))?;
        let data = match method {
            0 => body.to_vec(),
            8 => decompress_to_vec_with_limit(body, size)
                .map_err(|_| format!("'{name}' does not inflate"))?,
            other => return Err(format!("'{name}' uses compression method {other}")),
        };
        if data.len() != size || crc32(&data) != crc {
            return Err(format!("'{name}' fails its CRC check"));
        }
        members.push((name, data));
    }
    Ok(members)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_matches_the_check_value() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn dos_time_of_a_known_moment() {
        // 2026-10-21 11:19:42 UTC.
        let (time, date) = dos_time(1_792_581_582_000);
        assert_eq!(date, ((2026 - 1980) << 9) | (10 << 5) | 21);
        assert_eq!(time, (11 << 11) | (19 << 5) | 21);
    }

    #[test]
    fn written_archives_read_back_without_their_directories() {
        let big = vec![7u8; 4000];
        let zip = write(
            &[
                Member {
                    name: "images/",
                    data: b"",
                },
                Member {
                    name: "GPISettings",
                    data: b"gpi",
                },
                Member {
                    name: "presets/",
                    data: b"",
                },
                Member {
                    name: "presets/Studio A",
                    data: &big,
                },
            ],
            0,
        );
        let members = read(&zip, 1 << 20).unwrap();
        assert_eq!(members.len(), 2);
        assert_eq!(members[0], ("GPISettings".into(), b"gpi".to_vec()));
        assert_eq!(members[1], ("presets/Studio A".into(), big));
        // The repetitive preset was deflated.
        assert!(zip.len() < 1000);
    }

    #[test]
    fn corrupt_and_oversized_archives_are_refused() {
        let zip = write(
            &[Member {
                name: "SavedSettings",
                data: &[1, 2, 3, 4],
            }],
            0,
        );
        assert!(read(&zip, 3).unwrap_err().contains("more than 3 bytes"));
        let mut broken = zip.clone();
        // The stored data's first byte.
        broken[30 + "SavedSettings".len()] ^= 0xFF;
        assert!(read(&broken, 100).unwrap_err().contains("CRC"));
        assert!(read(b"not a zip", 100).is_err());
    }
}
