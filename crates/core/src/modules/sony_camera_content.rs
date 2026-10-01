//! Reading what a Sony camera returns about images and clips: live view
//! frames, the content list of Remote Control with Transfer Mode, PTP
//! ObjectInfo in Content Transfer Mode, and the MediaProfile XML of video-only
//! cameras. Layouts follow the Camera Control PTP 3 and 2 References (GetObject,
//! GetObjectInfo, SDIO_GetContentInfoList and the ContentInfoList data format,
//! Acquiring Content from Video Only Models) and ISO 15740 for ObjectInfo.

use serde_json::{json, Map, Value};

use super::sony_camera_dataset::Reader;

/// A live view frame: the JPEG and, from PTP 3 cameras, the focus frame
/// information that goes with it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LiveFrame {
    pub jpeg: Vec<u8>,
    pub focal_frame_info: Option<Vec<u8>>,
}

fn slice_at(data: &[u8], offset: u32, size: u32) -> Option<&[u8]> {
    let start = offset as usize;
    let end = start.checked_add(size as usize)?;
    data.get(start..end)
}

/// The live view dataset: offset and size of the JPEG, then (PTP 3) offset
/// and size of the focal frame information. `None` when no image is in it,
/// which a camera answers when asked again too soon.
pub(crate) fn parse_live_view(data: &[u8]) -> Result<Option<LiveFrame>, String> {
    let mut r = Reader::new(data);
    let offset = r.u32()?;
    let size = r.u32()?;
    if size == 0 {
        return Ok(None);
    }
    let jpeg = slice_at(data, offset, size)
        .ok_or("the live view image lies outside the dataset")?
        .to_vec();
    let focal_frame_info = match (r.u32(), r.u32()) {
        (Ok(off), Ok(len)) if len > 0 && off as usize >= 16 => {
            slice_at(data, off, len).map(|s| s.to_vec())
        }
        _ => None,
    };
    Ok(Some(LiveFrame {
        jpeg,
        focal_frame_info,
    }))
}

/// Width and height from a JPEG's start-of-frame marker.
pub(crate) fn jpeg_size(jpeg: &[u8]) -> Option<(u16, u16)> {
    if jpeg.get(..2)? != [0xFF, 0xD8] {
        return None;
    }
    let mut i = 2;
    while i + 4 <= jpeg.len() {
        if jpeg[i] != 0xFF {
            i += 1;
            continue;
        }
        let marker = jpeg[i + 1];
        if marker == 0xFF {
            i += 1;
            continue;
        }
        if marker == 0xD8 || (0xD0..=0xD7).contains(&marker) || marker == 0x01 {
            i += 2;
            continue;
        }
        let length = u16::from_be_bytes([jpeg[i + 2], jpeg[i + 3]]) as usize;
        // Baseline, extended, progressive and lossless start-of-frame markers.
        if matches!(marker, 0xC0..=0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF) {
            let height = u16::from_be_bytes([*jpeg.get(i + 5)?, *jpeg.get(i + 6)?]);
            let width = u16::from_be_bytes([*jpeg.get(i + 7)?, *jpeg.get(i + 8)?]);
            return Some((width, height));
        }
        i += 2 + length;
    }
    None
}

/// The first complete JPEG (start to end of image) in a byte stream, for live
/// view served without one dataset per chunk.
pub(crate) fn find_jpeg(data: &[u8]) -> Option<&[u8]> {
    let start = data.windows(2).position(|w| w == [0xFF, 0xD8])?;
    let end = data[start..].windows(2).position(|w| w == [0xFF, 0xD9])?;
    Some(&data[start..start + end + 2])
}

pub(crate) fn file_format_name(format: u32) -> String {
    match format {
        0x3801 => "jpeg".into(),
        0xB101 => "raw".into(),
        0xB110 => "heif".into(),
        0x3008 => "wav".into(),
        0xB982 => "mp4".into(),
        0xBA82 => "xml".into(),
        0x3808 => "jfif".into(),
        0xB301 => "mpo".into(),
        0x3001 => "folder".into(),
        0 => "unspecified".into(),
        other => format!("0x{other:04X}"),
    }
}

fn content_type_name(v: u32) -> &'static str {
    match v {
        0x01 => "dcf",
        0x04 => "m4_style",
        0x08 => "xd_style",
        0x10 => "px_style",
        _ => "unspecified",
    }
}

fn group_name(v: u32) -> &'static str {
    match v {
        1 => "continuous",
        2 => "timelapse",
        3 => "bracket",
        _ => "none",
    }
}

fn hex_bytes(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn utf8_trimmed(b: &[u8]) -> String {
    let end = b.iter().position(|c| *c == 0).unwrap_or(b.len());
    String::from_utf8_lossy(&b[..end]).into_owned()
}

/// One file of a listed content, as needed to download it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ListedFile {
    pub id: String,
    pub size: u64,
}

/// A content list (SDIO_GetContentInfoList) as the command's value, and the
/// files in it with their sizes.
pub(crate) fn parse_content_info_list(data: &[u8]) -> Result<(Value, Vec<ListedFile>), String> {
    let mut r = Reader::new(data);
    let version = r.u16()?;
    r.u16()?;
    let list_time = r.u64()?;
    for _ in 0..(64 + 64) / 4 {
        r.u32()?;
    }
    let slot = r.u32()?;
    let count = r.u32()?;
    let mut items = Vec::new();
    let mut files_out = Vec::new();
    for _ in 0..count {
        // Removed contents leave zeros at the end of the list; stop there.
        let Ok(content_type) = r.u32() else { break };
        let parsed = (|| -> Result<Option<Value>, String> {
            let content_id = r.u32()?;
            let dir = r.u32()?;
            let file_number = r.u32()?;
            let group_type = r.u32()?;
            let group_id = r.u32()?;
            let representative = r.u32()? != 0;
            let created = r.u64()?;
            let modified = r.u64()?;
            let created_local = r.u64()?;
            let modified_local = r.u64()?;
            let rating = r.u32()? as i32;
            let protected = r.u32()? != 0;
            let dummy = r.u32()? != 0;
            let shotmarks = r.u32()?;
            if shotmarks as usize > r.remaining() {
                return Err("shot mark count does not fit".into());
            }
            let mut marked = false;
            for _ in 0..shotmarks {
                marked |= r.u8()? != 0;
            }
            let file_count = r.u32()?;
            let mut files = Vec::new();
            for _ in 0..file_count {
                let file_id = r.u16()?;
                r.u16()?;
                let path_len = r.u32()? as usize;
                if path_len > r.remaining() {
                    return Err("file path does not fit".into());
                }
                let path: Vec<u8> = (0..path_len).map(|_| r.u8()).collect::<Result<_, _>>()?;
                let format = r.u32()?;
                let size = r.u64()?;
                let umid: Vec<u8> = (0..32).map(|_| r.u8()).collect::<Result<_, _>>()?;
                let id = format!("c:{slot}:{content_id}:{file_id}");
                let mut file = json!({
                    "id": id,
                    "file_id": file_id,
                    "path": utf8_trimmed(&path),
                    "format": file_format_name(format),
                    "size": size,
                    "umid": hex_bytes(&umid),
                });
                if r.u32()? != 0 {
                    file["width"] = json!(r.u32()?);
                    file["height"] = json!(r.u32()?);
                }
                if r.u32()? != 0 {
                    let mut v = [0u32; 19];
                    for x in v.iter_mut() {
                        *x = r.u32()?;
                    }
                    file["video"] = json!({
                        "start_timecode": v[0],
                        "end_timecode": v[1],
                        "codec": match v[2] { 0x4832_3634 => "h264".to_string(), 0x4832_3635 => "h265".to_string(), 0 => "unspecified".to_string(), c => format!("0x{c:08X}") },
                        "proxy": v[3] != 0,
                        "width": v[5],
                        "height": v[6],
                        "frame_rate": v[10] as f64 / 1000.0,
                        "bit_rate_mbps": v[12],
                    });
                }
                if r.u32()? != 0 {
                    let mut a = [0u32; 4];
                    for x in a.iter_mut() {
                        *x = r.u32()?;
                    }
                    file["audio"] = json!({
                        "codec": a[0],
                        "bit_depth": a[1],
                        "sampling_rate": a[2],
                        "channels": a[3] >> 4,
                    });
                }
                files_out.push(ListedFile { id, size });
                files.push(file);
            }
            if content_type == 0 {
                return Ok(None);
            }
            Ok(Some(json!({
                "content_id": content_id,
                "type": content_type_name(content_type),
                "directory": dir,
                "file_number": file_number,
                "group": group_name(group_type),
                "group_id": group_id,
                "representative": representative,
                "created": created,
                "modified": modified,
                "created_local": created_local,
                "modified_local": modified_local,
                "rating": rating,
                "protected": protected,
                "placeholder": dummy,
                "shot_mark": marked,
                "files": files,
            })))
        })();
        match parsed {
            Ok(Some(item)) => items.push(item),
            Ok(None) => {}
            Err(e) if content_type == 0 => {
                let _ = e;
                break;
            }
            Err(e) => return Err(e),
        }
    }
    Ok((
        json!({
            "source": "content_info_list",
            "version": version,
            "slot": slot,
            "list_time": list_time,
            "items": items,
        }),
        files_out,
    ))
}

/// The parts of a PTP ObjectInfo this module reports.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ObjectInfo {
    pub storage: u32,
    pub format: u16,
    pub size: u32,
    pub width: u32,
    pub height: u32,
    pub filename: String,
    pub captured: String,
}

pub(crate) fn parse_object_info(data: &[u8]) -> Result<ObjectInfo, String> {
    let mut r = Reader::new(data);
    let storage = r.u32()?;
    let format = r.u16()?;
    r.u16()?; // protection
    let size = r.u32()?;
    r.u16()?; // thumb format
    r.u32()?; // thumb size
    r.u32()?; // thumb width
    r.u32()?; // thumb height
    let width = r.u32()?;
    let height = r.u32()?;
    r.u32()?; // bit depth
    r.u32()?; // parent
    r.u16()?; // association type
    r.u32()?; // association description
    r.u32()?; // sequence number
    let filename = r.string()?;
    let captured = r.string().unwrap_or_default();
    Ok(ObjectInfo {
        storage,
        format,
        size,
        width,
        height,
        filename,
        captured,
    })
}

impl ObjectInfo {
    pub(crate) fn to_json(&self, handle: u32) -> Value {
        json!({
            "id": format!("o:{handle}"),
            "name": self.filename,
            "format": file_format_name(self.format as u32),
            // 0xFFFFFFFF: four gigabytes or more; the download finds the end.
            "size": if self.size == u32::MAX { Value::Null } else { json!(self.size) },
            "width": self.width,
            "height": self.height,
            "captured": self.captured,
            "storage": format!("{:08X}", self.storage),
        })
    }
}

/// A PTP array of `u32` (storage IDs, object handles).
pub(crate) fn parse_u32_array(data: &[u8]) -> Result<Vec<u32>, String> {
    let mut r = Reader::new(data);
    let count = r.u32()? as usize;
    if count > r.remaining() / 4 {
        return Err(format!("array of {count} does not fit"));
    }
    (0..count).map(|_| r.u32()).collect()
}

/// The clips of a video-only camera's MediaProfile, with download ids
/// relative to the profile.
pub(crate) fn parse_media_profile(xml: &str, slot: u32) -> Result<Value, String> {
    let doc = roxmltree::Document::parse(xml.trim_start_matches('\u{feff}'))
        .map_err(|e| format!("unreadable MediaProfile: {e}"))?;
    let id = |uri: &str| format!("m:{slot}:{uri}");
    let attrs = |n: &roxmltree::Node, keys: &[(&str, &str)]| {
        let mut m = Map::new();
        for (attr, key) in keys {
            if let Some(v) = n.attribute(*attr) {
                m.insert(key.to_string(), json!(v));
            }
        }
        m
    };
    let mut items = Vec::new();
    for material in doc.descendants().filter(|n| n.has_tag_name("Material")) {
        let Some(uri) = material.attribute("uri") else {
            continue;
        };
        let mut item = attrs(
            &material,
            &[
                ("uri", "uri"),
                ("type", "type"),
                ("fps", "frame_rate"),
                ("aspectRatio", "aspect_ratio"),
                ("videoType", "video_type"),
                ("audioType", "audio_type"),
                ("umid", "umid"),
                ("status", "status"),
            ],
        );
        item.insert("id".into(), json!(id(uri)));
        if let Some(d) = material
            .attribute("dur")
            .and_then(|d| d.parse::<u64>().ok())
        {
            item.insert("duration_frames".into(), json!(d));
        }
        if let Some(c) = material.attribute("ch").and_then(|d| d.parse::<u64>().ok()) {
            item.insert("audio_channels".into(), json!(c));
        }
        if let Some(proxy) = material.children().find(|n| n.has_tag_name("Proxy")) {
            if let Some(p) = proxy.attribute("uri") {
                let mut m = attrs(
                    &proxy,
                    &[
                        ("uri", "uri"),
                        ("type", "type"),
                        ("videoType", "video_type"),
                    ],
                );
                m.insert("id".into(), json!(id(p)));
                item.insert("proxy".into(), Value::Object(m));
            }
        }
        let related: Vec<Value> = material
            .children()
            .filter(|n| n.has_tag_name("RelevantInfo"))
            .filter_map(|n| {
                let u = n.attribute("uri")?;
                Some(json!({"id": id(u), "uri": u, "type": n.attribute("type").unwrap_or("")}))
            })
            .collect();
        if !related.is_empty() {
            item.insert("related".into(), Value::Array(related));
        }
        items.push(Value::Object(item));
    }
    Ok(json!({"source": "media_profile", "slot": slot, "items": items}))
}

#[cfg(test)]
pub(crate) mod build {
    //! Test datasets built from the layouts.

    pub(crate) fn live_view(jpeg: &[u8], ptp3: bool) -> Vec<u8> {
        let header = if ptp3 { 16 } else { 8 };
        let pad = 8;
        let offset = (header + pad) as u32;
        let mut b = offset.to_le_bytes().to_vec();
        b.extend_from_slice(&(jpeg.len() as u32).to_le_bytes());
        if ptp3 {
            let ffi = [7u8; 4];
            b.extend_from_slice(&(offset + jpeg.len() as u32).to_le_bytes());
            b.extend_from_slice(&(ffi.len() as u32).to_le_bytes());
            b.extend(std::iter::repeat_n(0u8, pad));
            b.extend_from_slice(jpeg);
            b.extend_from_slice(&ffi);
        } else {
            b.extend(std::iter::repeat_n(0u8, pad));
            b.extend_from_slice(jpeg);
        }
        b
    }

    /// A minimal JPEG with a baseline start-of-frame of the given size.
    pub(crate) fn jpeg(width: u16, height: u16) -> Vec<u8> {
        let mut b = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x04, 0x4A, 0x46];
        b.extend_from_slice(&[0xFF, 0xC0, 0x00, 0x0B, 0x08]);
        b.extend_from_slice(&height.to_be_bytes());
        b.extend_from_slice(&width.to_be_bytes());
        b.extend_from_slice(&[0x01, 0x01, 0x11, 0x00]);
        b.extend_from_slice(&[0xFF, 0xD9]);
        b
    }

    /// A content list with one still content holding one JPEG file.
    pub(crate) fn content_info_list(slot: u32, content_id: u32, path: &str, size: u64) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(&100u16.to_le_bytes());
        b.extend_from_slice(&[0, 0]);
        b.extend_from_slice(&1_700_000_000_000u64.to_le_bytes());
        b.extend(std::iter::repeat_n(0u8, 128));
        b.extend_from_slice(&slot.to_le_bytes());
        b.extend_from_slice(&2u32.to_le_bytes()); // one content, one zeroed
                                                  // content
        for v in [1u32, content_id, 100, 1, 0, 0, 1] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        for t in [
            1_700_000_000_000u64,
            1_700_000_000_000,
            1_700_000_000_000,
            1_700_000_000_000,
        ] {
            b.extend_from_slice(&t.to_le_bytes());
        }
        for v in [3u32, 0, 0, 1] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        b.push(1); // one shot mark, set
        b.extend_from_slice(&1u32.to_le_bytes()); // one file
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&[0, 0]);
        let mut p = path.as_bytes().to_vec();
        p.push(0);
        b.extend_from_slice(&(p.len() as u32).to_le_bytes());
        b.extend_from_slice(&p);
        b.extend_from_slice(&0x3801u32.to_le_bytes());
        b.extend_from_slice(&size.to_le_bytes());
        b.extend(std::iter::repeat_n(0xAAu8, 32));
        b.extend_from_slice(&1u32.to_le_bytes()); // image parameters
        b.extend_from_slice(&6000u32.to_le_bytes());
        b.extend_from_slice(&4000u32.to_le_bytes());
        b.extend_from_slice(&0u32.to_le_bytes()); // no video
        b.extend_from_slice(&0u32.to_le_bytes()); // no audio
                                                  // a removed content: zeros
        b.extend(std::iter::repeat_n(0u8, 120));
        b
    }
}

#[cfg(test)]
mod tests {
    use super::build::*;
    use super::*;

    #[test]
    fn live_view_frames_from_both_protocol_versions() {
        let jpg = jpeg(640, 424);
        let f = parse_live_view(&live_view(&jpg, true)).unwrap().unwrap();
        assert_eq!(f.jpeg, jpg);
        assert_eq!(f.focal_frame_info, Some(vec![7; 4]));
        let f = parse_live_view(&live_view(&jpg, false)).unwrap().unwrap();
        assert_eq!(f.jpeg, jpg);
        assert_eq!(jpeg_size(&f.jpeg), Some((640, 424)));
        // Asked again too soon: no image.
        assert_eq!(parse_live_view(&[16, 0, 0, 0, 0, 0, 0, 0]).unwrap(), None);
        assert!(parse_live_view(&[16, 0, 0, 0, 9, 0, 0, 0]).is_err());
    }

    #[test]
    fn a_jpeg_inside_a_stream() {
        let jpg = jpeg(1, 1);
        let mut stream = b"--boundary\r\nContent-Type: image/jpeg\r\n\r\n".to_vec();
        stream.extend_from_slice(&jpg);
        stream.extend_from_slice(b"\r\n--boundary");
        assert_eq!(find_jpeg(&stream), Some(&jpg[..]));
    }

    #[test]
    fn content_info_list_with_a_trailing_removed_entry() {
        let (v, files) = parse_content_info_list(&content_info_list(
            1,
            42,
            "DCIM/100MSDCF/DSC00001.JPG",
            123_456,
        ))
        .unwrap();
        assert_eq!(v["slot"], 1);
        assert_eq!(v["items"].as_array().unwrap().len(), 1);
        let item = &v["items"][0];
        assert_eq!(item["content_id"], 42);
        assert_eq!(item["rating"], 3);
        assert_eq!(item["shot_mark"], true);
        let file = &item["files"][0];
        assert_eq!(file["id"], "c:1:42:1");
        assert_eq!(file["path"], "DCIM/100MSDCF/DSC00001.JPG");
        assert_eq!(file["format"], "jpeg");
        assert_eq!(file["width"], 6000);
        assert_eq!(
            files,
            [ListedFile {
                id: "c:1:42:1".into(),
                size: 123_456
            }]
        );
    }

    #[test]
    fn object_info_and_arrays() {
        let mut b = Vec::new();
        b.extend_from_slice(&0x0001_0001u32.to_le_bytes());
        b.extend_from_slice(&0x3801u16.to_le_bytes());
        b.extend_from_slice(&0u16.to_le_bytes());
        b.extend_from_slice(&5000u32.to_le_bytes());
        b.extend_from_slice(&0u16.to_le_bytes());
        for v in [0u32, 0, 0, 640, 480, 24, 0] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        b.extend_from_slice(&0u16.to_le_bytes());
        b.extend_from_slice(&0u32.to_le_bytes());
        b.extend_from_slice(&0u32.to_le_bytes());
        super::super::sony_camera_dataset::write_string(&mut b, "DSC00001.JPG");
        super::super::sony_camera_dataset::write_string(&mut b, "20260101T120000");
        let info = parse_object_info(&b).unwrap();
        assert_eq!(info.filename, "DSC00001.JPG");
        assert_eq!(info.size, 5000);
        assert_eq!(info.to_json(7)["id"], "o:7");
        assert_eq!(
            parse_u32_array(&[2, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0]).unwrap(),
            [1, 2]
        );
        assert!(parse_u32_array(&[9, 0, 0, 0]).is_err());
    }

    #[test]
    fn media_profile_clips() {
        let xml = r#"<?xml version="1.0"?><MediaProfile xmlns="urn:x"><Contents>
            <Material uri="./Clip/C0001.MXF" type="MXF" dur="250" fps="25p" videoType="XAVC" ch="4" umid="ab">
              <Proxy uri="./Sub/C0001S03.MP4" type="MP4"/>
              <RelevantInfo uri="./Clip/C0001M01.XML" type="XML"/>
            </Material></Contents></MediaProfile>"#;
        let v = parse_media_profile(xml, 2).unwrap();
        let clip = &v["items"][0];
        assert_eq!(clip["id"], "m:2:./Clip/C0001.MXF");
        assert_eq!(clip["duration_frames"], 250);
        assert_eq!(clip["proxy"]["id"], "m:2:./Sub/C0001S03.MP4");
        assert_eq!(clip["related"][0]["type"], "XML");
        assert!(parse_media_profile("<nope", 1).is_err());
    }
}
