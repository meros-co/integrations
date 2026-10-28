# Magewell Pro Convert (magewell-proconvert): one vector per command, a second
# for each command both APIs share (on an IP decoder), and telemetry vectors
# for the two summaries. Requests are as the Encoder API, Decoder API V1.3 and
# IP Decoder API V1.2 give them: GET /mwapi?method=<name>&<args> with values
# percent-encoded, or JSON under /api.
MW = "magewell-proconvert"


def _mw(command, input, query, model=None, **extra):
    v = {"spec": MW, "command": command, "input": input,
         "expect_request": {"method": "GET", "target": "/mwapi?method=" + query}, **extra}
    if model:
        v["model"] = model
    V.append(v)


def _ip(command, input, method, target, body=None, file=None, **extra):
    request = {"method": method, "target": target}
    if body is not None:
        request["body"] = body
    v = {"spec": MW, "command": command, "input": input, "model": "ip-to-hdmi",
         "expect_request": request, **extra}
    if file:
        v["file"] = file
    V.append(v)


# Shared by both APIs.
_mw("ping", {}, "ping", http_reply={"status": 200, "body": '{"status":0}'},
    expect_result={"ok": {"kind": "ack"}})
_ip("ping", {}, "GET", "/api/ping", file="ping-ip")
_mw("reboot", {}, "reboot")
_ip("reboot", {}, "POST", "/api/reboot", file="reboot-ip")
_mw("get_summary", {}, "get-summary-info",
    http_reply={"status": 200, "body": '{"status":0,"device":{"name":"Pro Convert"}}'},
    expect_result={"ok": {"kind": "value", "value": {"status": 0, "device": {"name": "Pro Convert"}}}})
_ip("get_summary", {}, "GET", "/api/system/summary", file="get_summary-ip")
_mw("get_signal_info", {}, "get-signal-info")
_ip("get_signal_info", {}, "GET", "/api/signal/info", file="get_signal_info-ip")
_mw("set_audio_gain", {"gain": -6.5}, "set-audio-config&gain=-6.50", model="ndi-to-hdmi")
_ip("set_audio_gain", {"gain": -6.5}, "POST", "/api/audio/config/set", '{"gain":-6.50}',
    file="set_audio_gain-ip")

# Encoders (Encoder API sections 2, 5-9).
_mw("get_caps", {}, "get-caps")
_mw("get_video_config", {}, "get-video-config")
_mw("get_ndi_config", {}, "get-ndi-config")
_mw("get_ndi_sources", {}, "get-ndi-sources")
_mw("get_tally_config", {}, "get-tally")
_mw("set_ndi_enabled", {"enabled": False}, "set-ndi-config&enable=false")
_mw("set_ndi_source_name", {"name": "#%board-id% (Stage)"},
    "set-ndi-config&source-name=%23%25board-id%25%20%28Stage%29")
_mw("set_ndi_groups", {"groups": "public,studio"}, "set-ndi-config&group-name=public%2Cstudio")
_mw("set_ndi_transport", {"transport": "rudp"},
    "set-ndi-config&enable-mcast=false&enable-rudp=true&enable-tcp=false&enable-udp=false")
_mw("set_ndi_multicast", {"address": "239.255.0.0", "netmask": "255.255.0.0", "ttl": 4},
    "set-ndi-config&mcast-addr=239.255.0.0&mcast-mask=255.255.0.0&mcast-ttl=4")
_mw("set_ndi_discovery", {"enabled": True, "servers": "10.0.0.5"},
    "set-ndi-config&enable-discovery=true&discovery-server=10.0.0.5")
_mw("set_ndi_failover", {"enabled": True, "ndi_name": "B (Cam)", "ip_address": "10.0.0.9:5963"},
    "set-ndi-config&enable-fail-over=true&fail-over-ndi-name=B%20%28Cam%29&fail-over-ip-addr=10.0.0.9%3A5963")
_mw("set_ndi_web_control", {"enabled": True}, "set-ndi-config&enable-web-control=true")
_mw("set_ndi_ptz_control", {"enabled": False}, "set-ndi-config&enable-ptz-control=false")
_mw("set_reference_level", {"level": "ebu"}, "set-ndi-config&reference-level=14")
_mw("set_bitrate_ratio", {"percent": 150}, "set-video-config&bit-rate-ratio=150")
_mw("set_brightness", {"value": -20}, "set-video-config&brightness=-20")
_mw("set_contrast", {"value": 120}, "set-video-config&contrast=120")
_mw("set_hue", {"value": 15}, "set-video-config&hue=15")
_mw("set_saturation", {"value": 0}, "set-video-config&saturation=0")
_mw("set_deinterlace", {"mode": "top-field"}, "set-video-config&deinterlace=top-field")
_mw("set_output_flip", {"enabled": True}, "set-video-config&out-flip=true")
_mw("set_output_mirror", {"enabled": True}, "set-video-config&out-mirror=true")
_mw("set_output_resolution", {"width": 1280, "height": 720},
    "set-video-config&out-raw-resolution=false&out-cx=1280&out-cy=720")
_mw("set_output_follows_input", {"enabled": True}, "set-video-config&out-raw-resolution=true")
_mw("set_ext_tally", {"enabled": True}, "set-tally&ext-tally=true",
    http_reply={"status": 200, "body": '{"status":7}'},
    expect_result={"error": {"error": "device_error", "code": "7"}})

# NDI decoders (Decoder API V1.3 sections 7-11).
_mw("list_presets", {}, "list-channels")
_mw("get_current_source", {}, "get-channel")
_mw("select_ndi_source", {"name": "STUDIO (Camera 1)"},
    "set-channel&ndi-name=true&name=STUDIO%20%28Camera%201%29")
_mw("select_preset", {"name": "Stage left"}, "set-channel&ndi-name=false&name=Stage%20left")
_mw("add_preset", {"name": "Cam 3", "url": "ntkndi://ndi?name=PC (Cam 3)"},
    "add-channel&name=Cam%203&url=ntkndi%3A%2F%2Fndi%3Fname%3DPC%20%28Cam%203%29")
_mw("delete_preset", {"name": "Cam 3"}, "del-channel&name=Cam%203")
_mw("set_hdmi_output", {"enabled": False}, "set-hdmi-output&enabled=false")
_mw("get_video_modes", {}, "get-supported-video-modes")
_mw("set_video_mode", {"width": 1920, "height": 1080, "interlaced": False, "field_rate": 5994, "aspect_ratio": 1.78},
    "set-video-mode&width=1920&height=1080&interlaced=false&field-rate=5994&aspect-ratio=1.78")
_mw("get_audio_config", {}, "get-audio-config")
_mw("set_overlay", {"overlay": "vu-meter", "visible": True}, "set-video-config&show-vu-meter=true")
_mw("set_switch_mode", {"mode": "keep-last"}, "set-video-config&switch-mode=keep-last")
_mw("set_aspect_conversion", {"mode": "windowbox"}, "set-video-config&ar-convert-mode=windowbox")
_mw("set_decoder_flip", {"horizontal": True}, "set-video-config&h-flip=true&v-flip=false")
_mw("set_buffer_duration", {"ms": 40}, "set-playback-config&buffer-duration=40")
_mw("set_decoder_ndi_discovery", {"enabled": False}, "set-ndi-config&enable-discovery=false&discovery-server=")
_mw("set_decoder_ndi_group", {"group": "studio"}, "set-ndi-config&group-name=studio")

# IP decoders (IP Decoder API V1.2 sections 3, 6, 7).
_ip("get_sources", {}, "GET", "/api/source/list?type=all")
_ip("select_source", {"id": 12}, "POST", "/api/source/select", '{"id":12}')
_ip("restart_source", {}, "POST", "/api/source/restart", "{}")
_ip("list_profiles", {}, "GET", "/api/profile/list")
_ip("select_profile", {"id": 2}, "POST", "/api/profile/select", '{"id":2}')
_ip("select_screen_source", {"profile": 2, "screen": 1, "source": 12}, "POST", "/api/profile/screen/select",
    '{"id":2,"screen-index":1,"source-id":12}')
_ip("ptz_move", {"pan": -0.5, "tilt": 0.25}, "POST", "/api/ptz/move", '{"pan":-0.50,"tilt":0.25}')
_ip("ptz_zoom", {"speed": 1.0}, "POST", "/api/ptz/zoom/set", '{"speed":1.00}')
_ip("ptz_focus", {"speed": 0.0}, "POST", "/api/ptz/focus/set", '{"speed":0.00}')
_ip("ptz_auto_focus", {}, "POST", "/api/ptz/focus/auto", "{}")
_ip("ptz_recall_preset", {"number": 3}, "POST", "/api/ptz/preset/recall", '{"number":3}')
_ip("ptz_store_preset", {"number": 3}, "POST", "/api/ptz/preset/store", '{"number":3}')

# The summaries (Encoder API section 5.1, IP Decoder API section 2).
telemetry(MW, "summary", inbound_http={
    "path": "/mwapi?method=get-summary-info",
    "body": json.dumps({"status": 0, "device": {"name": "Pro Convert", "model": "HDMI 4K Plus",
                                                "serial-no": "B401180706020", "fw-version": "1.1.72",
                                                "input-state": "1920x1080p60", "core-temp": 61.5, "up-time": 3600},
                        "ndi": {"name": "#00 (B401180706020)", "num-clients": 2, "tally-preview": False,
                                "tally-program": True, "video-width": 1920, "video-height": 1080,
                                "video-scan": "progressive", "video-field-rate": 60, "video-bit-rate": 125000}})},
    expect_state={"device": {"model": "HDMI 4K Plus", "name": "Pro Convert", "serial": "B401180706020",
                             "firmware": "1.1.72", "temperature": 61.5, "uptime": 3600,
                             "input_state": "1920x1080p60"},
                  "ndi": {"name": "#00 (B401180706020)", "clients": 2, "video_kbps": 125000,
                          "video": "1920x1080 progressive 60.00"},
                  "tally": {"preview": False, "program": True}})
telemetry(MW, "summary-failed", inbound_http={
    "path": "/mwapi?method=get-summary-info", "body": '{"status":37}'}, expect_state={})
V.append({"spec": MW, "telemetry": "summary-ip", "settings": {}, "inbound_http": {
    "path": "/api/system/summary",
    "body": json.dumps({"status": 0, "product-name": "Pro Convert IP to HDMI", "device-name": "Lobby",
                        "serial-number": "A506220808450", "firmware-ver": "1.2.18", "hdmi-state": 1,
                        "profile": {"streams": [{"name": "CAM (1)", "video": {"kbps": 8000},
                                                 "extra": {"tally-preview": True, "tally-program": False}}]}})},
    "expect_state": {"device": {"model": "Pro Convert IP to HDMI", "name": "Lobby", "serial": "A506220808450",
                                "firmware": "1.2.18", "output_state": "1"},
                     "ndi": {"name": "CAM (1)", "video_kbps": 8000},
                     "tally": {"preview": True, "program": False}}})


# ── Full control: the rest of each documented setting, and its read-back ──
# Encoders (Encoder API sections 7, 9, PTZ, EDID, Universal Interfaces,
# Network).
_mw("set_ndi_vendor", {"name": "Magewell", "id": "01234567-0123"},
    "set-ndi-config&vendor-name=Magewell&vendor-id=01234567-0123")
_mw("set_video_auto", {"setting": "output-sat-range", "enabled": False}, "set-video-config&out-auto-sat-range=false")
_mw("set_input_color_format", {"format": "rgb"}, "set-video-config&in-auto-color-fmt=false&in-color-fmt=rgb")
_mw("set_input_quant_range", {"range": "limited"}, "set-video-config&in-auto-quant-range=false&in-quant-range=limited")
_mw("set_input_aspect", {"x": 4, "y": 3}, "set-video-config&in-auto-aspect=false&in-aspect-x=4&in-aspect-y=3")
_mw("set_output_aspect", {"x": 16, "y": 9}, "set-video-config&out-auto-aspect=false&out-aspect-x=16&out-aspect-y=9")
_mw("set_output_color_format", {"format": "bt.709"}, "set-video-config&out-auto-color-fmt=false&out-color-fmt=bt.709")
_mw("set_output_sat_range", {"range": "extended"}, "set-video-config&out-auto-sat-range=false&out-sat-range=extended")
_mw("set_output_quant_range", {"range": "full"}, "set-video-config&out-auto-quant-range=false&out-quant-range=full")
_mw("set_frame_rate_conversion", {"rate": "half"}, "set-video-config&out-fr-convertion=half")
_mw("set_encoder_aspect_conversion", {"mode": "padding"}, "set-video-config&ar-convertion=padding")
_mw("set_low_res_full_frame_rate", {"enabled": True}, "set-video-config&low-res-full-fr=true")
_mw("reset_video_config", {}, "reset-video-config")
_mw("get_ptz_config", {}, "get-ptz-config")
_mw("set_ptz_none", {}, "set-ptz-config&proto=none")
_mw("set_ptz_serial", {"protocol": "visca", "camera": 2, "baudrate": "4800", "invert_pan": True, "invert_tilt": True},
    "set-ptz-config&proto=visca&index=2&baudrate=4800&invert-pan=true&invert-tilt=true")
_mw("set_ptz_visca_udp", {"camera": 2, "address": "10.10.10.123", "port": 52381},
    "set-ptz-config&proto=visca-udp&index=2&ip-addr=10.10.10.123&port=52381&visca-msg-hdr=false"
    "&invert-pan=false&invert-tilt=false")
_mw("set_ptz_visca_bridge", {"baudrate": "4800", "port": 1}, "set-ptz-config&proto=visca-udp2rs232&baudrate=4800&port=1")
_mw("set_ptz_limits", {"pan_left": -2448, "pan_center": 0, "pan_right": 2448, "tilt_top": 1280, "tilt_center": 0,
                       "tilt_bottom": -368, "zoom_out": 16384, "focus_near": 0, "focus_far": 2935},
    "set-ptz-config&focus-near-limit=0&focus-far-limit=2935&pan-left-limit=-2448&pan-center=0&pan-right-limit=2448"
    "&tilt-top-limit=1280&tilt-center=0&tilt-bottom-limit=-368&zoom-out-limit=16384")
_mw("apply_ptz_config", {}, "arrange-ptz-cameras")
_mw("get_edid_config", {}, "get-edid-config")
_mw("set_edid_option", {"option": "keep-last", "enabled": False}, "set-edid-config&keep-last=false")
_mw("reset_edid", {}, "set-default-edid")
_mw("get_auto_reboot", {}, "get-auto-reboot")
_mw("set_auto_reboot", {"enabled": True, "days": 2, "hour": 12, "minute": 21},
    "set-auto-reboot&enable=true&week-flags=2&hour=12&min=21")
_mw("get_ntp_server", {}, "get-ntp-server")
_mw("set_ntp_server", {"server": "ntp.aliyun.com"}, "set-ntp-server&ntp-server=ntp.aliyun.com")
_mw("get_net_access", {}, "get-net-access")
_mw("set_ssdp", {"enabled": True}, "set-net-access&use-ssdp=true")
_mw("get_network_status", {}, "get-eth-status")
_mw("set_device_name", {"name": "Stage Left"}, "set-eth-config&name=Stage%20Left")
_ip("set_device_name", {"name": "Magewell-1"}, "POST", "/api/system/set-device-name", '{"name":"Magewell-1"}',
    file="set_device_name-ip")
_mw("set_network_dhcp", {}, "set-eth-config&dhcp=true")
_mw("set_network_static", {"address": "192.168.1.90", "netmask": "255.255.255.0", "gateway": "192.168.1.1",
                           "dns": "192.168.1.1"},
    "set-eth-config&dhcp=false&addr=192.168.1.90&mask=255.255.255.0&gw-addr=192.168.1.1&dns-addr=192.168.1.1")

# NDI decoders (Decoder API V1.3 sections 7, 8, 10, 11).
_D = "ndi-to-hdmi"
_mw("modify_preset", {"name": "RTP", "new_name": "RTP 2", "url": "rtp://224.1.2.3:4000"},
    "modify-channel&name=RTP&new-name=RTP%202&url=rtp%3A%2F%2F224.1.2.3%3A4000", model=_D)
_mw("clear_presets", {}, "clear-channels", model=_D)
_mw("get_hdmi_output", {}, "get-hdmi-output", model=_D)
_mw("get_video_format", {}, "get-video-format", model=_D)
_mw("set_output_format", {"color_format": "rgb", "quant_range": "full"},
    "set-video-format&color-format=rgb&quant-range=full", model=_D)
_ip("set_output_format", {"color_format": "yuv422"}, "POST", "/api/video/config/set",
    '{"color-format":"yuv422","quant-range":"limited"}', file="set_output_format-ip")
_mw("set_audio_sample_rate", {"rate": "44100"}, "set-audio-config&sample-rate=44100", model=_D)
_ip("set_audio_sample_rate", {"rate": "follow"}, "POST", "/api/audio/config/set", '{"sample-rate":0}',
    file="set_audio_sample_rate-ip")
_mw("set_audio_channels", {"channels": "2"}, "set-audio-config&channels=2", model=_D)
_ip("set_audio_channels", {"channels": "follow"}, "POST", "/api/audio/config/set", '{"channels":0}',
    file="set_audio_channels-ip")
_mw("set_audio_channel_map", {"output": 1, "source": 5}, "set-audio-config&ch0=4", model=_D)
_mw("set_audio_convert_mode", {"mode": "ebu"}, "set-audio-config&convert-mode=ebu", model=_D)
_ip("set_audio_convert_mode", {"mode": "ebu"}, "POST", "/api/audio/config/set", '{"convert-mode":1}',
    file="set_audio_convert_mode-ip")
_mw("set_audio_check_pts", {"enabled": True}, "set-audio-config&check-pts=true", model=_D)
_ip("set_audio_check_pts", {"enabled": False}, "POST", "/api/audio/config/set", '{"check-pts":false}',
    file="set_audio_check_pts-ip")
_mw("set_vu_meter_mode", {"mode": "post-gain-dbfs"}, "set-video-config&vu-meter-mode=post-gain-dbfs", model=_D)
_mw("set_safe_area", {"mode": "4:3"}, "set-video-config&safe-area-mode=4%3A3", model=_D)
_ip("set_safe_area", {"mode": "80%"}, "POST", "/api/video/config/set", '{"safe-area":2}', file="set_safe_area-ip")
_mw("set_ident", {"mode": "ident-text", "text": "Screen 2"}, "set-video-config&ident-mode=ident-text&ident-text=Screen%202",
    model=_D)
_mw("set_reset_source_on_boot", {"enabled": True}, "set-video-config&reset-source-on-boot=true", model=_D)
_mw("set_decoder_deinterlace", {"mode": "weave"}, "set-video-config&deinterlace-mode=weave", model=_D)
_mw("set_decoder_input_color_format", {"format": "bt.709"},
    "set-video-config&in-auto-color-fmt=false&in-color-fmt=bt.709", model=_D)
_mw("set_decoder_input_color_auto", {"enabled": True}, "set-video-config&in-auto-color-fmt=true", model=_D)
_mw("set_follow_input", {"enabled": False}, "set-video-config&follow-input-mode=false", model=_D)
_ip("set_follow_input", {"enabled": True}, "POST", "/api/video/config/set", '{"follow-input":true}',
    file="set_follow_input-ip")
_mw("get_playback_config", {}, "get-playback-config", model=_D)
_mw("set_decoder_ndi_transport", {"transport": "multicast"},
    "set-ndi-config&enable-mcast=true&enable-rudp=false&enable-tcp=false&enable-udp=false", model=_D)
_mw("set_decoder_low_bandwidth", {"enabled": True}, "set-ndi-config&low-bandwidth=true", model=_D)
_ip("set_decoder_low_bandwidth", {"enabled": False}, "POST", "/api/settings/ndi/set", '{"low-bw":false}',
    file="set_decoder_low_bandwidth-ip")
_mw("set_decoder_mcast_subnets", {"subnets": "192.168.1.0/24,10.0.0.0/8"},
    "set-ndi-config&mcast-subnets=192.168.1.0%2F24%2C10.0.0.0%2F8", model=_D)
_ip("set_decoder_mcast_subnets", {"subnets": ""}, "POST", "/api/settings/ndi/set", '{"mcast-subnets":""}',
    file="set_decoder_mcast_subnets-ip")
_mw("set_decoder_ignore_hx_pts", {"enabled": True}, "set-ndi-config&ignore-ndi-hx-video-pts=true", model=_D)
_mw("set_decoder_timecode_first", {"enabled": False}, "set-ndi-config&use-timecode-first=false", model=_D)

# Commands both decoder APIs share, on an IP decoder.
_ip("get_video_config", {}, "GET", "/api/video/config/get", file="get_video_config-ip")
_ip("get_video_modes", {}, "GET", "/api/video/mode/get", file="get_video_modes-ip")
_ip("get_audio_config", {}, "GET", "/api/audio/config/get", file="get_audio_config-ip")
_ip("set_overlay", {"overlay": "vu-meter", "visible": True}, "POST", "/api/video/config/set", '{"show-vu-meter":true}',
    file="set_overlay-ip")
_ip("set_aspect_conversion", {"mode": "zoom"}, "POST", "/api/video/config/set", '{"ar-convert":1}',
    file="set_aspect_conversion-ip")
_ip("set_decoder_flip", {"vertical": True}, "POST", "/api/video/config/set", '{"h-flip":false,"v-flip":true}',
    file="set_decoder_flip-ip")
_ip("set_decoder_ndi_discovery", {"enabled": True, "servers": "10.10.35.34"}, "POST", "/api/settings/ndi/set",
    '{"enable-d":true,"d-server":"10.10.35.34"}', file="set_decoder_ndi_discovery-ip")
_ip("set_decoder_ndi_group", {"group": "Public"}, "POST", "/api/settings/ndi/set", '{"groups":"Public"}',
    file="set_decoder_ndi_group-ip")

# IP decoders only (IP Decoder API V1.2 sections 3-6, Settings, base API).
_ip("add_source_url", {"name": "srt test", "url": "srt://10.10.11.117:9000?mode=caller"}, "POST", "/api/source/add",
    '{"name":"srt test","type":1,"url":"srt://10.10.11.117:9000?mode=caller"}')
_ip("add_source", {"config": {"name": "ndi encoder test", "type": 3,
                              "ndi": {"name": "ENC (1)", "url": "10.10.15.43:5961", "transport": "rudp"}}},
    "POST", "/api/source/add",
    '{"name":"ndi encoder test","type":3,"ndi":{"name":"ENC (1)","url":"10.10.15.43:5961","transport":"rudp"}}')
_ip("modify_source", {"id": 1, "config": {"type": 1, "url": "srt://10.0.0.5:9000"}}, "POST", "/api/source/set",
    '{"id":1,"config":{"type":1,"url":"srt://10.0.0.5:9000"}}')
_ip("delete_source", {"id": 1}, "POST", "/api/source/del", '{"id":[1]}')
_ip("add_profile", {"config": {"name": "test", "mode": 4, "screens": [{"id": 1}, {"id": 2}, {"id": 3}, {"id": 4}]}},
    "POST", "/api/profile/add", '{"name":"test","mode":4,"screens":[{"id":1},{"id":2},{"id":3},{"id":4}]}')
_ip("modify_profile", {"id": 2, "config": {"pip": {"mode": 0, "scale": 0.49, "x": 0.153, "y": 0.413}}},
    "POST", "/api/profile/set", '{"id":2,"config":{"pip":{"mode":0,"scale":0.49,"x":0.153,"y":0.413}}}')
_ip("delete_profile", {"id": 1}, "POST", "/api/profile/del", '{"id":1}')
_ip("set_vu_meter_scale", {"scale": "post-dbfs"}, "POST", "/api/video/config/set", '{"vu-meter-mode":2}')
_ip("set_ip_ident", {"mode": "text", "text": "2K HDMI 85"}, "POST", "/api/video/config/set",
    '{"ident-mode":1,"ident-text":"2K HDMI 85"}')
_ip("set_ip_deinterlace", {"mode": "blend", "force": True}, "POST", "/api/video/config/set",
    '{"deinterlace-mode":2,"force-deint":true}')
_ip("set_ip_input_color_space", {"space": "bt.709"}, "POST", "/api/video/config/set", '{"in-color-fmt":2}')
_ip("set_hdr_output_mode", {"mode": "disable"}, "POST", "/api/video/config/set", '{"hdr-output-mode":1}')
_ip("set_signal_loss_mode", {"mode": "last-frame"}, "POST", "/api/video/config/set", '{"switch-mode":2}')
_ip("set_screen_debug", {"enabled": False}, "POST", "/api/video/config/set", '{"screen-debug":false}')
_ip("set_webrtc_preview", {"enabled": True}, "POST", "/api/video/config/set", '{"enable-webrtc":true}')
_ip("set_ip_video_mode", {"width": 1920, "height": 1080, "frame_rate": 6000, "aspect_x": 16, "aspect_y": 9},
    "POST", "/api/video/mode/set",
    '{"width":1920,"height":1080,"interlaced":false,"frame-rate":6000,"aspect-x":16,"aspect-y":9}')
_ip("wake_screen", {}, "POST", "/api/video/screen/wakeup")
_ip("set_ip_audio_channel_map", {"map": [1, 0, 2, 3, 4, 5, 6, 7]}, "POST", "/api/audio/config/set",
    '{"channel-map":[1,0,2,3,4,5,6,7]}')
_ip("get_settings", {}, "GET", "/api/settings/get")
_ip("set_config_mode", {"mode": "profile"}, "POST", "/api/settings/mode/set", '{"mode":2}')
_ip("set_ndi_receive_transport", {"transport": "multi-tcp"}, "POST", "/api/settings/ndi/set", '{"transport":"multi-tcp"}')
_ip("set_ndi_pts_mode", {"mode": "timecode"}, "POST", "/api/settings/ndi/set", '{"pts-mode":"timecode"}')
_ip("set_ndi_default_buffer", {"ms": 60}, "POST", "/api/settings/ndi/set", '{"buffer-ms":60}')
_ip("set_ndi_extra_ips", {"ips": "10.10.37.51,10.10.35.34"}, "POST", "/api/settings/ndi/set",
    '{"extra-ips":"10.10.37.51,10.10.35.34"}')
_ip("set_gui_language", {"language": "zh"}, "POST", "/api/settings/gui/set", '{"lang":"zh"}')
_ip("get_zen_master", {}, "GET", "/api/settings/zixi/master/get")
_ip("set_zen_master", {"enabled": True, "host": "xxx.io.zixi.com", "tunnel_port": 27547, "user": "xxx"},
    "POST", "/api/settings/zixi/master/set",
    '{"enable":true,"host":"xxx.io.zixi.com","ssh-port":22,"tunnel-port":27547,"user-name":"xxx"}')
_ip("get_system_info", {}, "POST", "/api/system/info")
_ip("set_auto_reboot_days", {"enabled": True, "hour": 23, "minute": 59, "days": "1,2"}, "POST",
    "/api/system/auto-reboot", '{"enable":true,"hour":23,"min":59,"week":[1,2]}')


# Telemetry: the documents' example replies.
def _mwt(name, path, body, state, **extra):
    telemetry(MW, name, inbound_http={"path": path, "body": body if isinstance(body, str) else json.dumps(body)},
              expect_state=state, **extra)


_mwt("summary-full", "/mwapi?method=get-summary-info", {
    "status": 0,
    "device": {"name": "Pro Convert", "model": "HDMI 4K Plus", "serial-no": "B401180706020", "hw-revision": "B",
               "fw-version": "1.1.72", "up-to-date": True, "input-state": "no-signal", "output-state": "unconnected",
               "ptz-proto": "none", "ptz-state": "disconnected", "cpu-usage": 5.00, "memory-usage": 58.33,
               "core-temp": 46.76, "board-id": 0, "up-time": 8006, "sd-size": 0, "fan-rpm": 0},
    "ethernet": {"state": "1000m", "mac-addr": "70:B3:D5:75:D2:41", "ip-addr": "192.168.1.90",
                 "ip-mask": "255.255.255.0", "gw-addr": "192.168.1.1", "dns-addr": "10.0.0.3",
                 "tx-speed-kbps": 10, "rx-speed-kbps": 5},
    "ndi": {"name": "#00 (B401180706020)", "enabled": True, "num-clients": 0, "tally-preview": False,
            "tally-program": False, "audio-drop-frames": 0, "video-drop-frames": 1, "video-bit-rate": 0,
            "audio-bit-rate": 0, "video-width": 0, "video-height": 0, "video-scan": "progressive",
            "video-field-rate": 0.00, "audio-num-channels": 2, "audio-sample-rate": 48000, "audio-bit-count": 16}},
    {"device": {"name": "Pro Convert", "model": "HDMI 4K Plus", "serial": "B401180706020", "hardware_revision": "B",
                "firmware": "1.1.72", "firmware_up_to_date": True, "input_state": "no-signal",
                "output_state": "unconnected", "cpu_usage": 5.0, "memory_usage": 58.33, "temperature": 46.76,
                "board_id": 0, "uptime": 8006, "fan_rpm": 0},
     "ptz": {"protocol": "none", "state": "disconnected"},
     "network": {"ethernet": {"state": "1000m", "mac": "70:B3:D5:75:D2:41", "ip_address": "192.168.1.90",
                              "netmask": "255.255.255.0", "gateway": "192.168.1.1", "dns": "10.0.0.3",
                              "tx_kbps": 10, "rx_kbps": 5}},
     "ndi": {"name": "#00 (B401180706020)", "enabled": True, "clients": 0, "video_kbps": 0, "audio_kbps": 0,
             "video_drops": 1, "audio_drops": 0, "audio_channels": 2, "audio_sample_rate": 48000,
             "video": "0x0 progressive 0.00"},
     "tally": {"preview": False, "program": False}})
_mwt("summary-decoder", "/mwapi?method=get-summary-info",
     {"status": 0, "device": {"model": "NDI to HDMI"},
      "ndi": {"name": "STUDIO (Camera 1)", "url": "192.168.1.90:5963", "connected": True}},
     {"device": {"model": "NDI to HDMI"},
      "ndi": {"name": "STUDIO (Camera 1)", "url": "192.168.1.90:5963", "enabled": True}})
_mwt("summary-ip-full", "/api/system/summary", {
    "status": 0, "mode": 1, "product-name": "Pro Convert IP to HDMI", "device-name": "Test 2K HDMI A",
    "hardware-rev": "A", "serial-number": "A443250103001", "firmware-ver": "1.1.369", "core-temp": 52.5,
    "fan-speed": 0, "hdmi-state": 1, "uptime": 231941, "ptz": {"enabled": False},
    "profile": {"audio-idx": 0, "id": 0, "mode": 1, "name": "Default", "streams": [{
        "name": "ULTRA ENCODE (C315230423002-2)", "protocol": "ndi", "state": 2,
        "audio": {"kbps": 260, "num-channels": 8, "sample-rate": 48000},
        "video": {"kbps": 6430}, "extra": {"tally-preview": False, "tally-program": False}}]}},
    {"device": {"model": "Pro Convert IP to HDMI", "name": "Test 2K HDMI A", "hardware_revision": "A",
                "serial": "A443250103001", "firmware": "1.1.369", "temperature": 52.5, "fan_rpm": 0,
                "output_state": "1", "uptime": 231941},
     "ndi": {"name": "ULTRA ENCODE (C315230423002-2)", "video_kbps": 6430, "audio_kbps": 260, "audio_channels": 8,
             "audio_sample_rate": 48000},
     "tally": {"preview": False, "program": False}, "ptz": {"enabled": False}, "profile": {"current_id": 0},
     "config": {"mode": "simple"}})
_mwt("video-config-encoder", "/mwapi?method=get-video-config", {
    "status": 0, "show-adv-ui": False, "in-auto-aspect": True, "in-aspect-x": 16, "in-aspect-y": 9,
    "in-auto-color-fmt": True, "in-color-fmt": "rgb", "in-auto-quant-range": True, "in-quant-range": "full",
    "brightness": 0, "contrast": 100, "hue": 0, "saturation": 100, "in-crop-enabled": False, "deinterlace": "none",
    "ar-convertion": "ignore", "out-flip": False, "out-mirror": True, "out-cx": 1920, "out-cy": 1080,
    "out-raw-resolution": True, "out-aspect-x": 16, "out-aspect-y": 9, "out-auto-aspect": True,
    "out-fr-convertion": "raw", "out-auto-color-fmt": True, "out-color-fmt": "bt.709", "out-auto-sat-range": True,
    "out-sat-range": "limited", "out-auto-quant-range": True, "out-quant-range": "limited", "bit-rate-ratio": 100,
    "low-res-full-fr": False},
    {"video": {"show_advanced": False, "input_aspect_auto": True, "input_aspect": "16:9", "input_color_auto": True,
               "input_color_format": "rgb", "input_quant_auto": True, "input_quant_range": "full", "brightness": 0,
               "contrast": 100, "hue": 0, "saturation": 100, "deinterlace": "none", "aspect_conversion": "ignore",
               "v_flip": False, "h_flip": True, "output_width": 1920, "output_height": 1080,
               "output_follows_input": True, "output_aspect": "16:9", "output_aspect_auto": True,
               "frame_rate_conversion": "raw", "output_color_auto": True, "output_color_format": "bt.709",
               "output_sat_auto": True, "output_sat_range": "limited", "output_quant_auto": True,
               "output_quant_range": "limited", "bitrate_ratio": 100, "low_res_full_frame_rate": False}})
_mwt("video-config-decoder", "/mwapi?method=get-video-config", {
    "status": 0, "show-title": False, "show-tally": True, "show-vu-meter": True, "vu-meter-mode": "none",
    "show-center-cross": False, "safe-area-mode": "4:3", "ident-mode": "ident-text", "ident-text": "Screen 2",
    "h-flip": False, "v-flip": True, "switch-mode": "blank", "reset-source-on-boot": False,
    "deinterlace-mode": "bob", "in-auto-color-fmt": True, "in-color-fmt": "bt.709", "ar-convert-mode": "full",
    "alpha-disp-mode": "alpha-blend-checkerboard", "follow-input-mode": True},
    {"video": {"overlay": {"title": False, "tally": True, "vu_meter": True, "center_cross": False},
               "vu_meter_mode": "none", "safe_area": "4:3", "ident_mode": "ident-text", "ident_text": "Screen 2",
               "h_flip": False, "v_flip": True, "switch_mode": "blank", "reset_source_on_boot": False,
               "deinterlace": "bob", "input_color_auto": True, "input_color_format": "bt.709",
               "aspect_conversion": "full", "output_follows_input": True}})
_mwt("video-config-ip", "/api/video/config/get", {
    "ar-convert": 0, "color-format": "rgb", "deinterlace-mode": 0, "enable-follow-input": False,
    "enable-webrtc": True, "follow-input": False, "force-deint": False, "h-flip": False, "hdr-output-mode": 1,
    "ident-mode": 2, "ident-text": "2K HDMI 85", "in-color-fmt": 2, "quant-range": "limited", "safe-area": 0,
    "screen-debug": True, "screen-mode": 1, "show-center-cross": False, "show-tally": True, "show-title": True,
    "show-vu-meter": True, "status": 0, "switch-mode": 0, "v-flip": False, "vu-meter-mode": 1},
    {"video": {"aspect_conversion": "windowbox", "output_color_format": "rgb", "deinterlace": "bob",
               "follow_input_allowed": False, "webrtc_preview": True, "output_follows_input": False,
               "force_deinterlace": False, "h_flip": False, "hdr_output_mode": "disable", "ident_mode": "source-name",
               "ident_text": "2K HDMI 85", "input_color_auto": False, "input_color_format": "bt.709",
               "output_quant_range": "limited", "safe_area": "none", "screen_debug": True, "view_mode": "single",
               "overlay": {"center_cross": False, "tally": True, "title": True, "vu_meter": True},
               "switch_mode": "no-signal-image", "v_flip": False, "vu_meter_mode": "post-dbvu"}}, model="ip-to-hdmi")
_mwt("video-format", "/mwapi?method=get-video-format", {"status": 0, "color-format": "rgb", "quant-range": "full"},
     {"video": {"output_color_format": "rgb", "output_quant_range": "full"}})
_mwt("hdmi-output", "/mwapi?method=get-hdmi-output", {"status": 0, "enabled": True}, {"output": {"enabled": True}})
_mwt("video-modes", "/mwapi?method=get-supported-video-modes", {"status": 0, "modes": [
    {"width": 2560, "height": 1440, "interlaced": False, "field-rate": 5995, "aspect-ratio": 1.77777779,
     "pref-mode": True, "curr-mode": False},
    {"width": 1920, "height": 1080, "interlaced": False, "field-rate": 5000, "aspect-ratio": 1.77777779,
     "pref-mode": False, "curr-mode": True}]},
    {"output": {"modes": {"0": {"width": 2560, "height": 1440, "interlaced": False, "rate": 5995, "preferred": True},
                          "1": {"width": 1920, "height": 1080, "interlaced": False, "rate": 5000, "preferred": False}},
                "mode": {"width": 1920, "height": 1080, "interlaced": False, "rate": 5000}}},
    state_before={"output": {"modes": {"2": {"width": 720}}}})
_mwt("video-modes-ip", "/api/video/mode/get", {"status": 0, "modes": [
    {"aspect-ratio": 1.7777777777777777, "aspect-x": 16, "aspect-y": 9, "curr-mode": True, "frame-rate": 6000,
     "height": 1080, "interlaced": False, "pref-mode": False, "width": 1920}]},
    {"output": {"modes": {"0": {"width": 1920, "height": 1080, "interlaced": False, "rate": 6000, "preferred": False}},
                "mode": {"width": 1920, "height": 1080, "interlaced": False, "rate": 6000}}})
_mwt("audio-config", "/mwapi?method=get-audio-config", {
    "status": 0, "gain": -44.00, "sample-rate": 0, "channels": 2, "bit-count": 0, "convert-mode": "smpte",
    "ch0": 4, "ch1": 5, "ch2": 2, "ch3": 3, "ch4": 4, "ch5": 5, "ch6": 6, "ch7": 7, "ch8": 8, "ch9": 9, "ch10": 10,
    "ch11": 11, "ch12": 12, "ch13": 13, "ch14": 14, "ch15": 15, "check-pts": True},
    {"audio": {"gain": -44.0, "sample_rate": 0, "channels": 2, "convert_mode": "smpte", "check_pts": True,
               "map": {"1": 5, "2": 6, "3": 3, "4": 4, "5": 5, "6": 6, "7": 7, "8": 8, "9": 9, "10": 10, "11": 11,
                       "12": 12, "13": 13, "14": 14, "15": 15, "16": 16}}})
_mwt("audio-config-ip", "/api/audio/config/get", {
    "channel-map": [0, 1, 2, 3, 4, 5, 6, 7], "channels": 0, "check-pts": True, "convert-mode": 1, "gain": 0,
    "sample-rate": 48000, "status": 0},
    {"audio": {"channel_map": "[0,1,2,3,4,5,6,7]", "channels": 0, "check_pts": True, "convert_mode": "ebu",
               "gain": 0.0, "sample_rate": 48000}})
_mwt("current-source", "/mwapi?method=get-channel", {"status": 0, "name": "5004", "ndi-name": False},
     {"source": {"name": "5004", "ndi": False}})
_mwt("presets", "/mwapi?method=list-channels", {"status": 0, "channels": [
    {"name": "RTP", "url": "rtp://224.1.2.3:4000?mw-buffer-duration=60", "hotkey": "ctrl+1"},
    {"name": "UDP", "url": "udp://224.1.2.3:4000?mw-buffer-duration=200"}]},
    {"presets": {"0": {"name": "RTP", "url": "rtp://224.1.2.3:4000?mw-buffer-duration=60", "hotkey": "ctrl+1"},
                 "1": {"name": "UDP", "url": "udp://224.1.2.3:4000?mw-buffer-duration=200"}}},
    state_before={"presets": {"2": {"name": "Old"}}})
_mwt("playback-config", "/mwapi?method=get-playback-config", {"status": 0, "buffer-duration": 60},
     {"decoder": {"buffer_ms": 60}})
_mwt("ndi-config-encoder", "/mwapi?method=get-ndi-config", {
    "status": 0, "enable": True, "source-name": "#%board-id% (%serial-no%)", "group-name": "public",
    "enable-web-control": True, "enable-ptz-control": False, "enable-fail-over": True, "fail-over-ndi-name": "",
    "fail-over-ip-addr": "", "enable-mcast": False, "enable-rudp": False, "enable-tcp": False,
    "mcast-addr": "239.255.0.0", "mcast-mask": "255.255.0.0", "mcast-ttl": 4, "enable-udp": True,
    "enable-discovery": False, "discovery-server": "", "reference-level": 20, "vendor-name": "", "vendor-id": ""},
    {"ndi": {"enabled": True, "source_name": "#%board-id% (%serial-no%)", "groups": "public", "web_control": True,
             "ptz_control": False, "failover": {"enabled": True, "name": "", "address": ""},
             "transport_flags": {"multicast": False, "rudp": False, "multi_tcp": False, "udp": True},
             "transport": "udp", "multicast": {"address": "239.255.0.0", "netmask": "255.255.0.0", "ttl": 4},
             "discovery": {"enabled": False, "servers": ""}, "reference_level": "smpte", "vendor_name": "",
             "vendor_id": ""}})
_mwt("ndi-config-decoder", "/mwapi?method=get-ndi-config", {
    "status": 0, "enable-discovery": False, "discovery-server": "", "mcast-subnets": "192.168.1.0/24,10.0.0.0/8",
    "group-name": "public", "low-bandwidth": False, "enable-mcast": False, "enable-rudp": False, "enable-tcp": False,
    "enable-udp": False, "ignore-ndi-hx-video-pts": True, "use-timecode-first": False},
    {"ndi": {"discovery": {"enabled": False, "servers": ""}, "multicast_subnets": "192.168.1.0/24,10.0.0.0/8",
             "groups": "public", "low_bandwidth": False,
             "transport_flags": {"multicast": False, "rudp": False, "multi_tcp": False, "udp": False},
             "transport": "tcp", "ignore_hx_pts": True, "timecode_first": False}})
_mwt("ndi-sources", "/mwapi?method=get-ndi-sources", {"status": 0, "sources": [
    {"ndi-name": "MAGEWELL (USB Capture HDMI (D206191017871))", "ip-addr": "192.168.1.192:5963"}]},
    {"ndi": {"sources": {"0": {"name": "MAGEWELL (USB Capture HDMI (D206191017871))",
                               "address": "192.168.1.192:5963"}}}})
_mwt("tally-config", "/mwapi?method=get-tally", {"status": 0, "ext-tally": False},
     {"tally": {"user_controlled": False}})
_mwt("ptz-config", "/mwapi?method=get-ptz-config", {
    "status": 0, "index": 1, "baudrate": 9600, "invert-pan": False, "invert-tilt": True, "ip-addr": "10.10.10.123",
    "pan-center": 0, "pan-left-limit": -2448, "pan-right-limit": 2448, "port": 1, "proto": "visca",
    "tilt-bottom-limit": -368, "tilt-center": 0, "tilt-top-limit": 1280, "visca-msg-hdr": False,
    "zoom-out-limit": 16384, "focus-near-limit": 0, "focus-far-limit": 2935},
    {"ptz": {"protocol": "visca", "camera": 1, "baudrate": 9600, "invert_pan": False, "invert_tilt": True,
             "address": "10.10.10.123", "port": 1, "visca_header": False,
             "limits": {"pan_left": -2448, "pan_center": 0, "pan_right": 2448, "tilt_top": 1280, "tilt_center": 0,
                        "tilt_bottom": -368, "zoom_out": 16384, "focus_near": 0, "focus_far": 2935}}})
_mwt("edid-config", "/mwapi?method=get-edid-config", {
    "status": 0, "smart-edid": True, "keep-last": False, "add-audio": True, "limit-pixel-clock": True, "data": "AP8="},
    {"edid": {"smart": True, "keep_last": False, "add_audio": True, "limit_pixel_clock": True}})
_mwt("auto-reboot", "/mwapi?method=get-auto-reboot", {"status": 0, "enable": True, "hour": 3, "min": 30,
                                                      "week-flags": 8},
     {"auto_reboot": {"enabled": True, "days": 8, "hour": 3, "minute": 30}})
_mwt("ntp-server", "/mwapi?method=get-ntp-server", {"status": 0, "ntp-server": "ntp.aliyun.com"},
     {"network": {"ntp_server": "ntp.aliyun.com"}})
_mwt("net-access", "/mwapi?method=get-net-access", {"status": 0, "use-ssdp": True}, {"network": {"ssdp": True}})
_mwt("eth-status", "/mwapi?method=get-eth-status", {
    "status": 0, "use-dhcp": True, "device-name": "Pro Convert", "state": "1000m", "mac-addr": "70:B3:D5:75:D2:41",
    "ip-addr": "192.168.1.90", "ip-mask": "255.255.255.0", "gw-addr": "192.168.1.1", "dns-addr": "10.0.0.3",
    "tx-speed-kbps": 0, "rx-speed-kbps": 5},
    {"device": {"name": "Pro Convert"},
     "network": {"ethernet": {"dhcp": True, "state": "1000m", "ip_address": "192.168.1.90", "netmask": "255.255.255.0",
                              "gateway": "192.168.1.1", "dns": "10.0.0.3"}}})
_mwt("sources-ip", "/api/source/list?type=all", {
    "mode": 2, "status": 0,
    "ndi": [{"config": {"hotkey": "none", "name": "ULTRA ENCODE Test (C315230423002-2)",
                        "ndi": {"name": "ULTRA ENCODE Test (C315230423002-2)", "url": "10.10.33.113:5984"}, "type": 2},
             "id": 10017, "in-use": 2, "p-use": [1, 3, 4], "protocol": 64}],
    "static": [{"config": {"hotkey": "none", "name": "SRT Server", "type": 1,
                           "url": "srt://0.0.0.0:8000?mode=listener&latency=120"},
                "id": 1, "in-use": 0, "protocol": 2}]},
    {"sources": {"10017": {"name": "ULTRA ENCODE Test (C315230423002-2)", "type": 2, "protocol": "ndi", "kind": "ndi",
                           "ndi_name": "ULTRA ENCODE Test (C315230423002-2)", "ndi_url": "10.10.33.113:5984",
                           "hotkey": "none"},
                 "1": {"name": "SRT Server", "type": 1, "protocol": "srt", "kind": "static",
                       "url": "srt://0.0.0.0:8000?mode=listener&latency=120", "hotkey": "none"}}},
    state_before={"sources": {"7": {"name": "Gone"}}})
_mwt("profiles-ip", "/api/profile/list", {
    "current-id": 2, "mode": 2, "status": 0, "profiles": [
        {"audio-idx": 0, "id": 0, "mode": 1, "name": "Default", "screens": [{"id": 10017, "name": "ENC (1)"}],
         "selected": False},
        {"audio-idx": 1, "id": 2, "mode": 2, "name": "PIP", "pip": {"mode": 0, "scale": 0.23, "x": 0.635, "y": 0.657},
         "screens": [{"id": 10017, "name": "ENC (1)"}, {"id": 10023, "name": "ENC (2)"}], "selected": True}]},
    {"profile": {"current_id": 2},
     "profiles": {"0": {"name": "Default", "view_mode": "single", "audio_screen": 0, "selected": False,
                        "screens": {"0": {"source_id": 10017, "source_name": "ENC (1)"}}},
                  "2": {"name": "PIP", "view_mode": "pip", "audio_screen": 1, "selected": True,
                        "screens": {"0": {"source_id": 10017, "source_name": "ENC (1)"},
                                    "1": {"source_id": 10023, "source_name": "ENC (2)"}}}}},
    state_before={"profiles": {"5": {"name": "Gone"}}})
_mwt("settings-ip", "/api/settings/get", {
    "gui": {"lang": "en"}, "mode": 2, "status": 0,
    "ndi": {"buffer-ms": 60, "d-server": "10.10.37.51,10.10.35.34", "enable-d": True, "extra-ips": "",
            "groups": "Public", "low-bw": False, "mcast-subnets": "", "pts-mode": "auto", "transport": "auto"}},
    {"config": {"mode": "profile", "language": "en"},
     "ndi": {"default_buffer_ms": 60, "discovery": {"servers": "10.10.37.51,10.10.35.34", "enabled": True},
             "extra_ips": "", "groups": "Public", "low_bandwidth": False, "multicast_subnets": "", "pts_mode": "auto",
             "transport": "auto"}})
_mwt("zen-master-ip", "/api/settings/zixi/master/get", {
    "enable": False, "has-key": False, "host": "xxx.io.zixi.com", "state": "disabled", "status": 0, "ssh-port": 22,
    "tunnel-port": 27547, "user-name": "xxx"},
    {"zen_master": {"enabled": False, "host": "xxx.io.zixi.com", "state": "disabled", "ssh_port": 22,
                    "tunnel_port": 27547, "user": "xxx"}})
_mwt("system-info-ip", "/api/system/info", {
    "device-name": "USB Fusion", "uptime": 8410, "status": 0, "code": "Success",
    "datetime": {"cur-time": "2021-12-20 13:25:57", "zonename": "Asia/Shanghai", "ntp-enable": True,
                 "ntp-server1": "0.pool.ntp.org", "ntp-server2": "1.pool.ntp.org"},
    "auto-reboot": {"enable": True, "hour": 23, "min": 59, "week": [1, 2]}},
    {"device": {"name": "USB Fusion", "uptime": 8410, "time": "2021-12-20 13:25:57", "timezone": "Asia/Shanghai"},
     "network": {"ntp_server": "0.pool.ntp.org", "ntp_enabled": True, "ntp_server2": "1.pool.ntp.org"},
     "auto_reboot": {"enabled": True, "hour": 23, "minute": 59, "week": "[1,2]"}})

# Re-reads after a change.
_OKB = '{"status":0}'
for _name, _path, _target in [
    ("reread-video-config", "/mwapi?method=set-video-config&brightness=-20", "/mwapi?method=get-video-config"),
    ("reread-video-reset", "/mwapi?method=reset-video-config", "/mwapi?method=get-video-config"),
    ("reread-ndi-config", "/mwapi?method=set-ndi-config&enable=false", "/mwapi?method=get-ndi-config"),
    ("reread-tally", "/mwapi?method=set-tally&ext-tally=true", "/mwapi?method=get-tally"),
    ("reread-ptz", "/mwapi?method=arrange-ptz-cameras", "/mwapi?method=get-ptz-config"),
    ("reread-edid", "/mwapi?method=set-edid-config&keep-last=false", "/mwapi?method=get-edid-config"),
    ("reread-auto-reboot", "/mwapi?method=set-auto-reboot&enable=true&week-flags=2&hour=12&min=21",
     "/mwapi?method=get-auto-reboot"),
    ("reread-ntp", "/mwapi?method=set-ntp-server&ntp-server=ntp.aliyun.com", "/mwapi?method=get-ntp-server"),
    ("reread-net-access", "/mwapi?method=set-net-access&use-ssdp=true", "/mwapi?method=get-net-access"),
    ("reread-eth", "/mwapi?method=set-eth-config&name=Stage%20Left", "/mwapi?method=get-eth-status"),
    ("reread-hdmi-output", "/mwapi?method=set-hdmi-output&enabled=false", "/mwapi?method=get-hdmi-output"),
    ("reread-video-mode", "/mwapi?method=set-video-mode&width=1920&height=1080&interlaced=false&field-rate=5994"
     "&aspect-ratio=1.78", "/mwapi?method=get-supported-video-modes"),
    ("reread-audio", "/mwapi?method=set-audio-config&gain=-6.50", "/mwapi?method=get-audio-config"),
    ("reread-channel", "/mwapi?method=set-channel&ndi-name=false&name=Stage%20left", "/mwapi?method=get-channel"),
    ("reread-presets", "/mwapi?method=clear-channels", "/mwapi?method=list-channels"),
    ("reread-playback", "/mwapi?method=set-playback-config&buffer-duration=40", "/mwapi?method=get-playback-config"),
    ("reread-video-config-ip", "/api/video/config/set", "/api/video/config/get"),
    ("reread-video-mode-ip", "/api/video/mode/set", "/api/video/mode/get"),
    ("reread-audio-ip", "/api/audio/config/set", "/api/audio/config/get"),
    ("reread-profiles-ip", "/api/profile/screen/select", "/api/profile/list"),
    ("reread-settings-ip", "/api/settings/ndi/set", "/api/settings/get"),
    ("reread-zen-master-ip", "/api/settings/zixi/master/set", "/api/settings/zixi/master/get"),
]:
    _mwt(_name, _path, _OKB, {}, expect_then_send=[{"method": "GET", "target": _target}])
_mwt("reread-sources-ip", "/api/source/del", _OKB, {}, model="ip-to-hdmi",
     expect_then_send=[{"method": "GET", "target": "/api/source/list?type=all"}])
_mwt("reread-system-ip", "/api/system/auto-reboot", '{"status":0,"code":"Success"}', {},
     expect_then_send=[{"method": "POST", "target": "/api/system/info"}])
_mwt("reread-refused", "/mwapi?method=set-video-config&brightness=-20", '{"status":7}', {})


# AES67, Audio DX and IP to USB: JSON under /api, as the AES67 API V2.0, the
# Audio DX API and the IP to USB API give the requests. Bodies are the
# documents' fields in the order the spec writes them.
def _api(model, command, input, method, target, body=None, file=None, **extra):
    request = {"method": method, "target": target}
    if body is not None:
        request["body"] = body
    v = {"spec": MW, "command": command, "input": input, "model": model, "expect_request": request, **extra}
    if file:
        v["file"] = file
    V.append(v)


def _aes(command, input, method, target, body=None, file=None, **extra):
    _api("aes67", command, input, method, target, body, file, **extra)


def _dx(command, input, method, target, body=None, file=None, **extra):
    _api("audio-dx", command, input, method, target, body, file, **extra)


def _usb(command, input, method, target, body=None, file=None, **extra):
    _api("ip-to-usb", command, input, method, target, body, file, **extra)


# Existing commands, the new models' messages.
_aes("ping", {}, "GET", "/api/ping", file="ping-aes67")
_dx("ping", {}, "GET", "/api/ping", file="ping-dx")
_usb("ping", {}, "GET", "/api/ping", file="ping-usb", http_reply={"status": 200, "body": '{"status":0,"code":"Success"}'},
     expect_result={"ok": {"kind": "ack"}})
_aes("reboot", {}, "POST", "/api/reboot", file="reboot-aes67")
_dx("reboot", {}, "POST", "/api/reboot", file="reboot-dx")
_usb("reboot", {}, "POST", "/api/reboot", file="reboot-usb")
_aes("get_summary", {}, "POST", "/api/aes67/summary-info", file="get_summary-aes67")
_dx("get_summary", {}, "POST", "/api/aoip/summary-info", file="get_summary-dx")
_usb("get_summary", {}, "POST", "/api/system/summary/get", file="get_summary-usb")
_aes("get_system_info", {}, "POST", "/api/system/info", file="get_system_info-aes67")
_dx("get_system_info", {}, "POST", "/api/system/info", file="get_system_info-dx")
_usb("get_system_info", {}, "POST", "/api/system/info", file="get_system_info-usb")
_aes("set_device_name", {"name": "Stage AES67"}, "POST", "/api/system/set-device-name", '{"name":"Stage AES67"}',
     file="set_device_name-aes67")
_dx("set_device_name", {"name": "Booth DX"}, "POST", "/api/system/set-device-name", '{"name":"Booth DX"}',
    file="set_device_name-dx")
_usb("set_device_name", {"name": "Magewell-1"}, "POST", "/api/system/set-device-name", '{"name":"Magewell-1"}',
     file="set_device_name-usb")
_aes("set_network_dhcp", {}, "POST", "/api/network/if-set", '{"iface":"eth0","use-dhcp":true}',
     file="set_network_dhcp-aes67")
_dx("set_network_dhcp", {}, "POST", "/api/network/if-set", '{"iface":"eth0","use-dhcp":true}',
    file="set_network_dhcp-dx")
_usb("set_network_dhcp", {}, "POST", "/api/network/if-set", '{"iface":"eth0","use-dhcp":true}',
     file="set_network_dhcp-usb")
_usb("set_network_static", {"address": "10.10.10.88", "netmask": "255.255.240.0", "gateway": "10.10.0.1",
                            "dns": "10.10.0.3"}, "POST", "/api/network/if-set",
     '{"iface":"eth0","use-dhcp":false,"ipaddr":"10.10.10.88","netmask":"255.255.240.0","gateway":"10.10.0.1",'
     '"dns1":"10.10.0.3","dns2":""}', file="set_network_static-usb")
_usb("set_auto_reboot_days", {"enabled": True, "hour": 23, "minute": 59, "days": "1,2"}, "POST",
     "/api/system/auto-reboot", '{"enable":true,"hour":23,"min":59,"week":[1,2]}', file="set_auto_reboot_days-usb")
_usb("get_video_config", {}, "GET", "/api/video/config/get", file="get_video_config-usb")
_usb("set_decoder_flip", {"horizontal": True}, "POST", "/api/video/config/update",
     '{"data":{"h_flip":true,"v_flip":false}}', file="set_decoder_flip-usb")
_usb("set_aspect_conversion", {"mode": "zoom"}, "POST", "/api/video/config/update",
     '{"data":{"ar_convert_mode":"Crop"}}', file="set_aspect_conversion-usb")
_usb("select_source", {"id": 1}, "POST", "/api/source/select", '{"id":1}', file="select_source-usb")
_usb("add_source", {"config": {"name": "name1", "protocol": "rtsp", "type": "url",
                               "data": {"url": "rtsp://10.0.0.5:554/live", "buffer_ms": 60, "ptz_en": False}}},
     "POST", "/api/source/add",
     '{"name":"name1","protocol":"rtsp","type":"url","data":{"url":"rtsp://10.0.0.5:554/live","buffer_ms":60,'
     '"ptz_en":false}}', file="add_source-usb")
_usb("modify_source", {"id": 1, "config": {"name": "name1"}}, "POST", "/api/source/update",
     '{"id":1,"config":{"name":"name1"}}', file="modify_source-usb")
_usb("delete_source", {"id": 1}, "POST", "/api/source/delete", '{"id":[1]}', file="delete_source-usb")

# Shared by the three (AES67 API sections 16-17, Audio DX API 8-9, IP to USB
# API 3 and 9).
_aes("get_device_info", {}, "POST", "/api/system/device-info")
_dx("get_device_info", {}, "POST", "/api/system/device-info", file="get_device_info-dx")
_usb("get_device_info", {}, "POST", "/api/system/device-info", file="get_device_info-usb")
_aes("set_ntp", {"server1": "0.pool.ntp.org", "server2": "1.pool.ntp.org"}, "POST", "/api/system/set-date-time",
     '{"ntp-enable":true,"ntp-server1":"0.pool.ntp.org","ntp-server2":"1.pool.ntp.org"}')
_usb("set_clock", {"time": "2024-04-11 09:50:18", "server1": "0.pool.ntp.org", "server2": "1.pool.ntp.org"},
     "POST", "/api/system/set-date-time",
     '{"ntp-enable":false,"ntp-server1":"0.pool.ntp.org","ntp-server2":"1.pool.ntp.org","time":"2024-04-11 09:50:18"}')
_dx("set_timezone", {"zone": "Asia/Shanghai"}, "POST", "/api/system/timezone-set", '{"zonename":"Asia/Shanghai"}')
_aes("get_network_interfaces", {}, "POST", "/api/network/if-info")
_dx("set_network_address", {"address": "10.10.10.88", "netmask": "255.255.240.0", "gateway": "10.10.0.1"},
    "POST", "/api/network/if-set",
    '{"iface":"eth0","use-dhcp":false,"ipaddr":"10.10.10.88","netmask":"255.255.240.0","gateway":"10.10.0.1"}')
_aes("get_default_route", {}, "POST", "/api/network/if-route")
_aes("get_dns", {}, "POST", "/api/network/get-dns")
_aes("set_dns_manual", {"dns1": "10.0.1.3"}, "POST", "/api/network/set-dns",
     '{"is-manual":true,"dns1":"10.0.1.3","dns2":""}')
_dx("set_dns_auto", {}, "POST", "/api/network/set-dns", '{"is-manual":false}')

# General settings: numbers on the AES67, text on the Audio DX.
_aes("get_general_settings", {}, "POST", "/api/aes67/general-info")
_dx("get_general_settings", {}, "POST", "/api/aoip/general-info", file="get_general_settings-dx")
_dx("get_general_range", {}, "POST", "/api/aoip/general-range")
_aes("set_aes67_sample_rate", {"rate": "96000"}, "POST", "/api/aes67/general-settings", '{"tx-sample-rate":96000}')
_aes("set_aes67_packet_time", {"ptime": "0.125ms"}, "POST", "/api/aes67/general-settings", '{"tx-ptime":"0.125ms"}')
_aes("set_stream_aac_bitrate", {"kbps": "192"}, "POST", "/api/aes67/general-settings", '{"aac-bitrate-kbps":192}')
_dx("set_dx_sample_rate", {"rate": "48000"}, "POST", "/api/aoip/general-apply", '{"tx-sample-rate":"48000"}')
_aes("set_unbalanced_input_level", {"level": "-10dBV"}, "POST", "/api/aes67/general-settings", '{"unbld-in":5}')
_dx("set_unbalanced_input_level", {"level": "+4dBu"}, "POST", "/api/aoip/general-apply", '{"unbld-in":"+4dBu"}',
    file="set_unbalanced_input_level-dx")
_aes("set_unbalanced_output_level", {"level": "-2dBu"}, "POST", "/api/aes67/general-settings", '{"unbld-out":3}')
_dx("set_unbalanced_output_level", {"level": "0dBu"}, "POST", "/api/aoip/general-apply", '{"unbld-out":"0dBu"}',
    file="set_unbalanced_output_level-dx")
_aes("set_balanced_input_level", {"level": "+24dBu"}, "POST", "/api/aes67/general-settings", '{"bld-in":0}')
_dx("set_balanced_input_level", {"level": "-2dBu"}, "POST", "/api/aoip/general-apply", '{"bld-in":"-2dBV"}',
    file="set_balanced_input_level-dx")
_aes("set_balanced_output_level", {"level": "+4dBu"}, "POST", "/api/aes67/general-settings", '{"bld-out":1}')
_dx("set_balanced_output_level", {"level": "+18dBu"}, "POST", "/api/aoip/general-apply", '{"bld-out":"+18dBu"}',
    file="set_balanced_output_level-dx")
_aes("set_mic_bias", {"enabled": True}, "POST", "/api/aes67/general-settings", '{"micbias":true}')
_dx("set_mic_bias", {"enabled": False}, "POST", "/api/aoip/general-apply", '{"micbias":false}', file="set_mic_bias-dx")
_aes("set_igmp_version", {"version": "v3"}, "POST", "/api/aes67/general-settings", '{"igmp":3}')
_dx("set_igmp_version", {"version": "auto"}, "POST", "/api/aoip/general-apply", '{"igmp":"Auto"}',
    file="set_igmp_version-dx")
_aes("set_test_tone", {"mode": "analog-output"}, "POST", "/api/aes67/general-settings", '{"audio-pattern":2}')
_dx("set_test_tone", {"mode": "analog-input"}, "POST", "/api/aoip/general-apply", '{"audio-pattern":"input"}',
    file="set_test_tone-dx")

# PTP (AES67 API section 5).
_aes("get_ptp_settings", {}, "POST", "/api/aes67/ptp-info")
_aes("get_ptp_state", {}, "POST", "/api/aes67/ptp-state")
_aes("set_ptp_version", {"version": "PTPV2"}, "POST", "/api/aes67/ptp-settings", '{"version":"PTPV2"}')
_aes("set_ptp_priority", {"priority1": 128, "priority2": 255}, "POST", "/api/aes67/ptp-settings",
     '{"priority1":128,"priority2":255}')
_aes("set_ptp_domain", {"domain": 0}, "POST", "/api/aes67/ptp-settings", '{"domain":0}')
_aes("set_ptp_role", {"role": "follower"}, "POST", "/api/aes67/ptp-settings", '{"role":"Slaver"}')
_aes("set_ptp_delay_mechanism", {"mechanism": "P2P"}, "POST", "/api/aes67/ptp-settings", '{"delay-mech":"P2P"}')
_aes("set_ptp_intervals", {"announce": -2, "sync": -3}, "POST", "/api/aes67/ptp-settings",
     '{"announce":-2,"sync":-3}')
_aes("reset_ptp", {}, "POST", "/api/aes67/ptp-default")

# AES67 sources and sinks (sections 6-7).
_SRC = {"uid": 1, "name": "unicast-111", "enable": True, "dst-ip": "239.10.10.100", "dst-port": 5004, "payload": 96,
        "depth": "L24", "channel-num": 2, "ttl": 254, "dscp": "BE"}
_SRC_BODY = ('{"uid":1,"name":"unicast-111","enable":true,"dst-ip":"239.10.10.100","dst-port":5004,"payload":96,'
             '"depth":"L24","channel-num":2,"ttl":254,"dscp":"BE"}')
_aes("get_aes67_sources", {}, "POST", "/api/aes67/source-info")
_aes("apply_aes67_source", {"config": _SRC}, "POST", "/api/aes67/source-apply", _SRC_BODY)
_aes("update_aes67_source", {"config": _SRC}, "POST", "/api/aes67/source-update", _SRC_BODY)
_aes("set_aes67_source_enabled", {"uid": 1, "enabled": False}, "POST", "/api/aes67/source-update",
     '{"uid":1,"enable":false}')
_aes("delete_aes67_source", {"uid": 1}, "POST", "/api/aes67/source-del", '{"uid":1}')
_SINK = {"sink-idx": 0, "name": "Pro_Convert_AES67_1015_0_01", "is-manual": False,
         "source-name": "Pro_Convert_AES67_1015_0_01", "source-dst-ip": "239.178.203.32", "source-dst-port": 5004}
_SINK_BODY = ('{"sinks":[{"sink-idx":0,"name":"Pro_Convert_AES67_1015_0_01","is-manual":false,'
              '"source-name":"Pro_Convert_AES67_1015_0_01","source-dst-ip":"239.178.203.32","source-dst-port":5004}]}')
_aes("get_aes67_sinks", {}, "POST", "/api/aes67/sink-info")
_aes("apply_aes67_sink", {"sink": _SINK}, "POST", "/api/aes67/sink-apply", _SINK_BODY)
_aes("update_aes67_sink", {"sink": _SINK}, "POST", "/api/aes67/sink-update", _SINK_BODY)
_aes("delete_aes67_sink", {"sink": 3}, "POST", "/api/aes67/sink-del", '{"sinks":[{"sink-idx":3}]}')
_aes("get_sink_statistics", {"sink": 0}, "POST", "/api/aes67/sink-statistics", '{"sink-idx":0}')
_aes("clear_sink_statistics", {"sink": 1}, "POST", "/api/aes67/sink-statistics-clear", '{"sink-idx":1}')
_aes("parse_sdp", {"sdp": "v=0\r\ns=unicast-111\r\nm=audio 5004 RTP/AVP 96\r\n"}, "POST", "/api/aes67/sdp-parse",
     '{"sdp":"v=0\\r\\ns=unicast-111\\r\\nm=audio 5004 RTP/AVP 96\\r\\n"}')

# Streams (sections 8-14).
_aes("select_stream_receiver", {"type": "icecast"}, "POST", "/api/ices/receiver-select", '{"type":2}')
_NDI_TX = {"uid": 1, "enable": True, "ndi-name": "D424211217005_0_01", "group": "public", "mode": 3}
_NDI_TX_BODY = '{"uid":1,"enable":true,"ndi-name":"D424211217005_0_01","group":"public","mode":3}'
_aes("get_ndi_senders", {}, "POST", "/api/ndi/sender-info")
_aes("apply_ndi_sender", {"config": _NDI_TX}, "POST", "/api/ndi/sender-apply", _NDI_TX_BODY)
_aes("update_ndi_sender", {"config": _NDI_TX}, "POST", "/api/ndi/sender-update", _NDI_TX_BODY)
_aes("set_ndi_sender_enabled", {"uid": 1, "enabled": True}, "POST", "/api/ndi/sender-update", '{"uid":1,"enable":true}')
_aes("delete_ndi_sender", {"uid": 1}, "POST", "/api/ndi/sender-del", '{"uid":1}')
_aes("find_ndi_failover_sources", {}, "POST", "/api/ndi/sender-find")
_aes("set_ndi_sender_discovery", {"servers": "10.0.0.5,10.0.0.6"}, "POST", "/api/ndi/sender-options",
     '{"discovery-server":"10.0.0.5,10.0.0.6"}')
_NDI_RX = {"uid": 1, "enable": True, "ndi-name": "DESKTOP (Audio)", "url": "10.10.14.202:5961", "buffer-duration": 60}
_NDI_RX_BODY = '{"uid":1,"enable":true,"ndi-name":"DESKTOP (Audio)","url":"10.10.14.202:5961","buffer-duration":60}'
_aes("get_ndi_receivers", {}, "POST", "/api/ndi/receiver-info")
_aes("apply_ndi_receiver", {"config": _NDI_RX}, "POST", "/api/ndi/receiver-apply", _NDI_RX_BODY)
_aes("update_ndi_receiver", {"config": _NDI_RX}, "POST", "/api/ndi/receiver-update", _NDI_RX_BODY)
_aes("set_ndi_receiver_enabled", {"uid": 2, "enabled": False}, "POST", "/api/ndi/receiver-update",
     '{"uid":2,"enable":false}')
_aes("delete_ndi_receiver", {"uid": 2}, "POST", "/api/ndi/receiver-del", '{"uid":2}')
_aes("find_ndi_sources", {}, "POST", "/api/ndi/receiver-find")
_aes("set_ndi_receiver_options", {"servers": "10.0.0.5", "group": "public"}, "POST", "/api/ndi/receiver-options",
     '{"discovery-server":"10.0.0.5","group":"public","extra-ips":""}')
_SRT = {"uid": 1, "enable": True, "name": "srt_01", "mode": 1, "bind-port": 8000, "latency": 120}
_SRT_BODY = '{"uid":1,"enable":true,"name":"srt_01","mode":1,"bind-port":8000,"latency":120}'
_aes("get_srt_senders", {}, "POST", "/api/srt/sender-info")
_aes("apply_srt_sender", {"config": _SRT}, "POST", "/api/srt/sender-apply", _SRT_BODY)
_aes("update_srt_sender", {"config": _SRT}, "POST", "/api/srt/sender-update", _SRT_BODY)
_aes("set_srt_sender_enabled", {"uid": 1, "enabled": False}, "POST", "/api/srt/sender-update", '{"uid":1,"enable":false}')
_aes("delete_srt_sender", {"uid": 1}, "POST", "/api/srt/sender-del", '{"uid":1}')
_aes("get_srt_receivers", {}, "POST", "/api/srt/receiver-info")
_aes("apply_srt_receiver", {"config": _SRT}, "POST", "/api/srt/receiver-apply", _SRT_BODY)
_aes("update_srt_receiver", {"config": _SRT}, "POST", "/api/srt/receiver-update", _SRT_BODY)
_aes("set_srt_receiver_enabled", {"uid": 1, "enabled": True}, "POST", "/api/srt/receiver-update",
     '{"uid":1,"enable":true}')
_aes("delete_srt_receiver", {"uid": 1}, "POST", "/api/srt/receiver-del", '{"uid":1}')
_ICE_TX = {"uid": 1, "enable": True, "name": "test1", "service-url": "http://10.0.0.9:8000", "username": "source",
           "password": "hackme"}
_ICE_TX_BODY = ('{"uid":1,"enable":true,"name":"test1","service-url":"http://10.0.0.9:8000","username":"source",'
                '"password":"hackme"}')
_aes("get_icecast_senders", {}, "POST", "/api/ices/sender-info")
_aes("apply_icecast_sender", {"config": _ICE_TX}, "POST", "/api/ices/sender-apply", _ICE_TX_BODY)
_aes("update_icecast_sender", {"config": _ICE_TX}, "POST", "/api/ices/sender-update", _ICE_TX_BODY)
_aes("set_icecast_sender_enabled", {"uid": 1, "enabled": False}, "POST", "/api/ices/sender-update",
     '{"uid":1,"enable":false}')
_aes("delete_icecast_sender", {"uid": 1}, "POST", "/api/ices/sender-del", '{"uid":1}')
_ICE_RX = {"uid": 1, "enable": True, "name": "icecast_01", "service-url": "http://10.0.0.9:8000", "room": "stream",
           "buffer-duration": 500, "headroom": 0}
_ICE_RX_BODY = ('{"uid":1,"enable":true,"name":"icecast_01","service-url":"http://10.0.0.9:8000","room":"stream",'
                '"buffer-duration":500,"headroom":0}')
_aes("get_icecast_receivers", {}, "POST", "/api/ices/receiver-info")
_aes("apply_icecast_receiver", {"config": _ICE_RX}, "POST", "/api/ices/receiver-apply", _ICE_RX_BODY)
_aes("update_icecast_receiver", {"config": _ICE_RX}, "POST", "/api/ices/receiver-update", _ICE_RX_BODY)
_aes("set_icecast_receiver_enabled", {"uid": 1, "enabled": True}, "POST", "/api/ices/receiver-update",
     '{"uid":1,"enable":true}')
_aes("delete_icecast_receiver", {"uid": 1}, "POST", "/api/ices/receiver-del", '{"uid":1}')
_aes("list_icecast_streams", {}, "POST", "/api/ices/list-streams")

# Audio DX streams and Dante (sections 5 and 7).
_TX = {"uid": 11, "enable": True, "name": "NDI", "type": "ndi",
       "ndi": {"source-name": "test11", "group-name": "public", "transport-mode": "tcp-unicast"},
       "aac-bitrate-kbps": "128"}
_dx("get_tx_streams", {}, "POST", "/api/tx/live-info")
_dx("apply_tx_stream", {"config": _TX}, "POST", "/api/tx/live-apply",
    '{"uid":11,"enable":true,"name":"NDI","type":"ndi","ndi":{"source-name":"test11","group-name":"public",'
    '"transport-mode":"tcp-unicast"},"aac-bitrate-kbps":"128"}')
_dx("delete_tx_stream", {"uid": 11}, "POST", "/api/tx/live-del", '{"uid":11}')
_dx("get_rx_streams", {}, "POST", "/api/rx/live-info")
_dx("apply_rx_stream", {"uid": 9, "enabled": True, "name": "TS over SRT", "stream": "Stream1",
                        "url": "srt://10.10.11.54:10000?mode=caller&latency=100&mw-buffer-duration=100"},
    "POST", "/api/rx/live-apply",
    '{"uid":9,"enable":true,"name":"TS over SRT","stream-no":"Stream1",'
    '"url":"srt://10.10.11.54:10000?mode=caller&latency=100&mw-buffer-duration=100"}')
_dx("delete_rx_stream", {"uid": 9}, "POST", "/api/rx/live-del", '{"uid":9}')
_dx("set_rx_ndi_options", {"enabled": True, "servers": "10.0.0.5"}, "POST", "/api/rx/ndi-options-apply",
    '{"enable-discovery":true,"discovery-server":"10.0.0.5","group-name":"public","extra-ips":""}')
_dx("get_dante_state", {}, "POST", "/api/dante/state")

# The matrix (AES67 API section 15, Audio DX API section 6).
_aes("get_matrix", {}, "POST", "/api/aes67/matrix-info")
_dx("get_matrix", {}, "POST", "/api/aoip/matrix-info", file="get_matrix-dx")
_dx("get_matrix_layout", {}, "POST", "/api/aoip/matrix-desc")
_aes("get_audio_meters", {}, "POST", "/api/aes67/audio-meter")
_dx("get_audio_meters", {}, "POST", "/api/aoip/audio-meter", file="get_audio_meters-dx")
_aes("get_channel_meters", {"tx": 1, "rx": [1, 2, 3, 4]}, "POST", "/api/aes67/audio-meter-limit",
     '{"tx-no":1,"rx-no":[1,2,3,4]}')
_dx("get_channel_meters", {"tx": 0, "rx": [8]}, "POST", "/api/aoip/audio-meter-limit", '{"tx-no":0,"rx-no":[8]}',
    file="get_channel_meters-dx")
_aes("get_crosspoint_meter", {"tx": 2, "rx": 8}, "POST", "/api/aes67/matrix-meter", '{"tx-no":2,"rx-no":8}')
_aes("set_crosspoint", {"tx": 0, "rx": 8, "connected": True}, "POST", "/api/aes67/matrix-settings",
     '{"matrix":[{"tx-no":0,"rx-no":8,"mix":1}]}')
_dx("set_crosspoint", {"tx": 8, "rx": 0, "connected": False}, "POST", "/api/aoip/matrix-settings",
    '{"matrix":[{"tx-no":8,"rx-no":0,"mix":0}]}', file="set_crosspoint-dx")
_aes("set_crosspoints", {"matrix": [{"tx-no": 0, "rx-no": 0, "mix": 1}, {"tx-no": 1, "rx-no": 1, "mix": 1}]},
     "POST", "/api/aes67/matrix-settings", '{"matrix":[{"tx-no":0,"rx-no":0,"mix":1},{"tx-no":1,"rx-no":1,"mix":1}]}')
_dx("set_crosspoints", {"matrix": [{"tx-no": 0, "rx-no": 4, "mix": 0}]}, "POST", "/api/aoip/matrix-settings",
    '{"matrix":[{"tx-no":0,"rx-no":4,"mix":0}]}', file="set_crosspoints-dx")
_aes("clear_matrix", {}, "POST", "/api/aes67/matrix-clear")
_dx("clear_matrix", {}, "POST", "/api/aoip/matrix-clear", file="clear_matrix-dx")
_aes("set_tx_volume", {"tx": 3, "muted": False, "db": -6}, "POST", "/api/aes67/volume-tx",
     '{"volumes":[{"tx-no":3,"tx-mute":false,"db":-6}]}')
_dx("set_tx_volume", {"tx": 0, "muted": True, "db": 0}, "POST", "/api/aoip/volume-tx",
    '{"volumes":[{"tx-no":0,"tx-mute":true,"db":0}]}', file="set_tx_volume-dx")
_aes("set_crosspoint_volume", {"tx": 3, "rx": 8, "muted": False, "db": 12}, "POST", "/api/aes67/volume-mix",
     '{"volumes":[{"tx-no":3,"rx-no":8,"rx-mute":false,"db":12}]}')
_dx("set_crosspoint_volume", {"tx": 0, "rx": 1, "muted": True, "db": -36}, "POST", "/api/aoip/volume-mix",
    '{"volumes":[{"tx-no":0,"rx-no":1,"rx-mute":true,"db":-36}]}', file="set_crosspoint_volume-dx")

# IP to USB (sections 3-8).
_usb("get_usb_mode", {}, "GET", "/api/system/usb-mode/get")
_usb("set_usb_mode", {"mode": "content-sharing"}, "POST", "/api/system/usb-mode/update", '{"mode":"Content mode"}')
_usb("get_ndi_discovery", {}, "GET", "/api/ndi/config/get")
_usb("set_usb_ndi_discovery", {"enabled": True, "servers": "192.168.66.1,192.168.66.2"}, "POST",
     "/api/ndi/config/update",
     '{"data":{"enable":true,"discovery_server":"192.168.66.1,192.168.66.2","groups":"public"}}')
_usb("get_no_signal_image", {}, "POST", "/api/resource/image/get", '{"id":"no_signal_1","type":"simple"}')
_usb("delete_no_signal_image", {"id": "no_signal_1"}, "POST", "/api/resource/image/delete", '{"id":"no_signal_1"}')
_usb("get_usb_sources", {"list": "discovered"}, "GET", "/api/source/list?type=dynamic&page=1")
_usb("add_usb_ndi_source", {"name": "name1", "ndi_name": "ULTRA ENCODE (C315230423002-2)", "url": "100.100.33.21:5000",
                            "transport": "tcp"}, "POST", "/api/source/add",
     '{"name":"name1","protocol":"ndi","type":"ndi","data":{"name":"ULTRA ENCODE (C315230423002-2)",'
     '"url":"100.100.33.21:5000","low_bw":false,"transport":"tcp","pts_mode":"auto","buffer_ms":60}}')
_usb("get_stream_status", {}, "GET", "/api/stream/status/get")


# The no-signal image upload (IP to USB API section 6.1): multipart/form-data
# with the id and type fields and the PNG; the boundary is the core's, derived
# from the parts (SPEC.md §4, "Binary request bodies"). The image is a
# made-up four bytes.
def _mw_form(parts):
    import hashlib
    digest = hashlib.sha256(b"".join(n.encode() + b"\0" + d + b"\0" for n, d, _ in parts)).hexdigest()
    boundary = "----form-" + digest[:16]
    body = b""
    for name, data, filename in parts:
        body += f"--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"".encode()
        if filename:
            body += f"; filename=\"{filename}\"\r\nContent-Type: application/octet-stream".encode()
        body += b"\r\n\r\n" + data + b"\r\n"
    body += f"--{boundary}--\r\n".encode()
    return body, f"multipart/form-data; boundary={boundary}"


_IMG_BODY, _IMG_TYPE = _mw_form([("id", b"no_signal_1", None), ("type", b"simple", None),
                                 ("file", b"\x89PNG", "no_signal.png")])
V.append({"spec": MW, "command": "upload_no_signal_image", "input": {"image": "iVBORw=="}, "model": "ip-to-usb",
          "expect_request": {"method": "POST", "target": "/api/resource/image/upload", "body_hex": _IMG_BODY.hex(),
                             "content_type": _IMG_TYPE}})


# Telemetry for the new models: the documents' example replies.
def _mwm(name, model, path, body, state, **extra):
    _mwt(name, path, body, state, model=model, **extra)


_mwm("summary-aes67", "aes67", "/api/aes67/summary-info", {
    "status": 0, "temperature": "79.61°C", "card-address": 0,
    "unbalance": {"in": {"linked": True, "depth": "L24", "channel-num": 2},
                  "out": {"linked": False, "depth": "L24", "channel-num": 2}},
    "balance": {"in": {"linked": False, "depth": "L24", "channel-num": 2},
                "out": {"linked": False, "depth": "L24", "channel-num": 2}},
    "uac": {"usb-connected": True, "in": {"linked": False, "sample-rate": 48000, "depth": "L24", "channel-num": 4},
            "out": {"linked": False, "sample-rate": 48000, "depth": "L24", "channel-num": 4}},
    "sources": [{"name": "Pro_Convert_AES67_7005_0_01", "depth": "L24", "channel-num": 2, "sample-rate": 48000,
                 "ptime": "1ms"}],
    "sinks": [{"name": "Pro_Convert_AES67_1015_0_01", "depth": "L24", "channel-num": 2, "sample-rate": "48000",
               "ptime": "1ms", "qos": 2}]},
    {"device": {"temperature": 79.61, "board_id": 0},
     "io": {"unbalanced": {"in": {"linked": True, "depth": "L24", "channels": 2},
                           "out": {"linked": False, "depth": "L24", "channels": 2}},
            "balanced": {"in": {"linked": False, "depth": "L24", "channels": 2},
                         "out": {"linked": False, "depth": "L24", "channels": 2}},
            "usb": {"connected": True,
                    "in": {"linked": False, "sample_rate": 48000, "depth": "L24", "channels": 4},
                    "out": {"linked": False, "sample_rate": 48000, "depth": "L24", "channels": 4}}},
     "aes67": {"active_sources": {"0": {"name": "Pro_Convert_AES67_7005_0_01", "depth": "L24", "channels": 2,
                                        "sample_rate": 48000, "ptime": "1ms"}},
               "active_sinks": {"0": {"name": "Pro_Convert_AES67_1015_0_01", "depth": "L24", "channels": 2,
                                      "sample_rate": 48000, "ptime": "1ms", "qos": 2}}}})
_mwm("summary-dx", "audio-dx", "/api/aoip/summary-info", {
    "status": 0, "temperature": "52.5°C", "card-address": 3,
    "uac": {"usb-connected": False, "in": {"linked": False, "sample-rate": 48000, "depth": "L24", "channel-num": 2}}},
    {"device": {"temperature": 52.5, "board_id": 3},
     "io": {"usb": {"connected": False, "in": {"linked": False, "sample_rate": 48000, "depth": "L24", "channels": 2}}}})
_mwm("summary-usb", "ip-to-usb", "/api/system/summary/get", {"status": 0, "code": "Success", "data": {"cpu_temp": 54}},
     {"device": {"temperature": 54.0}})
_mwm("general-aes67", "aes67", "/api/aes67/general-info", {
    "status": 0, "tx-sample-rate": 48000, "tx-ptime": "1ms", "unbld-in": 2, "unbld-out": 2, "bld-in": 1, "bld-out": 0,
    "micbias": True, "st2110": False, "igmp": 0, "aac-bitrate-kbps": 128, "audio-pattern": 0, "uac-num-channels": 4},
    {"aoip": {"sample_rate": 48000, "packet_time": "1ms", "st2110": False},
     "analog": {"unbalanced_in_level": "0dBu", "unbalanced_out_level": "0dBu", "balanced_in_level": "+18dBu",
                "balanced_out_level": "+18dBu", "mic_bias": True, "test_tone": "off"},
     "network": {"igmp": "auto"}, "stream": {"aac_bitrate_kbps": 128}, "io": {"usb": {"channel_setting": 4}}})
_mwm("general-dx", "audio-dx", "/api/aoip/general-info", {
    "status": 0, "device-lock": False, "tx-sample-rate": "48000", "unbld-in": "+12dBu", "unbld-out": "+12dBu",
    "bld-in": "-2dBV", "bld-out": "+18dBu", "igmp": "Auto", "micbias": True, "audio-pattern": "off",
    "uac-num-channels": 2},
    {"dante": {"device_lock": False}, "aoip": {"sample_rate": 48000},
     "analog": {"unbalanced_in_level": "+12dBu", "unbalanced_out_level": "+12dBu", "balanced_in_level": "-2dBu",
                "balanced_out_level": "+18dBu", "mic_bias": True, "test_tone": "off"},
     "network": {"igmp": "auto"}, "io": {"usb": {"channel_setting": 2}}})
_mwm("ptp-info", "aes67", "/api/aes67/ptp-info", {
    "status": 0, "version": "PTPV2", "priority1": 255, "priority2": 255, "domain": 0, "gmid": "D0-C8-57-FF-FE-80-BD-5A",
    "role": "Slaver", "delay-mech": "E2E", "announce": -2, "sync": -1},
    {"ptp": {"version": "PTPV2", "priority1": 255, "priority2": 255, "domain": 0,
             "grandmaster": "D0-C8-57-FF-FE-80-BD-5A", "role": "follower", "delay_mechanism": "E2E",
             "announce_interval": -2, "sync_interval": -1}})
_mwm("ptp-default", "aes67", "/api/aes67/ptp-default", {
    "status": 0, "version": "PTPV2", "priority1": 255, "priority2": 255, "domain": 0, "role": "Auto",
    "delay-mech": "E2E", "announce": -2, "sync": -1},
    {"ptp": {"version": "PTPV2", "priority1": 255, "priority2": 255, "domain": 0, "role": "auto",
             "delay_mechanism": "E2E", "announce_interval": -2, "sync_interval": -1}})
_mwm("ptp-state", "aes67", "/api/aes67/ptp-state", {
    "status": 0, "is-sync": True, "is-master": False, "lock-to": "D0-C8-57-FF-FE-80-DF-9D",
    "statistics": {"datetime": "2022-10-14 14:26:03", "interval": 2000, "delay": [-81, 286, -182]}},
    {"ptp": {"synced": True, "leader": False, "locked_to": "D0-C8-57-FF-FE-80-DF-9D", "datetime": "2022-10-14 14:26:03",
             "statistics_interval_ms": 2000, "delay_ns": "[-81,286,-182]"}})
_mwm("aes67-sources", "aes67", "/api/aes67/source-info", {"status": 0, "sources": [{
    "uid": 1, "name": "unicast-111", "enable": True, "state": 1, "dst-ip": "239.10.10.100", "dst-port": 5004,
    "payload": 96, "depth": "L24", "channel-num": 2, "ttl": 254, "dscp": "BE", "sdp": "v=0\r\ns=unicast-111\r\n"}]},
    {"aes67": {"sources": {"1": {"name": "unicast-111", "enabled": True, "state": "sending", "address": "239.10.10.100",
                                 "port": 5004, "payload": 96, "depth": "L24", "channels": 2, "ttl": 254, "dscp": "BE",
                                 "sdp": "v=0\r\ns=unicast-111\r\n"}}}},
    state_before={"aes67": {"sources": {"4": {"name": "Gone"}}}})
_mwm("aes67-sinks", "aes67", "/api/aes67/sink-info", {"status": 0, "sinks": [{
    "sink-idx": 0, "state": 2, "name": "Pro_Convert_AES67_1015_0_01", "is-manual": False,
    "source-name": "Pro_Convert_AES67_1015_0_01", "source-src-ip": "10.10.4.195", "source-src-mac": "d0:c8:57:80:df:9d",
    "source-dst-ip": "239.178.203.32", "source-dst-port": 5004, "ssrc": 425112524, "depth": "L24", "channel-num": 2,
    "sample-rate": "48000", "ptime": "1ms", "mediaclk-offset": 0, "max-delay": 0, "max-buffer": 6, "ttl": 15, "sdp": ""}],
    "sources": [{"source-name": "Pro_Convert_AES67_1015_0_01", "source-src-ip": "10.10.4.195",
                 "source-src-mac": "d0:c8:57:80:df:9d", "source-dst-ip": "239.178.203.32", "source-dst-port": 5004,
                 "ssrc": 425112524, "depth": "L24", "channel-num": 2, "sample-rate": "48000", "ptime": "1ms",
                 "mediaclk-offset": 0, "ttl": 15, "sdp": ""}]},
    {"aes67": {"sinks": {"0": {"state": "connecting", "name": "Pro_Convert_AES67_1015_0_01", "manual": False,
                               "source_name": "Pro_Convert_AES67_1015_0_01", "source_ip": "10.10.4.195",
                               "source_mac": "d0:c8:57:80:df:9d", "address": "239.178.203.32", "port": 5004,
                               "ssrc": 425112524, "depth": "L24", "channels": 2, "sample_rate": 48000, "ptime": "1ms",
                               "mediaclk_offset": 0, "max_delay_ms": 0, "max_buffer": 6, "ttl": 15, "sdp": ""}},
               "discovered": {"0": {"name": "Pro_Convert_AES67_1015_0_01", "source_ip": "10.10.4.195",
                                    "source_mac": "d0:c8:57:80:df:9d", "address": "239.178.203.32", "port": 5004,
                                    "ssrc": 425112524, "depth": "L24", "channels": 2, "sample_rate": 48000,
                                    "ptime": "1ms", "mediaclk_offset": 0, "ttl": 15, "sdp": ""}}}})
V.append({"spec": MW, "telemetry": "sink-statistics", "model": "aes67",
          "inbound_http": {"path": "/api/aes67/sink-statistics", "request": json.loads('{"sink-idx":2}'), "body": json.dumps({
              "status": 0, "qos": 2, "duration": 4690, "peak": 0, "average": 0, "late": 0, "lost": 0, "buff-count": 0,
              "buff-underflow": 0, "buff-overflow": 0, "statistics": [{"x-us": 5000, "y-count": 0}]})},
          "expect_state": {"aes67": {"sink_stats": {"2": {
              "qos": 2, "duration_s": 4690, "peak_us": 0, "average_us": 0, "late": 0, "lost": 0, "buffer_count": 0,
              "underflows": 0, "overflows": 0, "histogram": '[{"x-us":5000,"y-count":0}]'}}}}})
_mwm("ndi-senders", "aes67", "/api/ndi/sender-info", {"status": 0, "discovery-server": "", "ndi": [{
    "uid": 1, "enable": True, "ndi-name": "D424211217005_0_01", "group": "public", "enable-full": True,
    "discovery-server-enable": True, "audio-standard": 0, "mode": 3, "multicast-ttl": 0, "multicast-net-prefix": "",
    "multicast-netmask": "", "failover-enable": False, "failover-ndi-name": "", "failover-url": "", "web-control": True,
    "report": {"module-name": "mws_ndi_sink_0", "module-type": 34, "ndi-name": "PRO-CONVERT-AES67 (D424211217005_0_01)",
               "num-clients": 0}}]},
    {"stream": {"ndi_tx_discovery_servers": ""},
     "streams": {"ndi_tx": {"1": {"enabled": True, "name": "D424211217005_0_01", "group": "public",
                                  "full_bandwidth": True, "discovery_enabled": True, "audio_standard": "smpte",
                                  "transport": "tcp", "multicast_ttl": 0, "multicast_address": "",
                                  "multicast_netmask": "", "failover_enabled": False, "failover_name": "",
                                  "failover_url": "", "web_control": True,
                                  "published_name": "PRO-CONVERT-AES67 (D424211217005_0_01)", "clients": 0}}}})
_mwm("ndi-receivers", "aes67", "/api/ndi/receiver-info", {
    "status": 0, "discovery-server": "", "group": "", "extra-ips": "", "ndi": [{
        "uid": 1, "enable": True, "is-find": True, "alias": "DESKTOP (Intel UHD Graphics 630 1)",
        "ndi-name": "DESKTOP (Intel UHD Graphics 630 1)", "url": "10.10.14.202:5961", "audio-standard": 0,
        "buffer-duration": 60, "headroom-db": -20, "format": {"sample-rate": 48000, "channel-num": 2, "bit-depth": 32},
        "report": {"audio-jitter": 22222, "audio-kbps": 3000, "is-connected": True, "living-time-ms": 6442306,
                   "module-name": "mws_ndi_src_0", "module-type": 33, "video-jitter": 0, "video-kbps": 0}}]},
    {"ndi": {"discovery": {"servers": ""}, "groups": "", "extra_ips": ""},
     "streams": {"ndi_rx": {"1": {"enabled": True, "found": True, "alias": "DESKTOP (Intel UHD Graphics 630 1)",
                                  "ndi_name": "DESKTOP (Intel UHD Graphics 630 1)", "url": "10.10.14.202:5961",
                                  "audio_standard": "smpte", "buffer_ms": 60, "headroom_db": -20, "sample_rate": 48000,
                                  "channels": 2, "bit_depth": 32, "connected": True, "audio_kbps": 3000,
                                  "audio_jitter": 22222, "uptime_ms": 6442306}}}})
_mwm("ndi-find", "aes67", "/api/ndi/receiver-find", {"status": 0, "sources": [
    {"ndi-name": "DESKTOP-KN2V7CQ (Intel UHD Graphics 630 1)", "url": "10.10.14.202:5961"}]},
    {"ndi": {"sources": {"0": {"name": "DESKTOP-KN2V7CQ (Intel UHD Graphics 630 1)", "address": "10.10.14.202:5961"}}}})
_mwm("srt-senders", "aes67", "/api/srt/sender-info", {"status": 0, "srt": [{
    "uid": 1, "enable": True, "name": "srt_01", "mode": 1, "remote-ip": "", "remote-port": 8000, "bind-port": 10000,
    "stream-id": "1", "connect-timeout": 3000, "retry-duration": 3000, "latency": 120, "bandwidth": 25, "mtu": 1500,
    "enc": 0, "passphrase": "", "report": {"living-time-ms": 8970, "mode": "listener", "module-name": "mws_srt_sink_0",
                                           "module-type": 114, "num-clients": 0}}]},
    {"streams": {"srt_tx": {"1": {"enabled": True, "name": "srt_01", "mode": "listener", "remote_ip": "",
                                  "remote_port": 8000, "bind_port": 10000, "stream_id": "1", "connect_timeout_ms": 3000,
                                  "retry_ms": 3000, "latency_ms": 120, "bandwidth_percent": 25, "mtu": 1500,
                                  "encryption": "none", "clients": 0, "uptime_ms": 8970}}}})
_mwm("srt-receivers", "aes67", "/api/srt/receiver-info", {"status": 0, "srt": [{
    "uid": 1, "enable": True, "name": "srt_01", "mode": 1, "remote-ip": "", "remote-port": 8000, "bind-port": 8000,
    "stream-id": "1", "latency": 120, "is-encrypted": False, "passphrase": "", "buffer-duration": 120,
    "headroom-db": 0, "format": {"sample-rate": 0, "channel-num": 0, "bit-depth": 0}, "report": {}}]},
    {"streams": {"srt_rx": {"1": {"enabled": True, "name": "srt_01", "mode": "listener", "remote_ip": "",
                                  "remote_port": 8000, "bind_port": 8000, "stream_id": "1", "latency_ms": 120,
                                  "encrypted": False, "buffer_ms": 120, "headroom_db": 0, "sample_rate": 0,
                                  "channels": 0, "bit_depth": 0}}}})
_mwm("icecast-senders", "aes67", "/api/ices/sender-info", {"status": 0, "ices": [{
    "uid": 1, "enable": True, "name": "test1", "service-url": "http://10.0.0.9:8000", "username": "source",
    "password": "hackme", "state": "success"}]},
    {"streams": {"icecast_tx": {"1": {"enabled": True, "name": "test1", "url": "http://10.0.0.9:8000",
                                      "user": "source", "state": "success"}}}})
_mwm("icecast-receivers", "aes67", "/api/ices/receiver-info", {"status": 0, "ices": [{
    "uid": 1, "enable": True, "name": "icecast_01", "service-url": "http://10.0.0.9:1930", "room": "stream",
    "buffer-duration": 500, "headroom-db": 0, "state": "fail",
    "format": {"sample-rate": 0, "channel-num": 0, "bit-depth": 0, "coding-format": "NONE"}}]},
    {"streams": {"icecast_rx": {"1": {"enabled": True, "name": "icecast_01", "url": "http://10.0.0.9:1930",
                                      "room": "stream", "buffer_ms": 500, "headroom_db": 0, "state": "fail",
                                      "sample_rate": 0, "channels": 0, "bit_depth": 0, "codec": "NONE"}}}})
_mwm("icecast-streams", "aes67", "/api/ices/list-streams", {"streams": [
    {"room": "icecast_02", "content-type": "audio/aac", "is-magewell": True}], "status": 0},
    {"icecast": {"streams": {"0": {"room": "icecast_02", "content_type": "audio/aac", "magewell": True}}}})
_mwm("matrix-aes67", "aes67", "/api/aes67/matrix-info", {
    "status": 0, "max-tx-channels": 22, "max-rx-channels": 46,
    "matrix": [{"tx-no": 0, "tx-mute": False, "tx-volume": 0, "mix-state": [1, 0], "rx-mute": [False, True],
                "rx-volume": [0, -6]},
               {"tx-no": 1, "tx-mute": True, "tx-volume": -3, "mix-state": [], "rx-mute": [], "rx-volume": []}],
    "uac-rx": {"channel-num": 4},
    "live-rx": {"type": 0, "lives": [{"type": 0, "name": "DESKTOP (Intel UHD Graphics 630 1)", "channel-num": 2}]},
    "aes67-rx": [{"sink-idx": 0, "name": "Pro_Convert_AES67_1015_0_01", "channel-num": 2}],
    "uac-tx": {"channel-num": 4}, "live-tx": {"ndi": ["D424211217005_0_01"], "srt": [], "ices": []},
    "aes67-tx": [{"name": "Pro_Convert_AES67_7005_0_01", "channel-num": 2}]},
    {"matrix": {"max_tx": 22, "max_rx": 46, "usb_rx_channels": 4, "usb_tx_channels": 4,
                "tx": {"0": {"mute": False, "volume_db": 0,
                             "rx": {"0": {"state": "connected", "mute": False, "volume_db": 0},
                                    "1": {"state": "disconnected", "mute": True, "volume_db": -6}}},
                       "1": {"mute": True, "volume_db": -3}},
                "stream_rx": {"0": {"type": "ndi", "name": "DESKTOP (Intel UHD Graphics 630 1)", "channels": 2}},
                "aes67_rx": {"0": {"sink": 0, "name": "Pro_Convert_AES67_1015_0_01", "channels": 2}},
                "aes67_tx": {"0": {"name": "Pro_Convert_AES67_7005_0_01", "channels": 2}},
                "live_tx": {"ndi": '["D424211217005_0_01"]', "srt": "[]", "icecast": "[]"}},
     "stream": {"receiver_type": "ndi"}},
    state_before={"matrix": {"tx": {"5": {"mute": True}}}})
_mwm("matrix-dx", "audio-dx", "/api/aoip/matrix-info", {
    "status": 0, "max-tx-channels": 32, "max-rx-channels": 32,
    "matrix": [{"tx-no": 0, "tx-mute": False, "tx-volume": 0, "mix-state": [3], "rx-mute": [False], "rx-volume": [0]}],
    "uac-rx": {"channel-num": 4},
    "stream-rx": [{"stream-no": "Stream1", "stream-index": 4, "name": "NDI", "type": "ndi", "channel-num": 4}],
    "uac-tx": {"channel-num": 4},
    "stream-tx": [{"stream-no": "Stream1", "stream-index": 4, "name": "ndi-01", "type": "ndi", "channel-num": 4}]},
    {"matrix": {"max_tx": 32, "max_rx": 32, "usb_rx_channels": 4, "usb_tx_channels": 4,
                "tx": {"0": {"mute": False, "volume_db": 0,
                             "rx": {"0": {"state": "failed", "mute": False, "volume_db": 0}}}},
                "stream_rx": {"0": {"slot": "Stream1", "name": "NDI", "type": "ndi", "channels": 4}},
                "stream_tx": {"0": {"slot": "Stream1", "name": "ndi-01", "type": "ndi", "channels": 4}}}})
_mwm("matrix-desc", "audio-dx", "/api/aoip/matrix-desc", {
    "status": 0, "max-tx-channels": 32, "max-rx-channels": 32,
    "tx": [{"name": "Unbalance", "channel-count": 2, "channel-start": 0, "channel-end": 1, "show": True},
           {"name": "Dante", "channel-count": 8, "channel-start": 8, "channel-end": 15, "show": True}],
    "rx": [{"name": "Unused", "channel-count": 4, "channel-start": 28, "channel-end": 31, "show": False}]},
    {"matrix": {"groups": {"tx": {"0": {"name": "Unbalance", "channels": 2, "first": 0, "last": 1, "shown": True},
                                  "1": {"name": "Dante", "channels": 8, "first": 8, "last": 15, "shown": True}},
                           "rx": {"0": {"name": "Unused", "channels": 4, "first": 28, "last": 31, "shown": False}}}}})
_mwm("audio-meter", "aes67", "/api/aes67/audio-meter", {"tx-db": [-100, -81], "rx-db": [-88, -26.5], "status": 0},
     {"meters": {"tx": {"0": -100.0, "1": -81.0}, "rx": {"0": -88.0, "1": -26.5}}})
_mwm("dante-state", "audio-dx", "/api/dante/state", {
    "device": {"deivce-lock": False},
    "clock-basic": {"uuid": "FE:30:38:71:E3:C7", "master-uuid": "00:1D:C1:50:B6:D8",
                    "grandmaster-uuid": "00:1D:C1:50:B6:D8", "is-mute": False, "is-locked": True, "freq-ppm": 2},
    "clock-params": {"priority1": 254, "priority2": 116, "domain": 0, "sync-interval": 0, "announce-interval": 0,
                     "ttl": 16},
    "audio-format": {"sample-rate": 48000, "encoding": "PCM24"}, "status": 0},
    {"dante": {"device_lock": False, "sample_rate": 48000, "encoding": "PCM24",
               "clock": {"uuid": "FE:30:38:71:E3:C7", "leader": "00:1D:C1:50:B6:D8",
                         "grandmaster": "00:1D:C1:50:B6:D8", "muted": False, "locked": True, "freq_ppm": 2,
                         "priority1": 254, "priority2": 116, "domain": 0, "sync_interval": 0,
                         "announce_interval": 0, "ttl": 16}}})
_mwm("tx-streams", "audio-dx", "/api/tx/live-info", {
    "stream-no-max": 2, "streaming-count-max": 16, "streaming-count": 2, "status": 0,
    "streaming": [
        {"uid": 3, "enable": True, "name": "RTSP Server", "stream-no": "Stream1", "type": "rtsp",
         "rtsp": {"bind-port": 554, "max-client-num": 8, "key": "aud", "enable-auth": False, "username": "",
                  "password": ""},
         "aac-bitrate-kbps": "128", "report": {"num-clients": 1}},
        {"uid": 9, "enable": False, "name": "TS over SRT", "stream-no": "Stream2", "type": "srt",
         "srt": {"mode": "listener", "dst-ip": "", "dst-port": 8000, "bind-port": 10000, "stream-id": "12/12",
                 "connect-timeout": 3000, "retry-duration": 3000, "latency": 120, "bandwidth": 25, "mtu": 1500,
                 "enc": "disable", "passphrase": "", "enable-logo": False},
         "aac-bitrate-kbps": "128", "report": {"mode": "listener"}}],
    "discovery": [{"is-ndi": True, "ndi-name": "DESKTOP (Intel UHD Graphics 630 1)", "ndi-url": "192.168.65.2:5961"}]},
    {"stream": {"tx_slots": 2, "tx_max_streams": 16, "tx_count": 2},
     "streams": {"tx": {"3": {"enabled": True, "name": "RTSP Server", "slot": "Stream1", "type": "rtsp",
                              "aac_kbps": 128, "rtsp_port": 554, "rtsp_max_clients": 8, "rtsp_key": "aud",
                              "rtsp_auth": False, "rtsp_user": "", "clients": 1},
                        "9": {"enabled": False, "name": "TS over SRT", "slot": "Stream2", "type": "srt",
                              "aac_kbps": 128, "srt_mode": "listener", "srt_address": "", "srt_port": 8000,
                              "srt_bind_port": 10000, "srt_stream_id": "12/12", "srt_connect_timeout_ms": 3000,
                              "srt_retry_ms": 3000, "srt_latency_ms": 120, "srt_bandwidth_percent": 25,
                              "srt_mtu": 1500, "srt_encryption": "disable"}}},
     "ndi": {"failover_sources": {"0": {"name": "DESKTOP (Intel UHD Graphics 630 1)", "address": "192.168.65.2:5961"}}}})
_mwm("rx-streams", "audio-dx", "/api/rx/live-info", {
    "stream-no-max": 2, "mw-buffer-duration-min": 1, "mw-buffer-duration-max": 3000, "mw-buffer-duration-def": 100,
    "gst-count-max": 16, "gst-count": 1, "status": 0,
    "gst": [{"uid": 11, "enable": False, "name": "NDI", "stream-no": "Stream1",
             "url": "ntkndi://ndi?ndi-name=DESKTOP (1)&ndi-url=&mw-buffer-duration=100"}],
    "ndi-options": {"enable-discovery": False, "discovery-server": "", "group-name": "", "extra-ips": ""},
    "discovery": []},
    {"stream": {"rx_slots": 2, "rx_max_streams": 16, "rx_count": 1, "rx_buffer_min_ms": 1, "rx_buffer_max_ms": 3000,
                "rx_buffer_default_ms": 100},
     "streams": {"rx": {"11": {"enabled": False, "name": "NDI", "slot": "Stream1",
                               "url": "ntkndi://ndi?ndi-name=DESKTOP (1)&ndi-url=&mw-buffer-duration=100"}}},
     "ndi": {"discovery": {"enabled": False, "servers": ""}, "groups": "", "extra_ips": ""}},
    state_before={"ndi": {"sources": {"0": {"name": "Gone"}}}})
_mwm("device-info", "ip-to-usb", "/api/system/device-info", {
    "device-name": "Pro Router ONE", "product-id": "0x601", "product-name": "Pro Router ONE", "hardware-rev": "B",
    "serial-number": "0123456789", "firmware-ver": "0.9.210", "firmware-name": "Development",
    "build-time": "2023-04-14 06:48:13", "capability": {"support-ntp": True, "support-ipv6": True}, "status": 0,
    "code": "Success"},
    {"device": {"name": "Pro Router ONE", "product_id": "0x601", "model": "Pro Router ONE", "hardware_revision": "B",
                "serial": "0123456789", "firmware": "0.9.210", "firmware_name": "Development",
                "build_time": "2023-04-14 06:48:13", "capabilities": '{"support-ntp":true,"support-ipv6":true}'}})
_mwm("system-info-cpu", "aes67", "/api/system/info", {
    "device-name": "USB Fusion", "uptime": 8410, "cpu": {"total": 1624896, "idle": 1281701, "usage": 2110},
    "mem": {"total": 8069612, "avail": 7171768}, "status": 0},
    {"device": {"name": "USB Fusion", "uptime": 8410, "cpu_usage": 21.1, "memory_total_kb": 8069612,
                "memory_available_kb": 7171768}})
_mwm("set-device-name-usb", "ip-to-usb", "/api/system/set-device-name",
     {"status": 0, "code": "SUCCESS", "ext-need-reboot": True}, {"device": {"reboot_required": True}},
     expect_then_send=[{"method": "POST", "target": "/api/system/info"}])
_mwm("interfaces-aes67", "aes67", "/api/network/if-info", {"device-name": "AES67", "status": 0, "net": [
    {"enable": True, "iface": "eth0", "type": 0, "use-dhcp": True, "ipaddr": "10.10.12.166", "netmask": "255.255.240.0",
     "gateway": "10.10.0.1", "mac": "84:85:86:87:88:2e", "link-speed": 1000, "link-state": 2, "tx-speed-kbps": 0,
     "rx-speed-kbps": 107},
    {"enable": True, "iface": "usb0", "type": 3, "use-dhcp": True, "ipaddr": "192.168.66.1", "netmask": "255.255.255.0",
     "gateway": "192.168.66.1", "mac": "8e:40:df:be:7c:fa", "link-speed": 480, "link-state": 2, "tx-speed-kbps": 0,
     "rx-speed-kbps": 0}]},
    {"network": {"interfaces": {
        "eth0": {"enabled": True, "type": "ethernet", "dhcp": True, "ip_address": "10.10.12.166",
                 "netmask": "255.255.240.0", "gateway": "10.10.0.1", "mac": "84:85:86:87:88:2e", "link_speed": 1000,
                 "link_state": "connected", "tx_kbps": 0, "rx_kbps": 107},
        "usb0": {"enabled": True, "type": "usb", "dhcp": True, "ip_address": "192.168.66.1",
                 "netmask": "255.255.255.0", "gateway": "192.168.66.1", "mac": "8e:40:df:be:7c:fa", "link_speed": 480,
                 "link_state": "connected", "tx_kbps": 0, "rx_kbps": 0}}}},
    state_before={"network": {"interfaces": {"wlan0": {"enabled": True}}}})
_mwm("interfaces-usb", "ip-to-usb", "/api/network/if-info", {"device-name": "00A601230913015", "status": 0, "net": [
    {"support-enable": False, "enable": True, "prio": 34, "iface": "eth0", "type": 0, "use-dhcp": True,
     "ipaddr": "10.10.6.222", "netmask": "255.255.240.0", "gateway": "10.10.0.1", "dns1": "10.10.1.3", "dns2": "",
     "mac": "d0:c8:57:81:c7:5e", "link-speed": 1000, "link-state": 2, "tx-speed-kbps": 0, "rx-speed-kbps": 35},
    {"enable": True, "iface": "usb0", "type": 3, "use-dhcp": True, "ipaddr": "192.168.66.1", "link-state": 1}]},
    {"network": {"interfaces": {
        "eth0": {"enabled": True, "type": "ethernet", "dhcp": True, "ip_address": "10.10.6.222",
                 "netmask": "255.255.240.0", "gateway": "10.10.0.1", "dns1": "10.10.1.3", "dns2": "",
                 "mac": "d0:c8:57:81:c7:5e", "link_speed": 1000, "link_state": "connected", "tx_kbps": 0,
                 "rx_kbps": 35},
        "usb0": {"enabled": True, "type": "usb", "dhcp": True, "ip_address": "192.168.66.1",
                 "link_state": "disconnected"}}}})
_mwm("default-route", "audio-dx", "/api/network/if-route", {"iface": "eth0", "status": 0},
     {"network": {"default_route": "eth0"}})
_mwm("dns", "aes67", "/api/network/get-dns", {"is-manual": False, "dns1": "10.0.1.3", "dns2": "", "status": 0},
     {"network": {"dns": {"manual": False, "primary": "10.0.1.3", "secondary": ""}}})
_mwm("usb-mode", "ip-to-usb", "/api/system/usb-mode/get",
     {"data": {"mode": "Normal mode"}, "status": 0, "code": "Success"}, {"usb": {"mode": "video-conferencing"}})
_mwm("video-config-usb", "ip-to-usb", "/api/video/config/get",
     {"status": 0, "code": "Success", "data": {"h_flip": True, "v_flip": False, "ar_convert_mode": "Windowbox"}},
     {"video": {"h_flip": True, "v_flip": False, "aspect_conversion": "windowbox"}})
_mwm("ndi-discovery-usb", "ip-to-usb", "/api/ndi/config/get", {"status": 0, "code": "Success", "data": {
    "enable": True, "discovery_server": "192.168.66.1,192.168.66.2", "groups": "public"}},
    {"ndi": {"discovery": {"enabled": True, "servers": "192.168.66.1,192.168.66.2"}, "groups": "public"}})
_mwm("no-signal-image", "ip-to-usb", "/api/resource/image/get", {"status": 0, "code": "Success", "data": {
    "id": "no_signal_1", "type": "simple", "path": "/static/img/no_signal.png", "is_default": True}},
    {"video": {"no_signal_images": {"no_signal_1": {"path": "/static/img/no_signal.png", "default": True}}}})
_mwm("sources-usb", "ip-to-usb", "/api/source/list?type=static&page=1", {
    "status": 0, "code": "Success", "page": 1, "page_size": 25, "total": 2, "selected": 1, "data": [
        {"id": 1, "config": {"name": "name1", "protocol": "ndi", "type": "ndi", "data": {
            "name": "ULTRA ENCODE (C315230423002-2)", "url": "100.100.33.21:5000", "low_bw": False, "transport": "tcp",
            "pts_mode": "auto", "buffer_ms": 60}}},
        {"id": 2, "config": {"name": "name2", "protocol": "rtsp", "type": "url", "data": {
            "url": "100.100.33.21:5000", "buffer_ms": 60, "ptz_en": True, "ptz": {
                "protocol": "VISCA over UDP", "ip": "192.168.1.1", "port": 3000, "device_id": 1,
                "message_header": True, "invert_pan": False, "invert_tilt": False}}}}]},
    {"source": {"selected_id": 1},
     "sources": {"1": {"name": "name1", "protocol": "ndi", "source_type": "ndi", "buffer_ms": 60, "kind": "static",
                       "ndi_name": "ULTRA ENCODE (C315230423002-2)", "ndi_url": "100.100.33.21:5000",
                       "low_bandwidth": False, "transport": "tcp", "pts_mode": "auto"},
                 "2": {"name": "name2", "protocol": "rtsp", "source_type": "url", "buffer_ms": 60, "kind": "static",
                       "url": "100.100.33.21:5000", "ptz_enabled": True,
                       "ptz": {"protocol": "VISCA over UDP", "address": "192.168.1.1", "port": 3000, "camera": 1,
                               "header": True, "invert_pan": False, "invert_tilt": False}}}},
    state_before={"sources": {"7": {"name": "Gone"}}})
_mwm("discovered-usb", "ip-to-usb", "/api/source/list?type=dynamic&page=1", {
    "status": 0, "code": "Success", "selected": 0, "data": [
        {"id": 5, "config": {"name": "CAM (1)", "protocol": "ndi", "type": "d_ndi",
                             "data": {"name": "CAM (1)", "url": "10.0.0.5:5961"}}}]},
    {"source": {"selected_id": 0}, "ndi": {"discovered": {"5": {"name": "CAM (1)", "address": "10.0.0.5:5961"}}}})
_mwm("stream-status-usb", "ip-to-usb", "/api/stream/status/get", {"status": 0, "code": "Success", "data": {
    "audio": {"codec": "ADTS", "jitter": 118856, "kbps": 123, "sample_rate": 48000, "bit_count": 16, "channels": 2,
              "drops": 0},
    "video": {"codec": "H264", "jitter": 210575, "kbps": 8029, "width": 1920, "height": 1080, "field_rate": 60.0,
              "drops": 0},
    "extra": {"tally_preview": False, "tally_program": True}, "state": 2, "uptime": 2028, "name": "NDI Server",
    "protocol": "ndi"}},
    {"stream": {"name": "NDI Server", "protocol": "ndi", "state": 2, "uptime_ms": 2028,
                "video": {"codec": "H264", "jitter": 210575, "kbps": 8029, "width": 1920, "height": 1080,
                          "field_rate": 60.0, "drops": 0},
                "audio": {"codec": "ADTS", "jitter": 118856, "kbps": 123, "sample_rate": 48000, "bit_depth": 16,
                          "channels": 2, "drops": 0}},
     "tally": {"preview": False, "program": True}})

# Re-reads after a change on the new models.
for _name, _model, _path, _sends in [
    ("reread-ntp-aes67", "aes67", "/api/system/set-date-time", [("POST", "/api/system/info")]),
    ("reread-if-set", "audio-dx", "/api/network/if-set", [("POST", "/api/network/if-info")]),
    ("reread-dns", "aes67", "/api/network/set-dns", [("POST", "/api/network/get-dns")]),
    ("reread-general-aes67", "aes67", "/api/aes67/general-settings", [("POST", "/api/aes67/general-info")]),
    ("reread-general-dx", "audio-dx", "/api/aoip/general-apply", [("POST", "/api/aoip/general-info")]),
    ("reread-ptp", "aes67", "/api/aes67/ptp-settings", [("POST", "/api/aes67/ptp-info")]),
    ("reread-aes67-sources", "aes67", "/api/aes67/source-del",
     [("POST", "/api/aes67/source-info"), ("POST", "/api/aes67/matrix-info")]),
    ("reread-aes67-sinks", "aes67", "/api/aes67/sink-apply",
     [("POST", "/api/aes67/sink-info"), ("POST", "/api/aes67/matrix-info")]),
    ("reread-ndi-receivers", "aes67", "/api/ndi/receiver-options",
     [("POST", "/api/ndi/receiver-info"), ("POST", "/api/aes67/matrix-info")]),
    ("reread-icecast-senders", "aes67", "/api/ices/sender-update",
     [("POST", "/api/ices/sender-info"), ("POST", "/api/aes67/matrix-info")]),
    ("reread-receiver-select", "aes67", "/api/ices/receiver-select", [("POST", "/api/aes67/matrix-info")]),
    ("reread-matrix-dx", "audio-dx", "/api/aoip/volume-mix", [("POST", "/api/aoip/matrix-info")]),
    ("reread-rx-streams", "audio-dx", "/api/rx/ndi-options-apply",
     [("POST", "/api/rx/live-info"), ("POST", "/api/aoip/matrix-info")]),
    ("reread-usb-mode", "ip-to-usb", "/api/system/usb-mode/update", [("GET", "/api/system/usb-mode/get")]),
    ("reread-video-config-usb", "ip-to-usb", "/api/video/config/update", [("GET", "/api/video/config/get")]),
    ("reread-ndi-usb", "ip-to-usb", "/api/ndi/config/update", [("GET", "/api/ndi/config/get")]),
    ("reread-sources-usb", "ip-to-usb", "/api/source/select",
     [("GET", "/api/source/list?type=static&page=1"), ("GET", "/api/source/list?type=dynamic&page=1")]),
]:
    _mwm(_name, _model, _path, '{"status":0}', {},
         expect_then_send=[{"method": m, "target": t} for m, t in _sends])
_mwm("reread-image-upload", "ip-to-usb", "/api/resource/image/upload", '{"status":0,"code":"Success"}', {},
     expect_then_send=[{"method": "POST", "target": "/api/resource/image/get",
                        "body": '{"id":"no_signal_1","type":"simple"}'}])
V.append({"spec": MW, "telemetry": "reread-image-delete", "model": "ip-to-usb",
          "inbound_http": {"path": "/api/resource/image/delete", "request": json.loads('{"id":"no_signal_2"}'), "body": '{"status":0,"code":"success"}'},
          "expect_state": {}, "expect_then_send": [{"method": "POST", "target": "/api/resource/image/get",
                                                     "body": '{"id":"no_signal_2","type":"simple"}'}]})
V.append({"spec": MW, "telemetry": "reread-sink-statistics", "model": "aes67",
          "inbound_http": {"path": "/api/aes67/sink-statistics-clear", "request": json.loads('{"sink-idx":1}'), "body": '{"status":0}'},
          "expect_state": {}, "expect_then_send": [{"method": "POST", "target": "/api/aes67/sink-statistics",
                                                     "body": '{"sink-idx":1}'}]})
