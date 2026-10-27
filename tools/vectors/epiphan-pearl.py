EPN = "epiphan-pearl"
# Epiphan Pearl REST API v2.0 (OpenAPI openapi.yml, firmware 4.24.1
# examples): server /api/v2.0, JSON replies {"status": "ok", "result": ...}
# or {"status": <error>, "message": ...}. Path segments are percent-encoded,
# query values too (RFC 3986 unreserved characters kept); JSON bodies are
# compact. Request and reply bodies follow the document's own examples.

_A = "/api/v2.0"
_OK = '{"status":"ok"}'
_ACK = {"ok": {"kind": "ack"}}


def _p(command, input, method, target, body=None, **extra):
    request = {"method": method, "target": _A + target}
    if body is not None:
        request["body"] = body
    V.append({"spec": EPN, "command": command, "input": input, "expect_request": request, **extra})


def _value(result):
    return {"ok": {"kind": "value", "value": result}}


def _reply(result, status=200):
    return {"status": status, "body": json.dumps({"status": "ok", "result": result}, separators=(",", ":"))}


def _err(status, kind, message):
    return {"status": status, "body": json.dumps({"status": kind, "message": message}, separators=(",", ":"))}


def _j(value):
    return json.dumps(value, separators=(",", ":"))


def _code(status):
    return {"error": {"error": "device_error", "code": str(status)}}


# ── Channels ──────────────────────────────────────────────────────────────
_p("get_channels", {}, "GET",
   "/channels?publishers=true&publishers-status=true&publishers-settings=true&encoders=true&active_layout=true",
   http_reply=_reply([{"id": "1", "name": "HDMI-A"}]), expect_result=_value([{"id": "1", "name": "HDMI-A"}]))
_p("get_channel_name", {"channel": "1"}, "GET", "/channels/1/name",
   http_reply=_reply("HDMI-A"), expect_result=_value("HDMI-A"))
_p("get_channel_name", {"channel": "100"}, "GET", "/channels/100/name", file="get_channel_name-not-found",
   http_reply=_err(404, "notfound", "Channel not found"), expect_result=_code(404))
_p("set_channel_name", {"channel": "1", "name": "HDMI-A Updated"}, "PUT", "/channels/1/name?name=HDMI-A%20Updated")
_p("add_bookmark", {"channel": "1", "text": "New bookmark"}, "POST", "/channels/1/bookmarks?text=New%20bookmark",
   http_reply={"status": 200, "body": _OK}, expect_result=_ACK)
# 409 when the channel is not recording.
_p("add_bookmark", {"channel": "2", "text": "Q&A"}, "POST", "/channels/2/bookmarks?text=Q%26A",
   file="add_bookmark-not-recording",
   http_reply=_err(409, "conflict", "Channel is not being recorded"), expect_result=_code(409))
_p("set_active_layout", {"channel": "1", "layout": "2"}, "PUT", "/channels/1/layouts/active?id=2")

# ── Publishers ────────────────────────────────────────────────────────────
_p("list_publishers", {"channel": "1"}, "GET", "/channels/1/publishers",
   http_reply=_reply([{"id": "0", "type": "srt", "name": "Stream 1"}]),
   expect_result=_value([{"id": "0", "type": "srt", "name": "Stream 1"}]))
_p("get_publisher_types", {"channel": "1"}, "GET", "/channels/1/publishers/type")
_p("get_publishers_status", {"channel": "1"}, "GET", "/channels/1/publishers/status")
_p("get_publisher_type", {"channel": "1", "publisher": "1"}, "GET", "/channels/1/publishers/1/type",
   http_reply=_reply("rtsp"), expect_result=_value("rtsp"))
_p("get_publisher_name", {"channel": "1", "publisher": "1"}, "GET", "/channels/1/publishers/1/name")
_p("set_publisher_name", {"channel": "1", "publisher": "1", "name": "Stream1 Updated"}, "PUT",
   "/channels/1/publishers/1/name?name=Stream1%20Updated")
_p("get_publisher_status", {"channel": "1", "publisher": "2"}, "GET", "/channels/1/publishers/2/status",
   http_reply=_reply({"is_configured": True, "started": False, "state": "stopped"}),
   expect_result=_value({"is_configured": True, "started": False, "state": "stopped"}))
_p("get_publisher_status", {"channel": "1", "publisher": "99"}, "GET", "/channels/1/publishers/99/status",
   file="get_publisher_status-not-found",
   http_reply=_err(404, "notfound", "streamer '99' for channel '1' not found"), expect_result=_code(404))
_p("get_publisher_settings", {"channel": "1", "publisher": "0"}, "GET", "/channels/1/publishers/0/settings")

_rtmp_new = {"rtmp": {"disable_audio": False, "url": "rtmp://192.168.86.51", "stream": "", "username": "", "password": ""},
             "type": "rtmp", "common": {"enabled": False, "single_touch": False}}
_p("add_publisher", {"channel": "1", "name": "New RTMP", "publisher_settings": _rtmp_new}, "POST", "/channels/1/publishers",
   body=_j({"name": "New RTMP", "settings": _rtmp_new}),
   http_reply=_reply({"id": "0", "name": "New RTMP", "settings": _rtmp_new}, 201),
   expect_result=_value({"id": "0", "name": "New RTMP", "settings": _rtmp_new}))
_p("add_publisher", {"channel": "100", "name": "New RTMP", "publisher_settings": _rtmp_new}, "POST",
   "/channels/100/publishers", body=_j({"name": "New RTMP", "settings": _rtmp_new}), file="add_publisher-not-found",
   http_reply=_err(404, "notfound", "Channel not found"), expect_result=_code(404))
_p("add_rtmp_publisher", {"channel": "1", "name": "YouTube", "url": "rtmp://a.rtmp.youtube.com/live2", "stream": "abcd-1234"},
   "POST", "/channels/1/publishers",
   body='{"name":"YouTube","settings":{"type":"rtmp","common":{"enabled":true,"single_touch":true},'
        '"rtmp":{"url":"rtmp://a.rtmp.youtube.com/live2","stream":"abcd-1234"}}}')
_p("delete_publisher", {"channel": "1", "publisher": "3"}, "DELETE", "/channels/1/publishers/3")
_p("delete_publisher", {"channel": "1", "publisher": "99"}, "DELETE", "/channels/1/publishers/99",
   file="delete_publisher-not-found",
   http_reply=_err(404, "notfound", "streamer '99' for channel '1' not found"), expect_result=_code(404))

_srt_put = {"srt": {"url": "srt://127.0.0.1:19863", "stream_id": "streamid", "disable_audio": False, "mode": "caller",
                    "latency": 125, "bw_recovery_overhead": 25, "encryption": {"passphrase": "AnyPassphrase", "keylength": 128}},
            "type": "srt", "common": {"enabled": True, "single_touch": True}}
_p("set_publisher_settings", {"channel": "1", "publisher": "2", "publisher_settings": _srt_put}, "PUT",
   "/channels/1/publishers/2/settings", body=_j(_srt_put))
_patch = {"rtmp": {"url": "rtmp://192.168.86.57", "stream": "PATCHED STREAM"}, "common": {"enabled": False}}
_p("patch_publisher_settings", {"channel": "1", "publisher": "0", "publisher_settings": _patch}, "PATCH",
   "/channels/1/publishers/0/settings", body=_j(_patch))
_p("start_publisher", {"channel": "1", "publisher": "0"}, "POST", "/channels/1/publishers/0/control/start")
_p("stop_publisher", {"channel": "1", "publisher": "0"}, "POST", "/channels/1/publishers/0/control/stop")
_p("start_channel_publishers", {"channel": "2"}, "POST", "/channels/2/publishers/control/start")
_p("stop_channel_publishers", {"channel": "2"}, "POST", "/channels/2/publishers/control/stop")
_S = "/channels/1/publishers/0/settings"
_p("set_publisher_enabled", {"channel": "1", "publisher": "0", "enabled": True}, "PATCH", _S,
   body='{"common":{"enabled":true}}')
_p("set_publisher_single_touch", {"channel": "1", "publisher": "0", "single_touch": False}, "PATCH", _S,
   body='{"common":{"single_touch":false}}')
_p("set_publisher_audio_disabled", {"channel": "1", "publisher": "0", "type": "srt", "disabled": True}, "PATCH", _S,
   body='{"srt":{"disable_audio":true}}')
_p("set_rtmp_destination", {"channel": "1", "publisher": "0", "url": "rtmp://192.168.86.57", "stream": "PATCHED STREAM"},
   "PATCH", _S, body='{"rtmp":{"url":"rtmp://192.168.86.57","stream":"PATCHED STREAM"}}')
_p("set_rtmp_credentials", {"channel": "1", "publisher": "0", "username": "test", "password": "pa\"ss"}, "PATCH", _S,
   body='{"rtmp":{"username":"test","password":"pa\\"ss"}}')
_p("set_rtsp_destination", {"channel": "1", "publisher": "0", "url": "rtsp://192.168.86.57"}, "PATCH", _S,
   body='{"rtsp":{"url":"rtsp://192.168.86.57","transport":"udp"}}')
_p("set_srt_publisher_url", {"channel": "1", "publisher": "0", "url": "srt://127.0.0.1:19869"}, "PATCH", _S,
   body='{"srt":{"url":"srt://127.0.0.1:19869"}}')
_p("set_srt_publisher_port", {"channel": "1", "publisher": "0", "port": 19869}, "PATCH", _S,
   body='{"srt":{"port":19869}}')
_p("set_srt_publisher_latency", {"channel": "1", "publisher": "0", "latency": 200}, "PATCH", _S,
   body='{"srt":{"latency":200,"bw_recovery_overhead":25}}')
_p("set_srt_publisher_stream_id", {"channel": "1", "publisher": "0", "stream_id": "#!::r=live/cam1"}, "PATCH", _S,
   body='{"srt":{"stream_id":"#!::r=live/cam1"}}')
_p("set_srt_publisher_encryption", {"channel": "1", "publisher": "0", "passphrase": "AnyPassphrase2", "keylength": "256"},
   "PATCH", _S, body='{"srt":{"encryption":{"passphrase":"AnyPassphrase2","keylength":256}}}')
_p("clear_srt_publisher_encryption", {"channel": "1", "publisher": "0"}, "PATCH", _S,
   body='{"srt":{"encryption":null}}')
_p("set_ndi_publisher_name", {"channel": "1", "publisher": "0", "ndi_name": "MD NDI"}, "PATCH", _S,
   body='{"ndi":{"ndi_name":"MD NDI"}}')
_p("set_ndi_publisher_group", {"channel": "1", "publisher": "0", "ndi_group": "studio"}, "PATCH", _S,
   body='{"ndi":{"ndi_group":"studio"}}')
_p("set_hls_publisher_url", {"channel": "1", "publisher": "0", "url": "https://192.168.86.59"}, "PATCH", _S,
   body='{"hls":{"url":"https://192.168.86.59"}}')

# ── Inputs ────────────────────────────────────────────────────────────────
_p("list_inputs", {}, "GET", "/inputs")
_srt_in = {"audio": {"delay": 0}, "video": {"nosignal": {"image": "", "timeout": 5}, "force_full_color_range": True,
                                            "hwaccel_decoding": True},
           "srt": {"mode": "listener", "latency": 80, "encryption": {"passphrase": "111111111111111", "keylength": 128},
                   "port": 1024}}
_p("get_input_settings", {"input": "SRT1"}, "GET", "/inputs/SRT1/settings",
   http_reply=_reply(_srt_in), expect_result=_value(_srt_in))
# HDMI inputs answer 405 notallowed.
_p("get_input_settings", {"input": "hdmi-a"}, "GET", "/inputs/hdmi-a/settings", file="get_input_settings-not-supported",
   http_reply=_err(405, "notallowed", "Input settings are not supported"), expect_result=_code(405))
_new_srt = {"audio": {"delay": 0}, "video": {"nosignal": {"image": "", "timeout": 8}, "force_full_color_range": True,
                                             "hwaccel_decoding": True},
            "srt": {"mode": "listener", "latency": 83, "encryption": {"passphrase": "AAAAAAAAAAAAAAAAAA", "keylength": 256},
                    "port": 1029}}
_p("create_network_input", {"type": "srt", "name": "New SRT Listener", "input_settings": _new_srt}, "POST", "/inputs",
   body=_j({"type": "srt", "name": "New SRT Listener", "settings": _new_srt}),
   http_reply=_reply("SRT1", 201), expect_result=_value("SRT1"))
_p("create_network_input", {"type": "rtsp", "name": "Cam", "input_settings": {}}, "POST", "/inputs",
   body='{"type":"rtsp","name":"Cam","settings":{}}', file="create_network_input-limit",
   http_reply=_err(409, "conflict", "The maximum number of allowed network inputs is reached"), expect_result=_code(409))
_hdmi_put = {"video": {"nosignal": {"image": "", "timeout": 5}},
             "hdmi": {"deinterlacing": False, "audio": {"mute": False, "delay": 0}}}
_p("set_input_settings", {"input": "hdmi-a", "input_settings": _hdmi_put}, "PUT", "/inputs/hdmi-a/settings",
   body=_j(_hdmi_put))
_rca = {"audio": {"delay": 5}, "local_audio": {"gain": 8, "mute": True}}
_p("patch_input_settings", {"input": "analog-b", "input_settings": _rca}, "PATCH", "/inputs/analog-b/settings",
   body=_j(_rca))
_I = "/inputs/analog-a/settings"
_p("set_input_audio_delay", {"input": "SRT1", "delay": -20}, "PATCH", "/inputs/SRT1/settings",
   body='{"audio":{"delay":-20}}')
_p("set_input_audio_mute", {"input": "analog-b", "muted": True}, "PATCH", "/inputs/analog-b/settings",
   body='{"local_audio":{"mute":true}}')
_p("set_input_audio_gain", {"input": "analog-b", "gain": 8}, "PATCH", "/inputs/analog-b/settings",
   body='{"local_audio":{"gain":8}}')
_p("set_input_phantom_power", {"input": "analog-a", "phantom_power": True}, "PATCH", _I,
   body='{"local_audio":{"phantom_power":true}}', model="pearl-mini")
_p("set_input_audio_type", {"input": "analog-b", "input_type": "RCA+3.5mm"}, "PATCH", "/inputs/analog-b/settings",
   body='{"local_audio":{"input_type":"RCA+3.5mm"}}')
_p("set_input_stereo_pair", {"input": "analog-a", "stereo_pair": False}, "PATCH", _I,
   body='{"local_audio":{"stereo_pair":false}}')
_p("set_input_channel_gain", {"input": "analog-a", "channel": "channelA", "gain": 40}, "PATCH", _I,
   body='{"local_audio":{"channels":{"channelA":{"gain":40}}}}')
_p("set_input_channel_mute", {"input": "analog-a", "channel": "channelB", "muted": True}, "PATCH", _I,
   body='{"local_audio":{"channels":{"channelB":{"mute":true}}}}')
_p("set_input_nosignal_timeout", {"input": "SRT3", "timeout": 3}, "PATCH", "/inputs/SRT3/settings",
   body='{"video":{"nosignal":{"timeout":3}}}')
_p("set_srt_input_latency", {"input": "SRT1", "mode": "listener", "latency": 82}, "PATCH", "/inputs/SRT1/settings",
   body='{"srt":{"mode":"listener","latency":82}}')
_p("set_srt_input_url", {"input": "SRT3", "mode": "rendezvous", "url": "srt://10.2.4.6:1089"}, "PATCH",
   "/inputs/SRT3/settings", body='{"srt":{"mode":"rendezvous","url":"srt://10.2.4.6:1089"}}')
_p("set_srt_input_port", {"input": "SRT1", "port": 1025}, "PATCH", "/inputs/SRT1/settings",
   body='{"srt":{"mode":"listener","port":1025}}')
_p("set_srt_input_stream_id", {"input": "SRT2", "stream_id": "TEST"}, "PATCH", "/inputs/SRT2/settings",
   body='{"srt":{"mode":"caller","stream_id":"TEST"}}')
_p("set_srt_input_encryption", {"input": "SRT1", "mode": "listener", "passphrase": "111111111111111", "keylength": "128"},
   "PATCH", "/inputs/SRT1/settings",
   body='{"srt":{"mode":"listener","encryption":{"passphrase":"111111111111111","keylength":128}}}')
_p("set_usb_camera_settings", {"input": "USBA", "capture_mode": "1920x1080@30", "brightness": 50, "contrast": 50,
                               "saturation": 60, "sharpness": 50, "pan": -10, "tilt": 5, "power_line_frequency": 2,
                               "volume": 70},
   "PUT", "/inputs/USBA/settings",
   body='{"usb":{"capture_mode":"1920x1080@30","brightness":50,"contrast":50,"saturation":60,"sharpness":50,'
        '"pan":-10,"tilt":5,"power_line_frequency":2,"volume":70}}',
   http_reply={"status": 200, "body": _OK}, expect_result=_ACK)

# ── Outputs ───────────────────────────────────────────────────────────────
_p("list_outputs", {}, "GET", "/outputs", http_reply=_reply([{"id": "D1", "name": "HDMI"}]),
   expect_result=_value([{"id": "D1", "name": "HDMI"}]))
_p("set_output_source", {"output": "D1", "source": "SRT1"}, "PUT", "/outputs/D1/settings?source=SRT1",
   http_reply={"status": 200, "body": _OK}, expect_result=_ACK)
_p("set_output_source", {"output": "D30", "source": "multiview"}, "PUT", "/outputs/D30/settings?source=multiview",
   file="set_output_source-not-found",
   http_reply=_err(404, "notfound", "output 'D30' not found"), expect_result=_code(404))

# ── Recorders ─────────────────────────────────────────────────────────────
_p("list_recorders", {}, "GET", "/recorders")
_p("get_recorders_status", {}, "GET", "/recorders/status")
_p("get_recorder_status", {"recorder": "1"}, "GET", "/recorders/1/status",
   http_reply=_reply({"state": "started", "duration": 195, "active": "1", "total": "1"}),
   expect_result=_value({"state": "started", "duration": 195, "active": "1", "total": "1"}))
_p("start_recorder", {"recorder": "1"}, "POST", "/recorders/1/control/start")
_p("start_recorder", {"recorder": "1"}, "POST", "/recorders/1/control/start", file="start_recorder-bad-request",
   http_reply=_err(400, "badrequest", "Recorder is disabled"), expect_result=_code(400))
_p("stop_recorder", {"recorder": "1"}, "POST", "/recorders/1/control/stop")
_p("start_all_recorders", {}, "POST", "/recorders/control/start")
_p("stop_all_recorders", {}, "POST", "/recorders/control/stop")
_p("start_recorders", {"ids": "1,2"}, "POST", "/recorders/control/start?ids=1,2")
_p("stop_recorders", {"ids": "3"}, "POST", "/recorders/control/stop?ids=3")
_p("list_recorder_files", {"recorder": "1"}, "GET", "/recorders/1/archive/files")
_p("list_recorder_files_page", {"recorder": "1", "limit": 10}, "GET", "/recorders/1/archive/files?from=0&limit=10")

# ── Events ────────────────────────────────────────────────────────────────
_p("list_events", {}, "GET", "/schedule/events")
_p("list_events_by_status", {"status": "running"}, "GET", "/schedule/events?status=running")
_p("get_event", {"event": "ongoing"}, "GET", "/schedule/events/ongoing")
_adhoc = {"type": "vod", "title": "Example ad-hoc event", "description": "Example event\r\nmultiline\r\ndescription",
          "start": 60, "duration": 3600}
_p("create_adhoc_event", {"event": _adhoc}, "POST", "/schedule/events", body=_j(_adhoc))
_p("start_event", {"event": "782ec0f4bbcc48e2a42a44eea5e69dc5"}, "POST",
   "/schedule/events/782ec0f4bbcc48e2a42a44eea5e69dc5/control/start")
_p("start_event", {"event": "completed"}, "POST", "/schedule/events/completed/control/start", file="start_event-conflict",
   http_reply=_err(409, "conflict", "Only a scheduled event can be started"), expect_result=_code(409))
_p("stop_event", {"event": "ongoing"}, "POST", "/schedule/events/ongoing/control/stop")
_p("pause_event", {"event": "running"}, "POST", "/schedule/events/running/control/pause")
_p("resume_event", {"event": "paused"}, "POST", "/schedule/events/paused/control/resume")
_p("extend_event", {"event": "ongoing", "seconds": 300}, "POST", "/schedule/events/ongoing/control/extend",
   body='{"finish":300}')
_p("get_adhoc_session", {}, "GET", "/schedule/events/adhoc/session",
   http_reply={"status": 200, "body": '{"id":"u54321","name":"John Doe","created":1592581765,"expired":1592581865}'},
   expect_result=_value({"id": "u54321", "name": "John Doe", "created": 1592581765, "expired": 1592581865}))
_p("adhoc_login_kaltura", {"id": "JohnDoe"}, "POST", "/schedule/events/adhoc/session", body='{"id":"JohnDoe"}')
_p("adhoc_login_panopto", {"id": "JohnDoe", "password": "JohnDoePassword"}, "POST", "/schedule/events/adhoc/session",
   body='{"id":"JohnDoe","password":"JohnDoePassword"}')
_p("adhoc_logout", {}, "DELETE", "/schedule/events/adhoc/session")

# ── System ────────────────────────────────────────────────────────────────
_p("get_system_status", {}, "GET", "/system/status")
_p("get_connectivity", {}, "GET", "/system/connectivity/details")
_p("run_speedtest", {}, "GET", "/system/connectivity/tools/speedtest?mode=uplink&protocol=tcp&timeout=30",
   http_reply=_reply({"protocol": "tcp", "mode": "uplink", "bandwidth": 91318568, "bitrate_limit": 1000000000,
                      "duration": 10}),
   expect_result=_value({"protocol": "tcp", "mode": "uplink", "bandwidth": 91318568, "bitrate_limit": 1000000000,
                         "duration": 10}))
_p("run_speedtest", {"mode": "downlink", "protocol": "udp", "timeout": 5}, "GET",
   "/system/connectivity/tools/speedtest?mode=downlink&protocol=udp&timeout=5", file="run_speedtest-udp")
_p("reboot", {}, "POST", "/system/control/reboot", http_reply={"status": 200, "body": _OK}, expect_result=_ACK)
_p("shutdown", {}, "POST", "/system/control/shutdown")
_p("factory_reset", {}, "POST", "/system/control/factoryreset")
_p("get_firmware", {}, "GET", "/system/firmware",
   http_reply=_reply({"version": "4.24.1", "revision": "250107_1e4514f", "product_id": 44, "product_name": "Pearl Mini"}),
   expect_result=_value({"version": "4.24.1", "revision": "250107_1e4514f", "product_id": 44,
                         "product_name": "Pearl Mini"}))
_p("get_firmware_version", {}, "GET", "/system/firmware/version", http_reply=_reply("4.24.1"),
   expect_result=_value("4.24.1"))
_p("get_firmware_revision", {}, "GET", "/system/firmware/revision")
_p("get_product_id", {}, "GET", "/system/firmware/product_id", http_reply=_reply(44), expect_result=_value(44))
_p("get_product_name", {}, "GET", "/system/firmware/product_name")
_p("get_ident", {}, "GET", "/system/ident")
_p("list_single_touch", {}, "GET", "/system/singletouchcontrol")
_p("get_single_touch_state", {}, "GET", "/system/singletouchcontrol/0/state")
_p("toggle_single_touch", {}, "POST", "/system/singletouchcontrol/0/control/toggle")
_p("list_presets", {}, "GET", "/system/presets?details=true")
_p("apply_preset", {"preset": "Default"}, "POST", "/system/presets/Default/control/apply",
   http_reply=_reply({"reboot": True}), expect_result=_value(True))
_p("apply_preset", {"preset": "VERYLASTCONFIG1"}, "POST", "/system/presets/VERYLASTCONFIG1/control/apply",
   file="apply_preset-not-found",
   http_reply=_err(404, "notfound", "Configuration preset VERYLASTCONFIG1 is not found"), expect_result=_code(404))
_p("apply_preset_sections", {"preset": "Studio A", "sections": ["channels", "sources"]}, "POST",
   "/system/presets/Studio%20A/control/apply", body='{"sections":["channels","sources"]}')
_p("list_storages", {}, "GET", "/system/storages")
_p("get_storage_status", {"storage": "main"}, "GET", "/system/storages/main/status")
_p("eject_storage", {"storage": "external"}, "POST", "/system/storages/external/control/eject")
_p("eject_storage", {"storage": "main"}, "POST", "/system/storages/main/control/eject", file="eject_storage-main",
   http_reply=_err(405, "notallowed", "Storage 'main' does not support eject"), expect_result=_code(405))
_p("get_storage_transfer_status", {"storage": "external"}, "GET", "/system/storages/external/transfer/status?storage=true")
_p("list_afu", {}, "GET", "/afu")
_p("get_afu_status", {}, "GET", "/afu/status")
_p("get_afu_destination_status", {}, "GET", "/afu/0/status")


# ── Telemetry ─────────────────────────────────────────────────────────────
def _tel(name, path, body, expect_state, **extra):
    telemetry(EPN, name, inbound_http={"path": _A + path, "body": body if isinstance(body, str) else _j(body)},
              expect_state=expect_state, **extra)


def _get(target):
    return {"method": "GET", "target": _A + target}


_FAST = "/channels?publishers=true&publishers-status=true"
_srt_status = {"is_configured": True, "started": True, "state": "started", "duration": 188, "since": 1734472241,
               "statistics": {"total": {"duration": 182, "pkt_sent": 62875, "pkt_loss": 0, "pkt_retrans": 0, "pkt_drop": 0,
                                        "latency": 125, "byte_sent": 80118160, "byte_retrans": 0, "byte_drop": 0},
                              "current": {"duration": 20, "send_rate": 3.61, "loss_ratio": 0, "retrans_ratio": 0,
                                          "drop_ratio": 0, "latency": 125, "rtt": 699, "estimated_bandwidth": 299.736,
                                          "send_buffer": 1000, "stream_id": "", "ip": "192.168.86.102"}},
               "reconnections": 0, "warnings": [{"text": "Low bandwidth"}]}
_tel("publishers-status", _FAST, {"status": "ok", "result": [
    {"id": "1", "name": "HDMI-A", "publishers": [
        {"id": "0", "type": "rtmp", "name": "Stream 1", "status": {"is_configured": True, "started": False, "state": "stopped"}},
        {"id": "2", "type": "srt", "name": "Stream 3", "status": _srt_status}]},
    {"id": "2", "name": "SDI", "publishers": [
        {"id": "0", "type": "rtsp", "name": "Stream 1",
         "status": {"is_configured": False, "started": False, "state": "error", "description": "No URL"}}]}]},
     # Publisher 1/1 was deleted on the Pearl: the read replaces the list.
     state_before={"publishers": {"1": {"1": {"type": "rtsp", "name": "Old", "state": "stopped"}}}},
     expect_state={"publishers": {
         "1": {"0": {"type": "rtmp", "name": "Stream 1", "is_configured": True, "started": False, "state": "stopped"},
               "2": {"type": "srt", "name": "Stream 3", "is_configured": True, "started": True, "state": "started",
                     "duration": 188, "since": 1734472241, "reconnections": 0,
                     "warnings": {"0": {"text": "Low bandwidth"}},
                     "statistics": {"total": {"duration": 182, "pkt_sent": 62875, "pkt_loss": 0, "pkt_retrans": 0,
                                              "pkt_drop": 0, "latency": 125, "byte_sent": 80118160, "byte_retrans": 0,
                                              "byte_drop": 0},
                                    "current": {"duration": 20, "send_rate": 3.61, "loss_ratio": 0.0,
                                                "retrans_ratio": 0.0, "drop_ratio": 0.0, "latency": 125, "rtt": 699.0,
                                                "estimated_bandwidth": 299.736, "send_buffer": 1000, "stream_id": "",
                                                "ip": "192.168.86.102"}}}},
         "2": {"0": {"type": "rtsp", "name": "Stream 1", "is_configured": False, "started": False, "state": "error",
                     "description": "No URL"}}}})
_tel("publisher-added", "/channels/1/publishers", {"status": "ok", "result": {"id": "3", "name": "New RTMP", "settings": _rtmp_new}},
     {}, expect_then_send=[_get(_FAST)])
_tel("publisher-deleted", "/channels/1/publishers/3", _OK,
     {"publisher_settings": {"1": {"0": {"type": "rtmp"}}}},
     state_before={"publisher_settings": {"1": {"0": {"type": "rtmp"}, "3": {"type": "ndi", "ndi": {"ndi_name": "X"}}}}})
_tel("publisher-name", "/channels/1/publishers/1/name?name=Stream1%20Updated", {"status": "ok", "result": "Stream1 Updated"},
     {"publishers": {"1": {"1": {"name": "Stream1 Updated"}}}})
_tel("publisher-settings-rtmp", "/channels/1/publishers/0/settings",
     {"status": "ok", "result": {"rtmp": {"disable_audio": False, "url": "rtmp://192.168.86.51", "stream": "key",
                                          "username": "", "password": "secret"},
                                 "type": "rtmp", "common": {"enabled": True, "single_touch": False}}},
     # It was an SRT publisher before: the read replaces its settings.
     {"publisher_settings": {"1": {"0": {"type": "rtmp", "common": {"enabled": True, "single_touch": False},
                                         "rtmp": {"url": "rtmp://192.168.86.51", "stream": "key", "username": "",
                                                  "disable_audio": False}}}}},
     state_before={"publisher_settings": {"1": {"0": {"type": "srt", "srt": {"mode": "caller", "latency": 125}}}}})
_tel("publisher-settings-srt", "/channels/1/publishers/2/settings",
     {"status": "ok", "result": {"srt": {"port": 1029, "disable_audio": False, "mode": "listener", "latency": 125,
                                         "bw_recovery_overhead": 25,
                                         "encryption": {"passphrase": "1111111111", "keylength": 256}},
                                 "type": "srt", "common": {"enabled": False, "single_touch": True}}},
     {"publisher_settings": {"1": {"2": {"type": "srt", "common": {"enabled": False, "single_touch": True},
                                         "srt": {"port": 1029, "disable_audio": False, "mode": "listener", "latency": 125,
                                                 "bw_recovery_overhead": 25, "keylength": 256}}}}})
_tel("publisher-settings-mpegts", "/channels/1/publishers/4/settings",
     {"status": "ok", "result": {"mpegts_udp": {"port": 6004, "address": "239.0.0.1", "sap": False, "sap_ip": "",
                                                "sap_channel": 0, "sap_play_group": "", "limit_bandwidth": True,
                                                "limit_bandwidth_rate": 1024},
                                 "type": "mpegts-udp", "common": {"enabled": False, "single_touch": False}}},
     {"publisher_settings": {"1": {"4": {"type": "mpegts-udp", "common": {"enabled": False, "single_touch": False},
                                         "mpegts_udp": {"port": 6004, "address": "239.0.0.1", "sap": False, "sap_ip": "",
                                                        "sap_channel": 0, "sap_play_group": "", "limit_bandwidth": True,
                                                        "limit_bandwidth_rate": 1024}}}}})
_tel("publisher-settings-ndi", "/channels/1/publishers/5/settings",
     {"status": "ok", "result": {"ndi": {"disable_audio": False, "ndi_name": "MD NDI", "ndi_group": ""},
                                 "type": "ndi", "common": {"enabled": True, "single_touch": False}}},
     {"publisher_settings": {"1": {"5": {"type": "ndi", "common": {"enabled": True, "single_touch": False},
                                         "ndi": {"disable_audio": False, "ndi_name": "MD NDI", "ndi_group": ""}}}}})
_tel("publisher-settings-hls", "/channels/1/publishers/6/settings",
     {"status": "ok", "result": {"hls": {"disable_audio": False, "url": "https://h.example/up", "master_pl_url": "",
                                         "username": "", "password": "", "segment_duration": 6, "list_size": 6,
                                         "user_agent": "", "http_method": "POST", "media_seg_tmpl": ""},
                                 "type": "hls", "common": {"enabled": False, "single_touch": False}}},
     {"publisher_settings": {"1": {"6": {"type": "hls", "common": {"enabled": False, "single_touch": False},
                                         "hls": {"disable_audio": False, "url": "https://h.example/up",
                                                 "master_pl_url": "", "username": "", "segment_duration": 6,
                                                 "list_size": 6, "user_agent": "", "http_method": "POST",
                                                 "media_seg_tmpl": ""}}}}})
# A PATCH reply's flattened shape is not taken: the GET after it is.
_before = {"publisher_settings": {"1": {"0": {"type": "rtmp", "rtmp": {"url": "rtmp://old"}}}}}
_tel("publisher-patch-reply-ignored", "/channels/1/publishers/0/settings",
     '{"status":"ok","result":{"started":false,"single-touch":false,"disable-audio":false,"url":"rtmp://new"}}',
     _before, state_before=_before)

_SLOW = "/channels?publishers=true&encoders=true&active_layout=true"
_chan = {"id": "1", "name": "HDMI-A",
         "publishers": [{"id": "0", "type": "rtmp", "name": "Stream 1"}, {"id": "2", "type": "srt", "name": "Stream 3"}],
         "encoders": [{"id": "0", "type": "video", "name": "H.264", "resolution": "1920x1080", "framerate": 30,
                       "bitrate": 3000},
                      {"id": "1", "type": "audio", "name": "AAC", "channels": 2, "bitrate": 128}],
         "active_layout": {"id": "1", "name": "Default",
                           "sources": {"video": [{"id": "D2P0.hdmi-a", "name": "HDMI-A"}],
                                       "audio": [{"id": "D2P0.hdmi-a-audio", "name": "HDMI-A Audio"}]}}}
_chan_state = {"name": "HDMI-A",
               "encoders": {"0": {"type": "video", "name": "H.264", "resolution": "1920x1080", "framerate": 30.0,
                                  "bitrate": 3000},
                            "1": {"type": "audio", "name": "AAC", "channels": 2, "bitrate": 128}},
               "active_layout": {"id": "1", "name": "Default",
                                 "sources": {"video": {"0": {"id": "D2P0.hdmi-a", "name": "HDMI-A"}},
                                             "audio": {"0": {"id": "D2P0.hdmi-a-audio", "name": "HDMI-A Audio"}}}}}
_tel("channels-configuration", _SLOW, {"status": "ok", "result": [_chan]},
     {"channels": {"1": _chan_state}},
     state_before={"channels": {"9": {"name": "Deleted"}}},
     expect_then_send=[_get("/channels/1/publishers/0/settings"), _get("/channels/1/publishers/2/settings")])
# set_active_layout's read of one channel keeps the others.
_tel("channel-layout-readback", "/channels?ids=1&publishers=true&encoders=true&active_layout=true",
     {"status": "ok", "result": [_chan]},
     {"channels": {"1": _chan_state, "2": {"name": "SDI"}}},
     state_before={"channels": {"1": {"name": "HDMI-A", "active_layout": {"id": "2", "name": "PiP"}},
                                "2": {"name": "SDI"}}},
     expect_then_send=[_get("/channels/1/publishers/0/settings"), _get("/channels/1/publishers/2/settings")])
_tel("channel-name", "/channels/1/name", {"status": "ok", "result": "HDMI-A Updated"},
     {"channels": {"1": {"name": "HDMI-A Updated"}}})

_inputs = [{"id": "D2P496187.hdmi-a", "name": "HDMI-A", "audio": False, "video": True, "type": "embedded"},
           {"id": "analog-a", "name": "XLR", "audio": True, "video": False, "type": "embedded"},
           {"id": "USBA", "name": "USB-A", "audio": True, "video": True, "type": "usb"},
           {"id": "RTSP1", "name": "RTSP 1", "real_device_name": "RTSP 1", "audio": True, "video": True, "type": "rtsp"},
           {"id": "SRT1", "name": "SRT 1", "audio": True, "video": True, "type": "srt"}]
_tel("inputs", "/inputs", {"status": "ok", "result": _inputs},
     {"inputs": {"D2P496187.hdmi-a": {"name": "HDMI-A", "audio": False, "video": True, "type": "embedded"},
                 "analog-a": {"name": "XLR", "audio": True, "video": False, "type": "embedded"},
                 "USBA": {"name": "USB-A", "audio": True, "video": True, "type": "usb"},
                 "RTSP1": {"name": "RTSP 1", "real_device_name": "RTSP 1", "audio": True, "video": True, "type": "rtsp"},
                 "SRT1": {"name": "SRT 1", "audio": True, "video": True, "type": "srt"}}},
     state_before={"inputs": {"NDI1": {"name": "Gone", "type": "ndi"}}},
     expect_then_send=[_get("/inputs/RTSP1/settings"), _get("/inputs/SRT1/settings"), _get("/inputs/analog-a/settings")])
_tel("input-created", "/inputs", {"status": "ok", "result": "SRT2"}, {}, expect_then_send=[_get("/inputs")])
_tel("input-settings-srt", "/inputs/SRT1/settings", {"status": "ok", "result": _srt_in},
     {"input_settings": {"SRT1": {"audio": {"delay": 0},
                                  "video": {"nosignal": {"image": "", "timeout": 5}, "force_full_color_range": True,
                                            "hwaccel_decoding": True},
                                  "srt": {"mode": "listener", "latency": 80, "keylength": 128, "port": 1024}}}})
_tel("input-settings-analog", "/inputs/analog-a/settings",
     {"status": "ok", "result": {"audio": {"delay": 0},
                                 "local_audio": {"gain": 27, "mute": False, "phantom_power": True, "stereo_pair": False,
                                                 "channels": {"channelA": {"gain": 40, "mute": True},
                                                              "channelB": {"gain": 41, "mute": False}}}}},
     {"input_settings": {"analog-a": {"audio": {"delay": 0},
                                      "local_audio": {"gain": 27, "mute": False, "phantom_power": True,
                                                      "stereo_pair": False,
                                                      "channels": {"channelA": {"gain": 40, "mute": True},
                                                                   "channelB": {"gain": 41, "mute": False}}}}}},
     state_before={"input_settings": {"analog-a": {"local_audio": {"input_type": "XLR"}}}})
_tel("input-settings-rtsp-dotted-id", "/inputs/D2P1.rtsp/settings",
     {"status": "ok", "result": {"rtsp": {"url": "rtsp://10.10.10.10:8554/s", "username": "", "password": "x",
                                          "transport": "tcp"}}},
     {"input_settings": {"D2P1.rtsp": {"rtsp": {"url": "rtsp://10.10.10.10:8554/s", "username": "", "transport": "tcp"}}}})

_tel("outputs", "/outputs", {"status": "ok", "result": [{"id": "D1", "name": "HDMI"}]},
     {"outputs": {"D1": {"name": "HDMI"}}, "output_sources": {"D1": "SRT1"}},
     state_before={"outputs": {"D2": {"name": "Gone"}}, "output_sources": {"D1": "SRT1"}})
_tel("output-source", "/outputs/D1/settings?source=Program%20B", _OK, {"output_sources": {"D1": "Program B"}})

_tel("recorders", "/recorders", {"status": "ok", "result": [{"id": "1", "name": "HDMI-A", "multisource": False},
                                                            {"id": "m1", "name": "Multitrack", "multisource": True}]},
     {"recorders": {"1": {"name": "HDMI-A", "multisource": False}, "m1": {"name": "Multitrack", "multisource": True}}},
     expect_then_send=[_get("/recorders/1/archive/files"), _get("/recorders/m1/archive/files")])
_tel("recorders-status", "/recorders/status", {"status": "ok", "result": [
    {"id": "1", "name": "HDMI-A", "status": {"state": "started", "duration": 58, "active": "1", "total": "1"}},
    {"id": "2", "name": "HDMI-B", "status": {"state": "stopped"}}]},
     {"recorder_status": {"1": {"name": "HDMI-A", "state": "started", "duration": 58, "active": "1", "total": "1"},
                          "2": {"name": "HDMI-B", "state": "stopped"}}},
     state_before={"recorder_status": {"2": {"name": "HDMI-B", "state": "started", "duration": 12}}})
_tel("recorder-status", "/recorders/2/status", {"status": "ok", "result": {"state": "stopped"}},
     {"recorder_status": {"2": {"name": "HDMI-B", "state": "stopped"}}},
     state_before={"recorder_status": {"2": {"name": "HDMI-B", "state": "started", "duration": 12, "active": "1",
                                             "total": "1"}}})
_file = {"id": "VGA.1733956350.HDMI-A.mp4", "name": "HDMI-A_Dec11_17-32-30", "extension": "mp4", "recording": False,
         "downloaded": False, "created": "2024-12-11T17:32:30-0500", "duration": 1040, "size": 407160404,
         "recorder": "1", "uploading": False, "event_id": ""}
_file_state = {"VGA.1733956350.HDMI-A.mp4": {"name": "HDMI-A_Dec11_17-32-30", "extension": "mp4", "recording": False,
                                             "downloaded": False, "created": "2024-12-11T17:32:30-0500",
                                             "duration": 1040, "size": 407160404, "uploading": False, "event_id": ""}}
_tel("recorder-files", "/recorders/1/archive/files", {"status": "ok", "result": [_file]},
     {"recorder_files": {"1": _file_state}},
     state_before={"recorder_files": {"1": {"VGA.1.deleted.mp4": {"name": "deleted"}}}})
# A page adds to the list without replacing it.
_tel("recorder-files-page", "/recorders/1/archive/files?from=0&limit=1", {"status": "ok", "result": [_file]},
     {"recorder_files": {"1": dict(_file_state, **{"VGA.2.HDMI-A.mp4": {"name": "older"}})}},
     state_before={"recorder_files": {"1": {"VGA.2.HDMI-A.mp4": {"name": "older"}}}})

_event = {"id": "782ec0f4bbcc48e2a42a44eea5e69dc5", "status": "scheduled", "title": "Example event",
          "start": 1592571600, "finish": 1592573400, "tags": "test", "recorders": [{"id": "1"}]}
_event_state = {"status": "scheduled", "title": "Example event", "start": 1592571600, "finish": 1592573400,
                "tags": "test", "recorders": {"0": {"id": "1"}}}
_tel("events", "/schedule/events", {"status": "ok", "result": [_event]},
     {"events": {"782ec0f4bbcc48e2a42a44eea5e69dc5": _event_state}},
     state_before={"events": {"old": {"status": "finished"}}})
_tel("event", "/schedule/events/ongoing", {"status": "ok", "result": dict(_event, status="running")},
     {"events": {"782ec0f4bbcc48e2a42a44eea5e69dc5": {"status": "running", "title": "Example event",
                                                       "start": 1592571600, "finish": 1592573400, "tags": "test"}}})
_tel("adhoc-session", "/schedule/events/adhoc/session",
     '{"id":"u54321","name":"John Doe","created":1592581765,"expired":1592581865}',
     {"adhoc_session": {"id": "u54321", "name": "John Doe", "created": 1592581765, "expired": 1592581865}})

_tel("system-status", "/system/status", {"status": "ok", "result": {
    "date": "2025-02-14T08:41:09-05:00", "uptime": 5490, "cpuload": 25, "cpuload_high": False, "cputemp": 57,
    "cputemp_threshold": 70}},
     {"system": {"date": "2025-02-14T08:41:09-05:00", "uptime": 5490, "cpuload": 25, "cpuload_high": False,
                 "cputemp": 57.0, "cputemp_threshold": 70.0}})
_tel("firmware", "/system/firmware", {"status": "ok", "result": {"version": "4.24.1", "revision": "250107_1e4514f",
                                                                 "product_id": 44, "product_name": "Pearl Mini"}},
     {"firmware": {"version": "4.24.1", "revision": "250107_1e4514f", "product_id": 44, "product_name": "Pearl Mini"}})
_tel("firmware-version", "/system/firmware/version", {"status": "ok", "result": "4.24.1"},
     {"firmware": {"version": "4.24.1"}})
_tel("ident", "/system/ident", {"status": "ok", "result": {"name": "", "location": "Home",
                                                           "description": "Pearl Mini at Home"}},
     {"ident": {"name": "", "location": "Home", "description": "Pearl Mini at Home"}})
_tel("connectivity", "/system/connectivity/details", {"status": "ok", "result": {
    "external_ip": "174.115.41.91", "mdns": "GSAA495529", "dns": "error", "http": "ok", "https": "ok",
    "captive_portal": "ok", "icmp": "error", "epiphan_edge": "ok", "vtun": "ok"}},
     {"connectivity": {"external_ip": "174.115.41.91", "mdns": "GSAA495529", "dns": "error", "http": "ok", "https": "ok",
                       "captive_portal": "ok", "icmp": "error", "epiphan_edge": "ok", "vtun": "ok"}})
_tel("speedtest", "/system/connectivity/tools/speedtest?mode=uplink&protocol=tcp&timeout=30",
     {"status": "ok", "result": {"protocol": "tcp", "mode": "uplink", "bandwidth": 91318568,
                                 "bitrate_limit": 1000000000, "duration": 10}},
     {"speedtest": {"protocol": "tcp", "mode": "uplink", "bandwidth": 91318568, "bitrate_limit": 1000000000,
                    "duration": 10}},
     state_before={"speedtest": {"protocol": "udp", "udp_loss": 3}})
_tel("single-touch-list", "/system/singletouchcontrol", {"status": "ok", "result": [{"id": "0"}]}, {},
     expect_then_send=[_get("/system/singletouchcontrol/0/state")])
_tel("single-touch-state", "/system/singletouchcontrol/0/state", {"status": "ok", "result": {
    "pressed": False, "status": True, "recorders": {"total": 5, "active": 0, "success": 5},
    "publishers": {"total": 4, "active": 1, "success": 4}}},
     {"single_touch": {"0": {"pressed": False, "status": True, "recorders": {"total": 5, "active": 0, "success": 5},
                             "publishers": {"total": 4, "active": 1, "success": 4}}}})
_tel("presets", "/system/presets?details=true", {"status": "ok", "result": [
    {"name": "Default", "description": "Default profile", "sections": ["all"], "readonly": True},
    {"name": "Studio.A", "description": "", "sections": ["system", "channels"],
     "hash": "4dfdea2c1ce6f14e46c7f7464d702358", "size": 29696, "readonly": False}]},
     {"presets": {"Default": {"description": "Default profile", "sections": {"0": "all"}, "readonly": True},
                  "Studio.A": {"description": "", "sections": {"0": "system", "1": "channels"},
                               "hash": "4dfdea2c1ce6f14e46c7f7464d702358", "size": 29696, "readonly": False}}},
     state_before={"presets": {"12": {"description": "", "readonly": False}}})
_tel("storages", "/system/storages", {"status": "ok", "result": [{"id": "main"}, {"id": "external"}, {"id": "maintenance"}]},
     {}, expect_then_send=[_get("/system/storages/main/status"), _get("/system/storages/external/status"),
                           _get("/system/storages/maintenance/status"), _get("/system/storages/external/transfer/status")])
_tel("storage-status", "/system/storages/main/status",
     {"status": "ok", "result": {"state": "ready", "total": 15809413120, "free": 11821019136}},
     {"storages": {"main": {"state": "ready", "total": 15809413120, "free": 11821019136}}})
_tel("storage-transfer", "/system/storages/external/transfer/status", {"status": "ok", "result": {
    "state": "completed", "session": {"started": "2025-01-10T18:03:04-05:00", "completed": "2025-01-10T18:03:07-05:00",
                                      "total": {"count": 3, "size": 20113170},
                                      "processed": {"count": 3, "size": 20113170}},
    "current_time": "2025-01-10T18:03:13-05:00"}},
     {"storages": {"external": {"state": "ready", "transfer": {
         "state": "completed", "started": "2025-01-10T18:03:04-05:00", "completed": "2025-01-10T18:03:07-05:00",
         "total_count": 3, "total_size": 20113170, "processed_count": 3, "processed_size": 20113170}}}},
     state_before={"storages": {"external": {"state": "ready", "transfer": {"state": "running", "processed_count": 1}}}})
_tel("afu-status", "/afu/status", {"status": "ok", "result": [{"id": "0", "status": {
    "state": "uploading", "protocol": "webdav", "queue": {"files": 1, "size": 184323243},
    "file": {"recorder": "1", "id": "VGA.1736858583.Test channel.mp4", "uploaded": 0, "size": 184323243}}}]},
     {"afu": {"0": {"state": "uploading", "protocol": "webdav", "queue_files": 1, "queue_size": 184323243,
                    "file": {"recorder": "1", "id": "VGA.1736858583.Test channel.mp4", "uploaded": 0,
                             "size": 184323243}}}},
     state_before={"afu": {"0": {"state": "error", "error": "Connection refused"}}})
_tel("afu-destination", "/afu/0/status", {"status": "ok", "result": {
    "state": "idle", "protocol": "webdav", "queue": {"files": 0, "size": 0}}},
     {"afu": {"0": {"state": "idle", "protocol": "webdav", "queue_files": 0, "queue_size": 0}}},
     state_before={"afu": {"0": {"state": "uploading", "file": {"id": "x"}}}})
