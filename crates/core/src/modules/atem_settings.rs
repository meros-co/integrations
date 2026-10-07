//! Media pool management, recording and streaming settings, multiviewer
//! layout and window overlays, the video mode, the startup state, and macro
//! recording.
//!
//! Layouts from Sofie `commands/Media/`, `commands/Recording/`,
//! `commands/Streaming/`, `commands/Settings/`, `commands/Macro/` and
//! `commands/StartupStateCommand.ts`, with the LibAtem samples in Sofie
//! `commands/__tests__/libatem-data.json` for the field positions.

use serde_json::{json, Value};

use super::{
    fits, int, invalid, one, opt_bool, opt_int, opt_str, put16, put32, text, u16_at, u32_at,
    unsupported, Atem, CommandError, Out, Params, V2_28, V2_30,
};

/// Video modes, by wire value (Sofie `enums/index.ts:250-283`).
pub(super) const VIDEO_MODES: [&str; 28] = [
    "525i59_94_ntsc",
    "625i50_pal",
    "525i59_94_ntsc_16_9",
    "625i50_pal_16_9",
    "720p50",
    "720p59_94",
    "1080i50",
    "1080i59_94",
    "1080p23_98",
    "1080p24",
    "1080p25",
    "1080p29_97",
    "1080p50",
    "1080p59_94",
    "2160p23_98",
    "2160p24",
    "2160p25",
    "2160p29_97",
    "2160p50",
    "2160p59_94",
    "4320p23_98",
    "4320p24",
    "4320p25",
    "4320p29_97",
    "4320p50",
    "4320p59_94",
    "1080p30",
    "1080p60",
];

/// Multiviewer layouts: which quadrants are split into four small windows
/// (Sofie `enums/index.ts:372-382`).
pub(super) const LAYOUTS: [(&str, u8); 9] = [
    ("default", 0),
    ("top_left_small", 1),
    ("top_right_small", 2),
    ("program_bottom", 3),
    ("bottom_left_small", 4),
    ("program_right", 5),
    ("bottom_right_small", 8),
    ("program_left", 10),
    ("program_top", 12),
];

/// Recording disk status bits (Sofie `enums/index.ts:332-339`).
const DISK_STATUS: [&str; 4] = ["idle", "unformatted", "active", "recording"];

fn layout_name(v: u8) -> &'static str {
    LAYOUTS
        .iter()
        .find(|(_, x)| *x == v)
        .map(|(n, _)| *n)
        .unwrap_or("unknown")
}

/// A length-prefixed name and description from `at`, as MSRc and CMPr carry
/// them, padded to a multiple of 4 counted from an 8-byte header.
fn named_body(head: Vec<u8>, name: &str, description: &str) -> Vec<u8> {
    let mut b = head;
    b.extend_from_slice(name.as_bytes());
    b.extend_from_slice(description.as_bytes());
    let len = (8 + name.len() + description.len()).next_multiple_of(4);
    b.resize(len.max(b.len()), 0);
    b
}

impl Atem {
    pub(super) fn decode_settings(&mut self, name: &[u8; 4], b: &[u8]) -> Option<Value> {
        let need = |n: usize| (b.len() >= n).then_some(());
        let one = |i: u8| (i as u32 + 1).to_string();
        match name {
            // Video mode (Sofie `Settings/VideoMode.ts:29-34`).
            b"VidM" => {
                need(1)?;
                Some(json!({"video_mode": super::named(&VIDEO_MODES, b[0])}))
            }
            // u16 count, 2 rsv, then per mode: mode, 3 rsv, u32 multiviewer
            // modes, u32 down-convert modes, and from 2.28 a reconfigure
            // flag (Sofie `DeviceProfile/videoMixerConfigCommand.ts:12-33`).
            b"_VMC" => {
                need(2)?;
                let size = if self.version >= V2_28 { 13 } else { 12 };
                let mut modes = Vec::new();
                for i in 0..u16_at(b, 0) as usize {
                    let at = 4 + i * size;
                    if at + size > b.len() {
                        break;
                    }
                    modes.push(b[at]);
                }
                self.topology.video_modes = modes.clone();
                let names: Vec<_> = modes
                    .iter()
                    .map(|m| super::named(&VIDEO_MODES, *m))
                    .collect();
                Some(json!({"topology": {"video_modes": names}}))
            }
            // Multiviewer, layout, program and preview swapped (Sofie
            // `Settings/MultiViewerPropertiesCommand.ts:40-50`).
            b"MvPr" => {
                need(3)?;
                self.topology.multiviewers_seen.insert(b[0]);
                Some(json!({"multiviewers": {one(b[0]): {
                    "layout": layout_name(b[1]),
                    "program_preview_swapped": b[2] != 0,
                }}}))
            }
            // Multiviewer, window, VU meter on (Sofie
            // `Settings/MultiViewerWindowVuMeterCommand.ts:30-40`).
            b"VuMC" => {
                need(3)?;
                Some(json!({"multiviewers": {one(b[0]): {"windows": {one(b[1]): {
                    "vu_meter": b[2] != 0,
                }}}}}))
            }
            // Multiviewer, window, safe area on (Sofie
            // `Settings/MultiViewerWindowSafeAreaCommand.ts:24-34`).
            b"SaMw" => {
                need(3)?;
                Some(json!({"multiviewers": {one(b[0]): {"windows": {one(b[1]): {
                    "safe_area": b[2] != 0,
                }}}}}))
            }
            // Multiviewer, window, rsv, bits: 0 label, 1 border (Sofie
            // `Settings/MultiViewerWindowOverlayPropertiesCommand.ts:44-56`).
            b"MvOv" => {
                need(4)?;
                Some(json!({"multiviewers": {one(b[0]): {"windows": {one(b[1]): {
                    "label_visible": b[3] & 1 != 0,
                    "border_visible": b[3] & 2 != 0,
                }}}}}))
            }
            // Multiviewer, VU opacity in percent (Sofie
            // `Settings/MultiViewerVuOpacityCommand.ts:24-32`).
            b"VuMo" => {
                need(2)?;
                self.topology.multiviewers_seen.insert(b[0]);
                Some(json!({"multiviewers": {one(b[0]): {"vu_opacity": b[1]}}}))
            }
            // Filename[128], u32 working set 1 disk, u32 working set 2 disk,
            // record in all cameras (Sofie `Recording/RecordingSettingsCommand.ts:44-52`).
            b"RMSu" => {
                need(137)?;
                Some(json!({"recording": {
                    "filename": text(&b[..128]),
                    "working_set_1_disk": u32_at(b, 128),
                    "working_set_2_disk": u32_at(b, 132),
                    "record_in_all_cameras": b[136] != 0,
                }}))
            }
            // u32 disk id, u32 seconds left, u16 status (bit 5: removed),
            // volume name[64] (Sofie `Recording/RecordingDiskCommand.ts:30-45`).
            b"RTMD" => {
                need(74)?;
                let id = u32_at(b, 0);
                let status = u16_at(b, 8);
                let disk = if status & 32 != 0 {
                    self.topology.disks.remove(&id);
                    Value::Null
                } else {
                    self.topology.disks.insert(id);
                    let names: Vec<_> = DISK_STATUS
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| status & (1 << i) != 0)
                        .map(|(_, n)| *n)
                        .collect();
                    json!({
                        "time_available": u32_at(b, 4),
                        "status": names,
                        "volume_name": text(&b[10..74]),
                    })
                };
                Some(json!({"recording": {"disks": {id.to_string(): disk}}}))
            }
            // Record every input separately (ISO models; Sofie
            // `Recording/RecordingISOCommand.ts:20-30`).
            b"ISOi" => {
                need(1)?;
                Some(json!({"recording": {"record_all_inputs": b[0] != 0}}))
            }
            // u32 low and high audio bitrates (Sofie
            // `Streaming/StreamingAudioBitratesCommand.ts:22-30`).
            b"STAB" => {
                need(8)?;
                self.topology.audio_bitrates = Some((u32_at(b, 0), u32_at(b, 4)));
                Some(json!({"streaming": {"audio_bitrates": {
                    "low": u32_at(b, 0), "high": u32_at(b, 4),
                }}}))
            }
            // u32 encoding bitrate, u16 cache used (Sofie
            // `Streaming/StreamingStatsCommand.ts:14-20`).
            b"SRSS" => {
                need(6)?;
                Some(json!({"streaming": {
                    "encoding_bitrate": u32_at(b, 0),
                    "cache_used": u16_at(b, 4),
                }}))
            }
            // Four u16 clip lengths, u16 unassigned (Sofie
            // `Settings/MediaPool.ts:30-40`).
            b"MPSp" => {
                need(10)?;
                let max = [0, 2, 4, 6].map(|at| u16_at(b, at));
                self.topology.clip_max_frames = Some(max);
                Some(json!({"media_pool": {
                    "clip_max_frames": max,
                    "unassigned_frames": u16_at(b, 8),
                }}))
            }
            // Recording, rsv, u16 macro (0xFFFF when none) (Sofie
            // `Macro/MacroRecordingStatusCommand.ts:10-16`).
            b"MRcS" => {
                need(4)?;
                let index = u16_at(b, 2);
                Some(json!({"macro_recording": {
                    "recording": b[0] != 0,
                    "macro": if b[0] != 0 && index != 0xFFFF {
                        json!(index as u32 + 1)
                    } else {
                        Value::Null
                    },
                }}))
            }
            _ => None,
        }
    }

    fn check_encoder(&self, name: &str) -> Result<(), CommandError> {
        if self.version < V2_30 || !self.topology.encoder {
            return Err(unsupported(
                name,
                "this switcher (no encoder, or protocol before 2.30)",
            ));
        }
        Ok(())
    }

    /// A multiviewer numbered from 1 that the switcher reported.
    fn check_multiviewer(&self, params: &Params) -> Result<u8, CommandError> {
        let mv = int(params, "multiviewer") - 1;
        let known = self
            .topology
            .multiviewers_seen
            .contains(&(mv.clamp(0, 255) as u8))
            || self
                .topology
                .multiviewer_windows
                .keys()
                .any(|(m, _)| *m as i64 == mv);
        if !known {
            return Err(invalid(format!("multiviewer {} does not exist", mv + 1)));
        }
        Ok(mv as u8)
    }

    /// A multiviewer window numbered from 1 that the switcher reported.
    fn check_window(&self, params: &Params) -> Result<(u8, u8), CommandError> {
        let mv = self.check_multiviewer(params)?;
        let window = (int(params, "window") - 1).clamp(0, 255) as u8;
        if !self
            .topology
            .multiviewer_windows
            .contains_key(&(mv, window))
        {
            return Err(invalid(format!(
                "multiviewer {} has no window {}",
                mv as u32 + 1,
                window as u32 + 1
            )));
        }
        Ok((mv, window))
    }

    pub(super) fn build_settings(
        &self,
        name: &str,
        params: &Params,
    ) -> Result<Option<Out>, CommandError> {
        match name {
            // Capture the program output to the first free still slot (Sofie
            // `Media/MediaPoolCaptureStillCommand.ts`: an empty body).
            "capture_still" => {
                if self.topology.stills == 0 {
                    return Err(unsupported(name, "this switcher (no stills pool)"));
                }
                one(b"Capt", Vec::new())
            }
            // Slot, 3 rsv (Sofie `Media/MediaPoolClearStillCommand.ts:10-14`).
            "clear_still" => {
                let slot = self.check_index(params, "still", self.topology.stills)?;
                one(b"CSTL", vec![slot, 0, 0, 0])
            }
            "clear_clip" => {
                if self.topology.clips == 0 {
                    return Err(unsupported(name, "this switcher (no clip pool)"));
                }
                let slot = self.check_index(params, "clip", self.topology.clips)?;
                one(b"CMPC", vec![slot, 0, 0, 0])
            }
            // 3, clip, name at 2 (Sofie writes at most 44 bytes), u16 frames at
            // 66 (Sofie `Media/MediaPoolSetClipCommand.ts:15-25`).
            "set_clip" => {
                if self.topology.clips == 0 {
                    return Err(unsupported(name, "this switcher (no clip pool)"));
                }
                let slot = self.check_index(params, "clip", self.topology.clips)?;
                let clip_name = fits(params, "name", 43)?.unwrap_or_default();
                let mut b = vec![0u8; 68];
                b[0] = 3;
                b[1] = slot;
                b[2..2 + clip_name.len()].copy_from_slice(clip_name.as_bytes());
                put16(&mut b, 66, int(params, "frames") as u16);
                one(b"SMPC", b)
            }
            // Four u16 clip lengths; lengths not given keep the reported ones
            // (Sofie `Settings/MediaPool.ts:10-20`, from 2.28).
            "set_clip_lengths" => {
                if self.version < V2_28 || self.topology.clips == 0 {
                    return Err(unsupported(
                        name,
                        "this switcher (no clip pool, or protocol before 2.28)",
                    ));
                }
                let mut frames = self.topology.clip_max_frames.unwrap_or_default();
                let mut any = false;
                for (i, f) in frames.iter_mut().enumerate() {
                    if let Some(v) = opt_int(params, &format!("clip{}", i + 1)) {
                        if i >= self.topology.clips as usize {
                            return Err(invalid(format!("the pool has no clip {}", i + 1)));
                        }
                        *f = v as u16;
                        any = true;
                    }
                }
                if !any {
                    return Err(super::nothing_to_set());
                }
                if self.topology.clip_max_frames.is_none() {
                    return Err(invalid(
                        "the switcher has not reported its clip lengths".into(),
                    ));
                }
                let mut b = vec![0u8; 8];
                for (i, f) in frames.iter().enumerate() {
                    put16(&mut b, 2 * i, *f);
                }
                one(b"CMPS", b)
            }
            // Mask (0 filename, 1 working set 1, 2 working set 2, 3 record in
            // all cameras), filename[128] at 1, u32 disks at 132 and 136, flag
            // at 140 (Sofie `Recording/RecordingSettingsCommand.ts:8-30`).
            "set_recording" => {
                self.check_encoder(name)?;
                let mut b = vec![0u8; 144];
                if let Some(f) = fits(params, "filename", 127)? {
                    b[0] |= 1;
                    b[1..1 + f.len()].copy_from_slice(f.as_bytes());
                }
                for (bit, field, at) in [
                    (1, "working_set_1_disk", 132),
                    (2, "working_set_2_disk", 136),
                ] {
                    if let Some(id) = opt_int(params, field) {
                        if id != 0 && !self.topology.disks.contains(&(id as u32)) {
                            return Err(invalid(format!("the switcher has no disk {id}")));
                        }
                        b[0] |= 1 << bit;
                        put32(&mut b, at, id as u32);
                    }
                }
                if let Some(v) = opt_bool(params, "record_in_all_cameras") {
                    b[0] |= 8;
                    b[140] = v as u8;
                }
                if b[0] == 0 {
                    return Err(super::nothing_to_set());
                }
                one(b"CRMS", b)
            }
            // Empty: move recording to the other working-set disk (Sofie
            // `Recording/RecordingDiskCommand.ts:6-14`).
            "switch_recording_disk" => {
                self.check_encoder(name)?;
                one(b"RMSp", Vec::new())
            }
            // Record every input separately, 3 rsv; sent with the name the
            // switcher reports it by (Sofie `Recording/RecordingISOCommand.ts:6-16`).
            "set_recording_iso" => {
                self.check_encoder(name)?;
                one(
                    b"ISOi",
                    vec![super::flag(params, "record_all_inputs") as u8, 0, 0, 0],
                )
            }
            // u32 low and high audio bitrates; sent with the name the switcher
            // reports them by (Sofie `Streaming/StreamingAudioBitratesCommand.ts:6-18`).
            "set_streaming_audio_bitrates" => {
                self.check_encoder(name)?;
                let (low, high) = self.topology.audio_bitrates.unwrap_or((128_000, 192_000));
                let low = opt_int(params, "low").map_or(low, |v| v as u32);
                let high = opt_int(params, "high").map_or(high, |v| v as u32);
                if opt_int(params, "low").is_none() && opt_int(params, "high").is_none() {
                    return Err(super::nothing_to_set());
                }
                let mut b = vec![0u8; 8];
                put32(&mut b, 0, low);
                put32(&mut b, 4, high);
                one(b"STAB", b)
            }
            // Mask (0 layout, 1 swapped), multiviewer, layout, swapped (Sofie
            // `Settings/MultiViewerPropertiesCommand.ts:6-26`, from 2.28).
            "set_multiviewer_properties" => {
                if self.version < V2_28 {
                    return Err(unsupported(name, "this switcher (protocol before 2.28)"));
                }
                let mv = self.check_multiviewer(params)?;
                let mut b = vec![0u8, mv, 0, 0];
                if let Some(l) = opt_str(params, "layout") {
                    let v = LAYOUTS
                        .iter()
                        .find(|(n, _)| *n == l)
                        .map(|(_, v)| *v)
                        .ok_or_else(|| invalid(format!("unknown layout {l:?}")))?;
                    b[0] |= 1;
                    b[2] = v;
                }
                if let Some(v) = opt_bool(params, "program_preview_swapped") {
                    b[0] |= 2;
                    b[3] = v as u8;
                }
                if b[0] == 0 {
                    return Err(super::nothing_to_set());
                }
                one(b"CMvP", b)
            }
            // Multiviewer, window, on, rsv (Sofie
            // `Settings/MultiViewerWindowVuMeterCommand.ts:6-22`).
            "set_multiviewer_vu_meter" => {
                let (mv, window) = self.check_window(params)?;
                one(
                    b"VuMS",
                    vec![mv, window, super::flag(params, "enabled") as u8, 0],
                )
            }
            // Multiviewer, window, on, rsv; sent with the name the switcher
            // reports it by (Sofie `Settings/MultiViewerWindowSafeAreaCommand.ts:6-20`).
            "set_multiviewer_safe_area" => {
                if self.version < V2_28 {
                    return Err(unsupported(name, "this switcher (protocol before 2.28)"));
                }
                let (mv, window) = self.check_window(params)?;
                one(
                    b"SaMw",
                    vec![mv, window, super::flag(params, "enabled") as u8, 0],
                )
            }
            // Multiviewer, opacity, 2 rsv (Sofie
            // `Settings/MultiViewerVuOpacityCommand.ts:6-20`).
            "set_multiviewer_vu_opacity" => {
                let mv = self.check_multiviewer(params)?;
                one(b"VuMo", vec![mv, int(params, "opacity") as u8, 0, 0])
            }
            // Multiviewer, window, rsv, bits (0 label, 1 border), rsv, mask,
            // 2 rsv (Sofie `Settings/MultiViewerWindowOverlayPropertiesCommand.ts:8-34`).
            "set_multiviewer_overlay" => {
                let (mv, window) = self.check_window(params)?;
                let mut b = vec![mv, window, 0, 0, 0, 0, 0, 0];
                for (bit, field) in [(0, "label_visible"), (1, "border_visible")] {
                    if let Some(v) = opt_bool(params, field) {
                        b[5] |= 1 << bit;
                        if v {
                            b[3] |= 1 << bit;
                        }
                    }
                }
                if b[5] == 0 {
                    return Err(super::nothing_to_set());
                }
                one(b"CMvO", b)
            }
            // Mode, 3 rsv (Sofie `Settings/VideoMode.ts:10-20`).
            "set_video_mode" => {
                let mode = super::enum_param(params, "mode", &VIDEO_MODES)?
                    .ok_or_else(|| invalid("mode is required".into()))?;
                if !self.topology.video_modes.is_empty()
                    && !self.topology.video_modes.contains(&mode)
                {
                    return Err(invalid(format!(
                        "the switcher does not offer video mode {}",
                        VIDEO_MODES[mode as usize]
                    )));
                }
                one(b"CVdM", vec![mode, 0, 0, 0])
            }
            // A zero mode byte and 3 rsv (Sofie `StartupStateCommand.ts:6-26`).
            "save_startup_state" => one(b"SRsv", vec![0, 0, 0, 0]),
            "clear_startup_state" => one(b"SRcl", vec![0, 0, 0, 0]),
            // u16 macro, u16 name length, u16 description length, name,
            // description (Sofie `Macro/MacroRecordCommand.ts:8-24`).
            "start_macro_recording" => {
                let slot = self.check_index(params, "macro", self.topology.macros)?;
                let n = fits(params, "name", 255)?.unwrap_or_default();
                let d = fits(params, "description", 1023)?.unwrap_or_default();
                let mut head = vec![0u8; 6];
                put16(&mut head, 0, slot as u16);
                put16(&mut head, 2, n.len() as u16);
                put16(&mut head, 4, d.len() as u16);
                one(b"MSRc", named_body(head, &n, &d))
            }
            // Macro actions with no macro: 2 stop recording, 3 insert a user
            // wait (Sofie `Macro/MacroActionCommand.ts:15-32`).
            "stop_macro_recording" => one(b"MAct", vec![0xFF, 0xFF, 2, 0]),
            "macro_add_user_wait" => one(b"MAct", vec![0xFF, 0xFF, 3, 0]),
            // 2 rsv, u16 frames (Sofie `Macro/MacroAddTimedPauseCommand.ts:8-14`).
            "macro_add_pause" => {
                let mut b = vec![0u8; 4];
                put16(&mut b, 2, int(params, "frames") as u16);
                one(b"MSlp", b)
            }
            // Action 5 deletes the macro (Sofie `Macro/MacroActionCommand.ts:15-32`).
            "delete_macro" => {
                let slot = self.check_index(params, "macro", self.topology.macros)?;
                let [hi, lo] = (slot as u16).to_be_bytes();
                one(b"MAct", vec![hi, lo, 5, 0])
            }
            // Mask (0 name, 1 description), rsv, u16 macro, u16 name length,
            // u16 description length, name, description (Sofie
            // `Macro/MacroPropertiesCommand.ts:44-70`).
            "set_macro_properties" => {
                let slot = self.check_index(params, "macro", self.topology.macros)?;
                if self.topology.macros_used.get(&(slot as u16)) != Some(&true) {
                    return Err(invalid(format!("macro {} is empty", slot as u16 + 1)));
                }
                let n = fits(params, "name", 255)?;
                let d = fits(params, "description", 1023)?;
                let mask = n.is_some() as u8 | (d.is_some() as u8) << 1;
                if mask == 0 {
                    return Err(super::nothing_to_set());
                }
                let (n, d) = (n.unwrap_or_default(), d.unwrap_or_default());
                let mut head = vec![0u8; 8];
                head[0] = mask;
                put16(&mut head, 2, slot as u16);
                put16(&mut head, 4, n.len() as u16);
                put16(&mut head, 6, d.len() as u16);
                one(b"CMPr", named_body(head, &n, &d))
            }
            _ => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{cmd, decoded, hex, payload, ready, refused};
    use super::super::{command, CommandError};
    use serde_json::json;

    #[test]
    fn media_pool_layouts() {
        let (mut m, _) = ready();
        assert_eq!(
            payload(&mut m, "capture_still", json!({})),
            command(b"Capt", &[])
        );
        assert_eq!(
            payload(&mut m, "clear_still", json!({"still": 5})),
            command(b"CSTL", &[4, 0, 0, 0])
        );
        assert_eq!(
            payload(&mut m, "clear_clip", json!({"clip": 2})),
            command(b"CMPC", &[1, 0, 0, 0])
        );
        let p = payload(
            &mut m,
            "set_clip",
            json!({"clip": 1, "name": "Intro", "frames": 120}),
        );
        let mut b = vec![0u8; 68];
        b[0] = 3;
        b[2..7].copy_from_slice(b"Intro");
        b[67] = 120;
        assert_eq!(p, command(b"SMPC", &b));
        // Clip lengths need the switcher's own first.
        assert!(matches!(
            refused(&mut m, "set_clip_lengths", json!({"clip2": 80})),
            CommandError::InvalidParams { .. }
        ));
        decoded(
            &mut m,
            &[cmd(b"MPSp", &[0, 50, 0, 50, 0, 0, 0, 0, 0, 20, 0, 0])],
        );
        assert_eq!(
            payload(&mut m, "set_clip_lengths", json!({"clip2": 80})),
            command(b"CMPS", &[0, 50, 0, 80, 0, 0, 0, 0])
        );
        assert!(matches!(
            refused(&mut m, "set_clip_lengths", json!({"clip3": 80})),
            CommandError::InvalidParams { .. }
        ));
    }

    #[test]
    fn recording_and_streaming_layouts() {
        let (mut m, _) = ready();
        let p = payload(
            &mut m,
            "set_recording",
            json!({"filename": "Show", "record_in_all_cameras": true}),
        );
        let mut b = vec![0u8; 144];
        b[0] = 9;
        b[1..5].copy_from_slice(b"Show");
        b[140] = 1;
        assert_eq!(p, command(b"CRMS", &b));
        assert!(matches!(
            refused(&mut m, "set_recording", json!({"working_set_1_disk": 7})),
            CommandError::InvalidParams { .. }
        ));
        let mut rtmd = vec![0u8; 76];
        rtmd[3] = 7;
        rtmd[9] = 1;
        decoded(&mut m, &[cmd(b"RTMD", &rtmd)]);
        let p = payload(&mut m, "set_recording", json!({"working_set_1_disk": 7}));
        let mut b = vec![0u8; 144];
        b[0] = 2;
        b[135] = 7;
        assert_eq!(p, command(b"CRMS", &b));
        assert_eq!(
            payload(&mut m, "switch_recording_disk", json!({})),
            command(b"RMSp", &[])
        );
        assert_eq!(
            payload(
                &mut m,
                "set_recording_iso",
                json!({"record_all_inputs": true})
            ),
            command(b"ISOi", &[1, 0, 0, 0])
        );
        let p = payload(
            &mut m,
            "set_streaming_audio_bitrates",
            json!({"high": 256000}),
        );
        let mut b = 128_000u32.to_be_bytes().to_vec();
        b.extend_from_slice(&256_000u32.to_be_bytes());
        assert_eq!(p, command(b"STAB", &b));
    }

    #[test]
    fn multiviewer_video_mode_and_startup_layouts() {
        let (mut m, _) = ready();
        let mv = |extra: serde_json::Value| {
            let mut p = json!({"multiviewer": 1, "window": 3});
            p.as_object_mut()
                .unwrap()
                .extend(extra.as_object().unwrap().clone());
            p
        };
        let p = payload(
            &mut m,
            "set_multiviewer_properties",
            json!({"multiviewer": 1, "layout": "program_top", "program_preview_swapped": true}),
        );
        assert_eq!(p, command(b"CMvP", &[3, 0, 12, 1]));
        let p = payload(
            &mut m,
            "set_multiviewer_vu_meter",
            mv(json!({"enabled": true})),
        );
        assert_eq!(p, command(b"VuMS", &[0, 2, 1, 0]));
        let p = payload(
            &mut m,
            "set_multiviewer_safe_area",
            mv(json!({"enabled": false})),
        );
        assert_eq!(p, command(b"SaMw", &[0, 2, 0, 0]));
        let p = payload(
            &mut m,
            "set_multiviewer_vu_opacity",
            json!({"multiviewer": 1, "opacity": 85}),
        );
        assert_eq!(p, command(b"VuMo", &[0, 85, 0, 0]));
        let p = payload(
            &mut m,
            "set_multiviewer_overlay",
            mv(json!({"border_visible": true})),
        );
        assert_eq!(p, command(b"CMvO", &[0, 2, 0, 2, 0, 2, 0, 0]));
        assert!(matches!(
            refused(
                &mut m,
                "set_multiviewer_vu_meter",
                json!({"multiviewer": 1, "window": 4})
            ),
            CommandError::InvalidParams { .. }
        ));
        assert!(matches!(
            refused(
                &mut m,
                "set_multiviewer_vu_opacity",
                json!({"multiviewer": 2, "opacity": 5})
            ),
            CommandError::InvalidParams { .. }
        ));
        assert_eq!(
            payload(&mut m, "set_video_mode", json!({"mode": "1080p25"})),
            command(b"CVdM", &[10, 0, 0, 0])
        );
        // Once the switcher lists its modes, others are refused.
        let mut vmc = vec![0, 1, 0, 0];
        vmc.extend_from_slice(&[12; 13]);
        let s = decoded(&mut m, &[cmd(b"_VMC", &vmc)]);
        assert_eq!(s["topology"]["video_modes"], json!(["1080p50"]));
        assert!(matches!(
            refused(&mut m, "set_video_mode", json!({"mode": "1080p25"})),
            CommandError::InvalidParams { .. }
        ));
        assert_eq!(
            payload(&mut m, "save_startup_state", json!({})),
            command(b"SRsv", &[0, 0, 0, 0])
        );
        assert_eq!(
            payload(&mut m, "clear_startup_state", json!({})),
            command(b"SRcl", &[0, 0, 0, 0])
        );
    }

    #[test]
    fn macro_recording_layouts() {
        let (mut m, _) = ready();
        let p = payload(
            &mut m,
            "start_macro_recording",
            json!({"macro": 2, "name": "A", "description": "BC"}),
        );
        assert_eq!(
            p,
            command(b"MSRc", &[0, 1, 0, 1, 0, 2, b'A', b'B', b'C', 0, 0, 0])
        );
        assert_eq!(
            payload(&mut m, "stop_macro_recording", json!({})),
            command(b"MAct", &[0xFF, 0xFF, 2, 0])
        );
        assert_eq!(
            payload(&mut m, "macro_add_user_wait", json!({})),
            command(b"MAct", &[0xFF, 0xFF, 3, 0])
        );
        assert_eq!(
            payload(&mut m, "macro_add_pause", json!({"frames": 25})),
            command(b"MSlp", &[0, 0, 0, 25])
        );
        assert_eq!(
            payload(&mut m, "delete_macro", json!({"macro": 1})),
            command(b"MAct", &[0, 0, 5, 0])
        );
        assert_eq!(
            payload(
                &mut m,
                "set_macro_properties",
                json!({"macro": 1, "description": "x"})
            ),
            command(b"CMPr", &[2, 0, 0, 0, 0, 0, 0, 1, b'x', 0, 0, 0])
        );
        assert!(matches!(
            refused(
                &mut m,
                "set_macro_properties",
                json!({"macro": 2, "name": "x"})
            ),
            CommandError::InvalidParams { .. }
        ));
    }

    #[test]
    fn settings_state_from_the_libatem_samples() {
        let (mut m, _) = ready();
        let s = decoded(
            &mut m,
            &[
                cmd(b"VidM", &[8, 0, 0, 0]),
                cmd(b"MvPr", &[0, 12, 1, 0]),
                cmd(b"VuMC", &[0, 2, 1, 0]),
                cmd(b"SaMw", &[0, 2, 1, 0]),
                cmd(b"MvOv", &[0, 2, 0, 3]),
                cmd(b"VuMo", &[0, 85, 0, 0]),
                cmd(b"RMSu", &hex("35-36-32-65-37-37-37-31-2D-65-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-0F-68-45-A9-4D-E9-DB-97-00-00-00-00")),
                cmd(b"RTMD", &hex("2C-9D-2A-C4-6D-68-16-CD-00-01-63-62-36-33-64-30-65-65-2D-31-36-35-35-2D-34-64-63-66-2D-38-36-36-66-2D-34-32-61-34-31-37-63-33-34-65-39-35-35-35-64-61-64-38-65-31-2D-34-31-31-30-2D-34-30-33-00-00-00-00-00-00-00-00-00-00-00-00-00")),
                cmd(b"ISOi", &[1, 0, 0, 0]),
                cmd(b"STAB", &hex("7B-36-6D-D7-1B-27-66-8D")),
                cmd(b"SRSS", &hex("32-7F-E2-F7-00-1E-00-00")),
                cmd(b"MPSp", &hex("D4-C6-81-B3-9C-8A-A2-B1-FD-94-00-00")),
                cmd(b"MRcS", &hex("01-00-46-4C")),
            ],
        );
        assert_eq!(s["video_mode"], "1080p23_98");
        let mv = &s["multiviewers"]["1"];
        assert_eq!(mv["layout"], "program_top");
        assert_eq!(mv["program_preview_swapped"], true);
        assert_eq!(mv["vu_opacity"], 85);
        assert_eq!(
            mv["windows"]["3"],
            json!({"vu_meter": true, "safe_area": true, "label_visible": true,
                   "border_visible": true})
        );
        let r = &s["recording"];
        assert_eq!(r["filename"], "562e7771-e");
        assert_eq!(r["working_set_1_disk"], 258491817);
        assert_eq!(r["working_set_2_disk"], 1307171735);
        assert_eq!(r["record_in_all_cameras"], false);
        assert_eq!(r["record_all_inputs"], true);
        assert_eq!(
            r["disks"]["748497604"],
            json!({"time_available": 1835538125u32, "status": ["idle"],
                   "volume_name": "cb63d0ee-1655-4dcf-866f-42a417c34e9555dad8e1-4110-403"})
        );
        assert_eq!(
            s["streaming"]["audio_bitrates"],
            json!({"low": 2067164631u32, "high": 455566989})
        );
        assert_eq!(s["streaming"]["encoding_bitrate"], 847241975);
        assert_eq!(s["streaming"]["cache_used"], 30);
        assert_eq!(
            s["media_pool"]["clip_max_frames"],
            json!([54470, 33203, 40074, 41649])
        );
        assert_eq!(s["media_pool"]["unassigned_frames"], 64916);
        assert_eq!(
            s["macro_recording"],
            json!({"recording": true, "macro": 17997})
        );
        // A removed disk leaves the state.
        let s = decoded(
            &mut m,
            &[cmd(b"RTMD", &hex("37-78-20-2C-3E-67-C9-22-00-24-63-36-38-62-36-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00-00"))],
        );
        assert_eq!(
            s["recording"]["disks"]["930619436"],
            serde_json::Value::Null
        );
    }

    #[test]
    fn macro_loop_streaming_bitrates_and_extended_state() {
        let (mut m, _) = ready();
        // The loop setting and the run go in one packet, loop first.
        assert_eq!(
            payload(&mut m, "run_macro", json!({"macro": 1, "loop": true})),
            [
                command(b"MRCP", &[1, 1, 0, 0]),
                command(b"MAct", &[0, 0, 0, 0])
            ]
            .concat()
        );
        // One video bitrate needs the other until the switcher reports both.
        assert!(matches!(
            refused(
                &mut m,
                "set_streaming_service",
                json!({"low_bitrate": 1000})
            ),
            CommandError::InvalidParams { .. }
        ));
        let p = payload(
            &mut m,
            "set_streaming_service",
            json!({"low_bitrate": 1000, "high_bitrate": 2000}),
        );
        assert_eq!(p[8], 8);
        assert_eq!(
            &p[8 + 1092..8 + 1100],
            &[0, 0, 0x03, 0xE8, 0, 0, 0x07, 0xD0]
        );
        let mut srsu = vec![0u8; 1096];
        srsu[1088..1096].copy_from_slice(&[0, 0, 0x0B, 0xB8, 0, 0, 0x0F, 0xA0]);
        let mut mprp = vec![0, 0, 1, 1, 0, 4, 0, 3];
        mprp.extend_from_slice(b"OpenAll");
        mprp.push(0);
        let s = decoded(
            &mut m,
            &[
                cmd(b"SRSU", &srsu),
                cmd(b"StRS", &[0, 0x14, 0, 0]),
                cmd(b"MvIn", &[0, 2, 0, 2, 1, 0, 0, 0]),
                cmd(b"MPrp", &mprp),
            ],
        );
        assert_eq!(
            s["streaming"]["video_bitrates"],
            json!({"low": 3000, "high": 4000})
        );
        assert_eq!(s["streaming"]["state"], "streaming");
        assert_eq!(s["streaming"]["error"], "invalid_state");
        assert_eq!(
            s["multiviewers"]["1"]["windows"]["3"],
            json!({"source": 2, "supports_vu_meter": true, "supports_safe_area": false})
        );
        assert_eq!(
            s["macros"]["1"],
            json!({"name": "Open", "description": "All", "unsupported_ops": true})
        );
        // Now the high bitrate alone keeps the reported low one.
        let p = payload(
            &mut m,
            "set_streaming_service",
            json!({"high_bitrate": 5000}),
        );
        assert_eq!(
            &p[8 + 1092..8 + 1100],
            &[0, 0, 0x0B, 0xB8, 0, 0, 0x13, 0x88]
        );
    }
}
