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
