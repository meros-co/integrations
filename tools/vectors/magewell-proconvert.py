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
               "switch_mode": "no-signal-image", "v_flip": False, "vu_meter_mode": "post-dbvu"}})
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
    {"device": {"name": "USB Fusion", "uptime": 8410}, "network": {"ntp_server": "0.pool.ntp.org"},
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
    ("reread-sources-ip", "/api/source/del", "/api/source/list?type=all"),
    ("reread-profiles-ip", "/api/profile/screen/select", "/api/profile/list"),
    ("reread-settings-ip", "/api/settings/ndi/set", "/api/settings/get"),
    ("reread-zen-master-ip", "/api/settings/zixi/master/set", "/api/settings/zixi/master/get"),
]:
    _mwt(_name, _path, _OKB, {}, expect_then_send=[{"method": "GET", "target": _target}])
_mwt("reread-system-ip", "/api/system/auto-reboot", '{"status":0,"code":"Success"}', {},
     expect_then_send=[{"method": "POST", "target": "/api/system/info"}])
_mwt("reread-refused", "/mwapi?method=set-video-config&brightness=-20", '{"status":7}', {})
