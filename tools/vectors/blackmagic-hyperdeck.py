# HyperDeck: payload + CRLF; multiline-only commands put each parameter on its
# own line and end with a blank line (Blackmagic HyperDeck Ethernet Protocol,
# December 2024: command table p.2-9, syntax p.10).
H = "blackmagic-hyperdeck"

# Session and device (p.2, p.7-8, p.12-13, p.16, p.24).
text(H, "help", {}, "help\r\n")
text(H, "list_commands", {}, "commands\r\n")
text(H, "ping", {}, "ping\r\n", device_reply="200 ok\r\n", expect_result={"ok": {"kind": "ack"}})
text(H, "get_device_info", {}, "device info\r\n",
     device_reply="204 device info:\r\nprotocol version: 1.11\r\nmodel: HyperDeck Studio HD Pro\r\n\r\n",
     expect_result={"ok": {"kind": "value", "value": {"protocol version": "1.11", "model": "HyperDeck Studio HD Pro"}}})
text(H, "get_uptime", {}, "uptime\r\n")
text(H, "set_identify", {"enabled": True}, "identify: enable: true\r\n")
text(H, "set_watchdog", {"period": 30}, "watchdog: period: 30\r\n")
text(H, "reboot", {}, "reboot\r\n")
text(H, "set_response_version", {"version": 2}, "connection protocol: response version: 2\r\n")

# Remote control (p.6, p.12).
text(H, "get_remote", {}, "remote\r\n",
     device_reply="210 remote info:\r\nenabled: true\r\noverride: false\r\n\r\n",
     expect_result={"ok": {"kind": "value", "value": {"enabled": "true", "override": "false"}}})
text(H, "set_remote_enabled", {"enabled": False}, "remote: enable: false\r\n")
text(H, "set_remote_override", {"enabled": True}, "remote: override: true\r\n")

# Notifications (p.4-5, p.16).
text(H, "get_notify", {}, "notify\r\n")
text(H, "set_notify_transport", {"enabled": True}, "notify: transport: true\r\n")
text(H, "set_notify_slot", {"enabled": True}, "notify: slot: true\r\n")
text(H, "set_notify_remote", {"enabled": False}, "notify: remote: false\r\n")
text(H, "set_notify_configuration", {"enabled": True}, "notify: configuration: true\r\n")
text(H, "set_notify_dropped_frames", {"enabled": True}, "notify: dropped frames: true\r\n")
text(H, "set_notify_display_timecode", {"enabled": True}, "notify: display timecode: true\r\n")
text(H, "set_notify_timeline_position", {"enabled": False}, "notify: timeline position: false\r\n")
text(H, "set_notify_playrange", {"enabled": True}, "notify: playrange: true\r\n")
text(H, "set_notify_cache", {"enabled": True}, "notify: cache: true\r\n")
text(H, "set_notify_dynamic_range", {"enabled": True}, "notify: dynamic range: true\r\n")
text(H, "set_notify_slate", {"enabled": True}, "notify: slate: true\r\n")
text(H, "set_notify_clips", {"enabled": True}, "notify: clips: true\r\n")
text(H, "set_notify_disk", {"enabled": True}, "notify: disk: true\r\n")
text(H, "set_notify_device_info", {"enabled": True}, "notify: device info: true\r\n")
text(H, "set_notify_nas", {"enabled": False}, "notify: nas: false\r\n")

# Preview (p.2, p.13).
text(H, "preview_enable", {}, "preview: enable: true\r\n")
text(H, "preview_disable", {}, "preview: enable: false\r\n")

# Playback (p.2-3, p.14).
text(H, "play_default", {}, "play\r\n")
text(H, "play", {"speed": -50}, "play: single clip: true loop: false speed: -50\r\n")
text(H, "play_clip", {"clip_id": 3, "speed": 200, "loop": True, "single_clip": True},
     "play: clip id: 3 speed: 200 loop: true single clip: true\r\n")
text(H, "play_clip_timecode_offset", {"clip_id": 2, "offset": "00:00:05:00"},
     "play: clip id: 2 timecode: +00:00:05:00\r\n")
text(H, "play_timecode", {"timecode": "01:00:00:00"},
     "play: timecode: 01:00:00:00 speed: 100 loop: false single clip: true\r\n")
text(H, "play_timeline_frame", {"frame": 1500, "loop": True},
     "play: timeline: 1500 speed: 100 loop: true single clip: true\r\n")
text(H, "get_playrange", {}, "playrange\r\n")
text(H, "set_playrange_clip", {"clip_id": 5}, "playrange set: clip id: 5\r\n")
text(H, "set_playrange_clips", {"clip_id": 5, "count": 7}, "playrange set: clip id: 5 count: 7\r\n")
text(H, "set_playrange_timecode", {"in": "00:01:00:00", "out": "00:02:00:00"},
     "playrange set: in: 00:01:00:00 out: 00:02:00:00\r\n")
text(H, "set_playrange_frames", {"timeline_in": 0, "timeline_out": 250},
     "playrange set: timeline in: 0 timeline out: 250\r\n")
text(H, "clear_playrange", {}, "playrange clear\r\n")
text(H, "get_play_on_startup", {}, "play on startup\r\n")
text(H, "set_play_on_startup", {"enabled": True}, "play on startup: enable: true\r\n")
text(H, "set_play_on_startup_single_clip", {"single_clip": False}, "play on startup: single clip: false\r\n")
text(H, "get_play_option", {}, "play option\r\n")
text(H, "set_playback_stop_mode", {"mode": "black"}, "play option: stop mode: black\r\n")
text(H, "get_transport_info", {}, "transport info\r\n",
     device_reply="208 transport info:\r\nstatus: play\r\nspeed: 100\r\n\r\n",
     expect_result={"ok": {"kind": "value", "value": {"status": "play", "speed": "100"}}})
text(H, "stop", {}, "stop\r\n", device_reply="111 remote control disabled\r\n",
     expect_result={"error": {"error": "device_error", "code": "111"}})

# Recording (p.3).
text(H, "record", {}, "record\r\n", device_reply="200 ok\r\n", expect_result={"ok": {"kind": "ack"}})
text(H, "record_named", {"name": "Take 4"}, "record: name: Take 4\r\n")
text(H, "record_spill", {}, "record spill\r\n")
text(H, "record_spill_to_slot", {"slot_id": 2}, "record: spill: slot id: 2\r\n",
     device_reply="104 disk full\r\n", expect_result={"error": {"error": "device_error", "code": "104"}})
text(H, "get_spill_order", {}, "spill order\r\n")

# Timeline position (p.5-6, p.15).
text(H, "goto_clip", {"clip_id": 3}, "goto: clip id: 3\r\n")
text(H, "goto_clip_offset", {"offset": 2}, "goto: clip id: +2\r\n")
text(H, "goto_first_or_last_clip", {"which": "end"}, "goto: clip id: end\r\n")
text(H, "goto_clip_edge", {"which": "start"}, "goto: clip: start\r\n")
text(H, "goto_clip_frame", {"frame": 48}, "goto: clip: 48\r\n")
text(H, "goto_clip_frame_offset", {"offset": -10}, "goto: clip: -10\r\n")
text(H, "goto_timeline_edge", {"which": "end"}, "goto: timeline: end\r\n")
text(H, "goto_timeline_frame", {"frame": 900}, "goto: timeline: 900\r\n")
text(H, "goto_timeline_frame_offset", {"offset": 25}, "goto: timeline: +25\r\n")
text(H, "goto_timecode", {"timecode": "00:10:00:00"}, "goto: timecode: 00:10:00:00\r\n")
text(H, "goto_timecode_forward", {"duration": "00:00:10:00"}, "goto: timecode: +00:00:10:00\r\n")
text(H, "goto_timecode_backward", {"duration": "00:00:02:12"}, "goto: timecode: -00:00:02:12\r\n")
text(H, "goto_slot", {"slot_id": 2}, "goto: slot id: 2\r\n")
text(H, "goto_clip_and_clip_frame", {"clip_id": 4, "frame": 100}, "goto: clip id: 4 clip: 100\r\n")
text(H, "goto_clip_and_timeline_frame", {"clip_id": 4, "frame": 2000}, "goto: clip id: 4 timeline: 2000\r\n")
text(H, "goto_clip_and_timecode", {"clip_id": 4, "timecode": "00:00:03:00"},
     "goto: clip id: 4 timecode: 00:00:03:00\r\n")

# Jog and shuttle (p.6).
text(H, "jog_to_timecode", {"timecode": "00:00:30:00"}, "jog: timecode: 00:00:30:00\r\n")
text(H, "jog_forward", {"duration": "00:00:00:01"}, "jog: timecode: +00:00:00:01\r\n")
text(H, "jog_backward", {"duration": "00:00:00:05"}, "jog: timecode: -00:00:00:05\r\n")
text(H, "shuttle", {"speed": -400}, "shuttle: speed: -400\r\n")

# Timeline clips (p.3-4, p.18-19, p.23-24).
text(H, "get_clip_count", {}, "clips count\r\n",
     device_reply="214 clips count:\r\nclip count: 7\r\n\r\n", expect_result={"ok": {"kind": "value", "value": "7"}})
text(H, "get_clips", {}, "clips get\r\n")
text(H, "get_clip", {"clip_id": 2}, "clips get: clip id: 2\r\n")
text(H, "get_clips_range", {"clip_id": 2, "count": 3}, "clips get: clip id: 2 count: 3\r\n")
text(H, "get_clips_version", {"version": 3}, "clips get: version: 3\r\n")
text(H, "add_clip", {"name": "folder1/HyperDeck_0001.mp4"}, "clips add: name: folder1/HyperDeck_0001.mp4\r\n")
text(H, "insert_clip", {"clip_id": 2, "name": "Intro.mov"}, "clips add: clip id: 2 name: Intro.mov\r\n")
text(H, "add_clip_timecode_range", {"in": "00:00:01:00", "out": "00:00:04:00", "name": "Intro.mov"},
     "clips add: in: 00:00:01:00 out: 00:00:04:00 name: Intro.mov\r\n")
text(H, "add_clip_frame_range", {"frame_in": 25, "frame_out": 100, "name": "Intro.mov"},
     "clips add: frame in: 25 frame out: 100 name: Intro.mov\r\n")
text(H, "remove_clip", {"clip_id": 3}, "clips remove: clip id: 3\r\n")
text(H, "clear_clips", {}, "clips clear\r\n")
text(H, "rebuild_clips", {}, "clips rebuild\r\n")
text(H, "get_current_clip_info", {}, "clip info\r\n")
text(H, "get_clip_info", {"clip_id": 6}, "clip info: clip id: 6\r\n")
text(H, "get_clip_info_by_name", {"name": "Intro.mov"}, "clip info: name: Intro.mov\r\n")

# Disks, slots and external drives (p.2, p.4, p.17-18, p.23).
text(H, "disk_list", {}, "disk list\r\n")
text(H, "disk_list_slot", {"slot_id": 1}, "disk list: slot id: 1\r\n")
text(H, "disk_list_device", {"device": "network"}, "disk list: device: network\r\n")
text(H, "get_slot_info", {}, "slot info\r\n")
text(H, "get_slot_info_slot", {"slot_id": 2}, "slot info: slot id: 2\r\n")
text(H, "get_slot_info_device", {"device": "network"}, "slot info: device: network\r\n")
text(H, "select_slot", {"slot_id": 2}, "slot select: slot id: 2\r\n")
text(H, "select_slot_device", {"device": "usb1"}, "slot select: device: usb1\r\n")
text(H, "select_video_format", {"video_format": "1080p2997"}, "slot select: video format: 1080p2997\r\n")
text(H, "select_slot_video_format", {"slot_id": 2, "video_format": "NTSC"},
     "slot select: slot id: 2 video format: NTSC\r\n")
text(H, "unblock_slot", {}, "slot unblock\r\n")
text(H, "unblock_slot_id", {"slot_id": 1}, "slot unblock: slot id: 1\r\n")
text(H, "unblock_device", {"device": "usb1"}, "slot unblock: device: usb1\r\n")
text(H, "list_external_drives", {}, "external drive list\r\n")
text(H, "select_external_drive", {"device": "usb1"}, "external drive select: device: usb1\r\n")
text(H, "get_selected_external_drive", {}, "external drive selected\r\n")
text(H, "get_cache_info", {}, "cache info\r\n")

# Formatting (p.7).
text(H, "format_prepare_slot", {"slot_id": 1, "filesystem": "exFAT", "name": "Show Day 1"},
     "format: slot id: 1 prepare: exFAT name: Show Day 1\r\n")
text(H, "format_prepare_current", {"filesystem": "HFS+", "name": "Backup"}, "format: prepare: HFS+ name: Backup\r\n")
text(H, "format_prepare_device", {"device": "usb1", "filesystem": "exFAT", "name": "USB"},
     "format: device: usb1 prepare: exFAT name: USB\r\n")
text(H, "format_confirm", {"token": "4f3c2a"}, "format: confirm: 4f3c2a\r\n",
     device_reply="161 invalid token\r\n", expect_result={"error": {"error": "device_error", "code": "161"}})

# Dynamic range (p.4).
text(H, "get_dynamic_range", {}, "dynamic range\r\n")
text(H, "set_playback_dynamic_range", {"mode": "off"}, "dynamic range: playback override: off\r\n")
text(H, "set_record_dynamic_range", {"mode": "ST2084_1000"}, "dynamic range: record override: ST2084_1000\r\n")

# Configuration (p.6-7, p.22-23).
text(H, "get_configuration", {}, "configuration\r\n")
text(H, "set_video_input", {"input": "4xSDI"}, "configuration: video input: 4xSDI\r\n")
text(H, "set_audio_input", {"input": "XLR"}, "configuration: audio input: XLR\r\n")
text(H, "set_file_format", {"format": "QuickTimeProResHQ"}, "configuration: file format: QuickTimeProResHQ\r\n",
     device_reply="213 deck rebooting\r\n", expect_result={"ok": {"kind": "ack"}})
text(H, "set_audio_codec", {"codec": "AAC"}, "configuration: audio codec: AAC\r\n")
text(H, "set_timecode_input", {"source": "embedded"}, "configuration: timecode input: embedded\r\n")
text(H, "set_timecode_output", {"mode": "timeline"}, "configuration: timecode output: timeline\r\n")
text(H, "set_timecode_preference", {"preference": "dropframe"}, "configuration: timecode preference: dropframe\r\n")
text(H, "set_timecode_preset", {"timecode": "10:00:00:00"}, "configuration: timecode preset: 10:00:00:00\r\n")
text(H, "set_audio_input_channels", {"channels": 8}, "configuration: audio input channels: 8\r\n")
text(H, "set_record_trigger", {"trigger": "recordbit"}, "configuration: record trigger: recordbit\r\n")
text(H, "set_record_prefix", {"prefix": "Cam A"}, "configuration: record prefix: Cam A\r\n")
text(H, "set_record_cache", {"enabled": True}, "configuration: record cache: true\r\n")
text(H, "set_append_timestamp", {"enabled": False}, "configuration: append timestamp: false\r\n")
text(H, "set_usb_spill", {"enabled": True}, "configuration: usb spill: true\r\n")
text(H, "set_reference_source", {"source": "external"}, "configuration: reference source: external\r\n")
text(H, "set_genlock_input_resync", {"enabled": True}, "configuration: genlock input resync: true\r\n")
text(H, "set_default_standard", {"video_format": "2160p59.94"}, "configuration: default standard: 2160p59.94\r\n")
text(H, "set_xlr_input_type", {"xlr_id": 2, "type": "mic"}, "configuration: xlr input id: 2 xlr type: mic\r\n")
text(H, "set_xlr_mapping", {"channel": 1}, "configuration: xlr mapping: 1\r\n")
text(H, "unmap_xlr", {}, "configuration: xlr mapping: none\r\n")
text(H, "set_rca_mapping", {"channel": 9}, "configuration: rca mapping: 9\r\n")
text(H, "unmap_rca", {}, "configuration: rca mapping: none\r\n")

# Digital slate: queries single-line (p.7), sets multiline only (p.8-9).
text(H, "get_slate_clips", {}, "slate clips\r\n")
text(H, "get_slate_project", {}, "slate project\r\n")
text(H, "get_slate_lens", {}, "slate lens\r\n")
text(H, "set_slate_reel", {"reel": 12}, "slate clips:\r\nreel: 12\r\n\r\n")
text(H, "set_slate_scene_id", {"scene_id": "42A"}, "slate clips:\r\nscene id: 42A\r\n\r\n")
text(H, "set_slate_shot_type", {"shot_type": "MCU"}, "slate clips:\r\nshot type: MCU\r\n\r\n")
text(H, "set_slate_take", {"take": 3}, "slate clips:\r\ntake: 3\r\n\r\n")
text(H, "set_slate_take_scenario", {"scenario": "VFX"}, "slate clips:\r\ntake scenario: VFX\r\n\r\n")
text(H, "set_slate_take_auto_inc", {"enabled": True}, "slate clips:\r\ntake auto inc: true\r\n\r\n")
text(H, "set_slate_good_take", {"good": True}, "slate clips:\r\ngood take: true\r\n\r\n")
text(H, "set_slate_environment", {"environment": "exterior"}, "slate clips:\r\nenvironment: exterior\r\n\r\n")
text(H, "set_slate_day_night", {"day_night": "night"}, "slate clips:\r\nday night: night\r\n\r\n")
text(H, "set_slate_project_name", {"name": "Evening News"}, "slate project:\r\nproject name: Evening News\r\n\r\n")
text(H, "set_slate_camera", {"camera": "A"}, "slate project:\r\ncamera: A\r\n\r\n")
text(H, "set_slate_director", {"name": ""}, "slate project:\r\ndirector: \r\n\r\n")
text(H, "set_slate_camera_operator", {"name": "Sam"}, "slate project:\r\ncamera operator: Sam\r\n\r\n")
text(H, "set_slate_lens_type", {"lens_type": "Zeiss CP.3"}, "slate lens:\r\nlens type: Zeiss CP.3\r\n\r\n")
text(H, "set_slate_iris", {"iris": "f/2.8"}, "slate lens:\r\niris: f/2.8\r\n\r\n")
text(H, "set_slate_focal_length", {"focal_length": "50mm"}, "slate lens:\r\nfocal length: 50mm\r\n\r\n")
text(H, "set_slate_distance", {"distance": "3m"}, "slate lens:\r\ndistance: 3m\r\n\r\n")
text(H, "set_slate_filter", {"filter": "ND 0.6"}, "slate lens:\r\nfilter: ND 0.6\r\n\r\n")

# Network storage (p.7, p.9, p.25).
text(H, "list_nas", {}, "nas list\r\n")
text(H, "list_nas_discovered", {}, "nas discovered\r\n")
text(H, "get_nas_selected", {}, "nas selected\r\n")
text(H, "deselect_nas", {}, "nas deselect\r\n")
text(H, "add_nas", {"url": "smb://CloudStore80.local/Studio1"}, "nas add:\r\nurl: smb://CloudStore80.local/Studio1\r\n\r\n")
text(H, "add_nas_with_credentials",
     {"url": "smb://192.168.1.1/Main", "username": "user1234", "password": "Password1234"},
     "nas add:\r\nurl: smb://192.168.1.1/Main\r\nusername: user1234\r\npassword: Password1234\r\n\r\n")
text(H, "remove_nas", {"url": "smb://192.168.1.1/Main"}, "nas remove:\r\nurl: smb://192.168.1.1/Main\r\n\r\n")
text(H, "select_nas", {"url": "smb://192.168.1.1/Main"}, "nas select:\r\nurl: smb://192.168.1.1/Main\r\n\r\n")


# ── HyperDeck telemetry ──────────────────────────────────────────────────
# One notify line per kind on connect (the first is shown; the rest are queued
# behind its reply), then device info, transport info, slot info, remote and
# configuration once. The 5xx pushes carry the same fields as the 2xx replies
# (p.11-23).
telemetry(H, "transport", expect_connect_wire=["notify: transport: true\r\n"],
          inbound="508 transport info:\r\nstatus: play\r\nspeed: 100\r\nslot id: 1\r\nslot name: SD1\r\n"
          "device name: sd1\r\nclip id: 3\r\nsingle clip: false\r\ndisplay timecode: 01:00:10:00\r\n"
          "timecode: 00:00:10:00\r\nvideo format: 1080p25\r\nloop: true\r\ntimeline: 0\r\n"
          "input video format: 1080p25\r\ndynamic range: Rec709\r\nreference locked: true\r\n\r\n",
          expect_state={"transport": {"status": "play", "speed": 100, "slot": 1, "slot_name": "SD1",
                                      "device_name": "sd1", "clip": 3, "single_clip": False,
                                      "display_timecode": "01:00:10:00", "timecode": "00:00:10:00",
                                      "video_format": "1080p25", "loop": True, "timeline": 0,
                                      "input_video_format": "1080p25", "dynamic_range": "Rec709",
                                      "reference_locked": True}})
# "none" for a clip or slot id is not a number, so those are not assigned.
telemetry(H, "transport-none", inbound="208 transport info:\r\nstatus: stopped\r\nslot id: none\r\n"
          "clip id: none\r\nspeed: 0\r\n\r\n",
          expect_state={"transport": {"status": "stopped", "speed": 0}})
telemetry(H, "connection", inbound="500 connection info:\r\nprotocol version: 1.11\r\nmodel: HyperDeck Extreme 8K HDR\r\n\r\n",
          expect_state={"device": {"protocol_version": "1.11", "model": "HyperDeck Extreme 8K HDR"}})
telemetry(H, "device", inbound="204 device info:\r\nprotocol version: 1.11\r\nmodel: HyperDeck Studio 4K Pro\r\n"
          "unique id: 7c2e0d1a\r\nslot count: 3\r\nsoftware version: 8.4\r\nname: Deck A\r\n\r\n",
          expect_state={"device": {"protocol_version": "1.11", "model": "HyperDeck Studio 4K Pro",
                                   "unique_id": "7c2e0d1a", "slot_count": 3, "software_version": "8.4",
                                   "name": "Deck A"}})
telemetry(H, "slot", inbound="502 slot info:\r\nslot id: 2\r\nslot name: SD2\r\ndevice name: sd2\r\n"
          "status: mounted\r\nvolume name: Show\r\nrecording time: 3600\r\nvideo format: 1080p50\r\n"
          "blocked: false\r\nremaining size: 1000000000\r\ntotal size: 64000000000\r\n\r\n",
          expect_state={"slot_info": {"slot_id": 2, "slot_name": "SD2", "device_name": "sd2",
                                      "status": "mounted", "volume_name": "Show", "recording_time": 3600,
                                      "video_format": "1080p50", "blocked": False,
                                      "remaining_size": 1000000000, "total_size": 64000000000}})
telemetry(H, "remote", inbound="510 remote info:\r\nenabled: false\r\noverride: true\r\n\r\n",
          expect_state={"remote": {"enabled": False, "override": True}})
telemetry(H, "configuration", inbound="511 configuration:\r\naudio input: XLR\r\naudio mapping: 0\r\n"
          "video input: SDI\r\nfile format: QuickTimeProResHQ\r\naudio codec: PCM\r\ntimecode input: embedded\r\n"
          "timecode output: timeline\r\ntimecode preference: default\r\ntimecode preset: 00:00:00:00\r\n"
          "audio input channels: 2\r\nrecord trigger: none\r\nrecord prefix: Cam A\r\nrecord cache: false\r\n"
          "append timestamp: true\r\nreference source: auto\r\ngenlock input resync: false\r\nusb spill: true\r\n"
          "default standard: 1080p25\r\nxlr mapping: none\r\nrca mapping: 9\r\nxlr input id: 1\r\n"
          "xlr type: line\r\n\r\n",
          expect_state={"configuration": {
              "audio_input": "XLR", "audio_mapping": 0, "video_input": "SDI", "file_format": "QuickTimeProResHQ",
              "audio_codec": "PCM", "timecode_input": "embedded", "timecode_output": "timeline",
              "timecode_preference": "default", "timecode_preset": "00:00:00:00", "audio_input_channels": 2,
              "record_trigger": "none", "record_prefix": "Cam A", "record_cache": False,
              "append_timestamp": True, "reference_source": "auto", "genlock_input_resync": False,
              "usb_spill": True, "default_standard": "1080p25", "xlr_mapping": "none", "rca_mapping": "9",
              "xlr_input_id": 1, "xlr_type": "line"}})
telemetry(H, "notify", inbound="209 notify:\r\ntransport: true\r\nslot: true\r\nremote: false\r\n"
          "configuration: true\r\ndropped frames: false\r\ndisplay timecode: false\r\ntimeline position: false\r\n"
          "playrange: false\r\ncache: false\r\ndynamic range: false\r\nslate: false\r\nclips: false\r\n"
          "disk: false\r\ndevice info: false\r\nnas: true\r\n\r\n",
          expect_state={"notify": {"transport": True, "slot": True, "remote": False, "configuration": True,
                                   "dropped_frames": False, "display_timecode": False,
                                   "timeline_position": False, "playrange": False, "cache": False,
                                   "dynamic_range": False, "slate": False, "clips": False, "disk": False,
                                   "device_info": False, "nas": True}})
telemetry(H, "clip-count", inbound="214 clips count:\r\nclip count: 12\r\n\r\n",
          expect_state={"timeline": {"clip_count": 12}})
telemetry(H, "clips-info", inbound="205 clips info:\r\nclip count: 2\r\n1: Intro.mov 00:00:00:00 00:00:10:00\r\n"
          "2: Outro.mov 00:00:10:00 00:00:05:00\r\n\r\n",
          expect_state={"timeline": {"clip_count": 2}})
