# Kiloview NDI converters (kiloview-ndi): one vector per command, a second for
# each command both dialects share (on an N5), and telemetry vectors for every
# rule. Requests are as the documents give them: /api/v1/<module>/<method> on
# N3/N4/N30/N40 (NDI Device HTTP API Version 1.0) and
# /api/<module>/<method>.json on N5/N6 (N5 & N6 API version 3.0), parameters
# posted as a JSON object (Version 1.0 section 1.2, option C). Only the first
# request of a command is compared; the read-backs after it go once the
# device answers.
import json as _json

KV = "kiloview-ndi"
_OK = '{"result":"ok"}'


def _kv(command, input, method, target, body=None, model=None, file=None, **extra):
    request = {"method": method, "target": target}
    if body is not None:
        request["body"] = body
    v = {"spec": KV, "command": command, "input": input, "expect_request": request, **extra}
    if model:
        v["model"] = model
    if file:
        v["file"] = file
    V.append(v)


def _v3(command, input, method, target, body=None, **extra):
    """The same command on an N5, written to <command>-v3."""
    _kv(command, input, method, target, body, model="n5", file=command + "-v3", **extra)


def _t(name, path, data, state, model=None, request=None, **extra):
    body = data if isinstance(data, str) else _json.dumps({"result": "ok", "data": data})
    inbound = {"path": path, "body": body}
    if request is not None:
        inbound["request"] = request
    v = {"spec": KV, "telemetry": name, "inbound_http": inbound, "expect_state": state, **extra}
    if model:
        v["model"] = model
    V.append(v)


# Sign-in (V1.0 2.1; v3.0 1.1).
_kv("authorize", {"username": "api", "password": "p@ss word"}, "POST", "/api/v1/user/authorize",
    '{"username":"api","password":"p@ss word"}',
    http_reply={"status": 200, "body": '{"result":"ok","data":{"session":"559dee0bd779a894618d6e044c35a3fc",'
                '"token":"4345cd7b092d762bd4a646a98aa9f8ff"}}'},
    expect_result={"ok": {"kind": "value", "value": {"session": "559dee0bd779a894618d6e044c35a3fc",
                                                      "token": "4345cd7b092d762bd4a646a98aa9f8ff"}}})
_kv("authorize", {"username": "api", "password": "wrong"}, "POST", "/api/v1/user/authorize",
    '{"username":"api","password":"wrong"}', file="authorize-refused",
    http_reply={"status": 200, "body": '{"result":"error","msg":"Invalid user or password"}'},
    expect_result={"error": {"error": "device_error"}})
_v3("authorize", {"username": "api", "password": "p@ss word"}, "POST", "/api/user/authorize.json",
    "user=api&password=p%40ss%20word",
    http_reply={"status": 200, "body": '{"result":"ok","data":{"token":"2e33a9b0b06aa0a01ede70995674ee23","alias":"Admin"}}'},
    expect_result={"ok": {"kind": "value", "value": {"token": "2e33a9b0b06aa0a01ede70995674ee23", "alias": "Admin"}}})

# Mode (V1.0 3; v3.0 2).
_kv("get_mode", {}, "GET", "/api/v1/mode/get",
    http_reply={"status": 200, "body": '{"result":"ok","data":{"mode":"decoder"}}'},
    expect_result={"ok": {"kind": "value", "value": "decoder"}})
_kv("get_mode", {}, "GET", "/api/v1/mode/get", file="get_mode-auth-failed",
    http_reply={"status": 200, "body": '{"result":"auth-failed","msg":"Invalid token"}'},
    expect_result={"error": {"error": "device_error"}})
_v3("get_mode", {}, "GET", "/api/mode/get.json")
_kv("switch_mode", {"mode": "decoder"}, "POST", "/api/v1/mode/switch", '{"mode":"decoder"}',
    http_reply={"status": 200, "body": _OK}, expect_result={"ok": {"kind": "ack"}})
_v3("switch_mode", {"mode": "encoder"}, "POST", "/api/mode/switch.json", '{"mode":"encoder"}')
_kv("get_mode_status", {}, "GET", "/api/v1/mode/status",
    http_reply={"status": 200, "body": '{"result":"error","reason":"not-ready","msg":"starting"}'},
    expect_result={"error": {"error": "device_error"}})
_v3("get_mode_status", {}, "GET", "/api/mode/status.json")

# Encoder (V1.0 4; v3.0 3).
_kv("get_encoder_config", {}, "GET", "/api/v1/encoder/ndi/get_config")
_v3("get_encoder_config", {"ndi_type": "ndihx"}, "POST", "/api/encoder/ndi/get_config.json", '{"types":"ndihx"}')
_kv("set_ndi_group", {"group": "studio-a"}, "POST", "/api/v1/encoder/ndi/set_config", '{"device_group":"studio-a"}')
_v3("set_ndi_group", {"group": "", "ndi_type": "ndifull"}, "POST", "/api/encoder/ndi/set_config.json",
    '{"types":"ndifull","device_group":""}')
_kv("set_ndi_device_name", {"name": "My device"}, "POST", "/api/v1/encoder/ndi/set_config", '{"device_name":"My device"}')
_kv("set_ndi_channel_name", {"name": "Cam \"1\""}, "POST", "/api/v1/encoder/ndi/set_config",
    '{"channel_name":"Cam \\"1\\""}')
_v3("set_ndi_channel_name", {"name": "Channel-HX", "ndi_type": "ndihx"}, "POST", "/api/encoder/ndi/set_config.json",
    '{"types":"ndihx","channel_name":"Channel-HX"}')
_kv("set_ndi_tcp", {}, "POST", "/api/v1/encoder/ndi/set_config", '{"ndi_connection":"tcp"}')
_v3("set_ndi_tcp", {}, "POST", "/api/encoder/ndi/set_config.json", '{"types":"ndifull","ndi_connection":"tcp"}')
_kv("set_ndi_no_multi_tcp", {"ndi_type": "ndihx"}, "POST", "/api/encoder/ndi/set_config.json",
    '{"types":"ndihx","ndi_connection":"no_multi_tcp"}', model="n5")
_kv("set_ndi_multicast", {"netprefix": "239.254.0.0", "netmask": "255.255.0.0"}, "POST",
    "/api/v1/encoder/ndi/set_config", '{"ndi_connection":"multicast","netprefix":"239.254.0.0","netmask":"255.255.0.0"}')
_v3("set_ndi_multicast", {"netprefix": "239.255.0.0", "netmask": "255.255.0.0", "ndi_type": "ndihx"}, "POST",
    "/api/encoder/ndi/set_config.json",
    '{"types":"ndihx","ndi_connection":"multicast","netprefix":"239.255.0.0","netmask":"255.255.0.0"}')
_kv("set_ndi_bandwidth", {"percent": 125}, "POST", "/api/v1/encoder/ndi/set_config", '{"quality":125}')
_kv("set_hx_quality", {"quality": 20000}, "POST", "/api/encoder/ndi/set_config.json",
    '{"types":"ndihx","quality":20000}', model="n5")
_kv("set_ndi_ttl", {"ttl": 127}, "POST", "/api/encoder/ndi/set_config.json", '{"types":"ndifull","ttl":127}', model="n5")
_kv("set_ndi_scaling", {"resolution": "1080P60", "ndi_type": "ndihx"}, "POST", "/api/encoder/ndi/set_config.json",
    '{"types":"ndihx","resolution":"1080P60"}', model="n6")
_kv("set_hx_codec", {"codec": "H265"}, "POST", "/api/encoder/ndi/set_config.json", '{"types":"ndihx","codec":"H265"}',
    model="n5")
_kv("set_hx_version", {"version": 3}, "POST", "/api/encoder/ndi/set_config.json", '{"types":"ndihx","hx_version":3}',
    model="n5")
_kv("get_encoder_status", {}, "GET", "/api/v1/encoder/ndi/status")
_v3("get_encoder_status", {"ndi_type": "ndihx"}, "POST", "/api/encoder/ndi/status.json", '{"types":"ndihx"}')
_kv("get_video_sources", {}, "GET", "/api/encoder/ndi/get_video_source.json", model="n5",
    http_reply={"status": 200, "body": '{"result":"ok","data":["HDMI","USB"]}'},
    expect_result={"ok": {"kind": "value", "value": ["HDMI", "USB"]}})
_kv("select_video_source", {"source": "USB"}, "POST", "/api/encoder/ndi/select_video_source.json",
    '{"video_source":"USB"}', model="n6")
_kv("get_audio", {}, "GET", "/api/v1/encoder/ndi/get_audio")
_v3("get_audio", {}, "GET", "/api/audio/get_audio.json")
_kv("set_audio_source", {"signal": "analog"}, "POST", "/api/v1/encoder/ndi/set_audio", '{"signal":"analog"}')
_kv("set_audio_input", {"signal": "Line In"}, "POST", "/api/audio/set_audio.json", '{"signal":"Line In"}', model="n5")
_kv("set_audio_volume", {"volume": 150}, "POST", "/api/v1/encoder/ndi/set_audio", '{"volume":150}')
_v3("set_audio_volume", {"volume": 0}, "POST", "/api/audio/set_audio.json", '{"volume":0}')
_kv("get_ndi_enabled", {}, "GET", "/api/encoder/ndi/get_NDI_enable.json", model="n5",
    http_reply={"status": 200, "body": '{"result":"ok","data":{"enable":true}}'},
    expect_result={"ok": {"kind": "value", "value": True}})
_kv("set_ndi_enabled", {"enabled": False}, "POST", "/api/encoder/ndi/set_NDI_enable.json", '{"enable":false}',
    model="n5")

# Discovery (V1.0 5; v3.0 4).
_kv("get_ndi_sources", {}, "GET", "/api/v1/decoder/discovery/get")
_v3("get_ndi_sources", {}, "GET", "/api/decoder/discovery/get.json")
_kv("rescan_ndi_sources", {}, "GET", "/api/v1/decoder/discovery/get?force=1")
_v3("rescan_ndi_sources", {}, "GET", "/api/decoder/discovery/get.json?force=1")
_kv("get_manual_discovery", {}, "GET", "/api/v1/decoder/discovery/get_manual_targets")
_v3("get_manual_discovery", {}, "GET", "/api/decoder/discovery/get_manual_targets.json")
_kv("set_manual_discovery", {"ips": ["192.168.100.3", "172.16.10.5"], "groups": ["public", "my-group"]}, "POST",
    "/api/v1/decoder/discovery/set_manual_targets",
    '{"ip":["192.168.100.3","172.16.10.5"],"group_name":["public","my-group"]}')
_v3("set_manual_discovery", {"ips": [], "groups": []}, "POST", "/api/decoder/discovery/set_manual_targets.json",
    '{"ip":[],"group_name":[]}')

# Presets (V1.0 6; v3.0 5).
_kv("get_presets", {}, "GET", "/api/v1/decoder/preset/status")
_v3("get_presets", {}, "GET", "/api/decoder/preset/status.json")
_kv("add_preset", {"preset": 1, "name": "Spark_IO-1916001002 (Channel 1)", "url": "192.168.100.168:5961",
                   "group": "public"}, "POST", "/api/v1/decoder/preset/add",
    '{"id":"1","name":"Spark_IO-1916001002 (Channel 1)","url":"192.168.100.168:5961","group":"public"}')
_v3("add_preset", {"preset": 9, "name": "E1_NDI-1212-111 (Chan 1)", "url": "192.168.2.170:5963"}, "POST",
    "/api/decoder/preset/add.json",
    '{"position":"9","name":"E1_NDI-1212-111 (Chan 1)","url":"192.168.2.170:5963","group":""}')
_kv("remove_preset", {"preset": 3}, "POST", "/api/v1/decoder/preset/remove", '{"id":"3"}')
_v3("remove_preset", {"preset": 3}, "POST", "/api/decoder/preset/remove.json", '{"id":"3"}')
_kv("set_blank_color", {"color": "#ff00ff"}, "POST", "/api/v1/decoder/preset/set_blank", '{"color":"#ff00ff"}')
_v3("set_blank_color", {"color": "#000"}, "POST", "/api/decoder/preset/set_blank.json", '{"color":"#000"}')

# Current decoding (V1.0 7; v3.0 6).
_kv("get_decoding_status", {}, "GET", "/api/v1/decoder/current/status")
_v3("get_decoding_status", {}, "GET", "/api/decoder/current/status.json")
_kv("recall_preset", {"preset": 0}, "POST", "/api/v1/decoder/current/set", '{"id":"0"}')
_v3("recall_preset", {"preset": 4}, "POST", "/api/decoder/current/set.json", '{"id":"4"}')
_kv("decode_source", {"name": "CAM 2 (Channel 1)", "url": "10.0.0.12:5961", "group": "studio"}, "POST",
    "/api/v1/decoder/current/set", '{"name":"CAM 2 (Channel 1)","url":"10.0.0.12:5961"}')
_v3("decode_source", {"name": "CAM 2 (Channel 1)", "url": "10.0.0.12:5961", "group": "studio"}, "POST",
    "/api/decoder/current/set.json", '{"name":"CAM 2 (Channel 1)","url":"10.0.0.12:5961","group":"studio"}')

# Decoder output (V1.0 8; v3.0 7).
_kv("get_switch_timeout", {}, "GET", "/api/v1/decoder/output/get_smoothness",
    http_reply={"status": 200, "body": '{"result":"ok","data":{"timeout":200}}'},
    expect_result={"ok": {"kind": "value", "value": 200}})
_kv("set_switch_timeout", {"timeout_ms": 0}, "POST", "/api/v1/decoder/output/set_smoothness", '{"timeout":0}')
_kv("get_output_resolutions", {}, "GET", "/api/decoder/output/resolutions.json", model="n5")
_kv("get_output_options", {}, "GET", "/api/v1/decoder/output/get")
_v3("get_output_options", {}, "GET", "/api/decoder/output/get.json")
_kv("set_output_deinterlace", {"deinterlace": True}, "POST", "/api/v1/decoder/output/set", '{"resolution":"deint"}')
_kv("set_output_deinterlace", {"deinterlace": False}, "POST", "/api/v1/decoder/output/set", '{"resolution":"auto"}',
    file="set_output_deinterlace-off")
_kv("set_output_frame_rate", {"frame_rate": "29.97"}, "POST", "/api/v1/decoder/output/set", '{"frame_rate":29.97}')
_kv("set_output_resolution", {"resolution": "1080P50"}, "POST", "/api/decoder/output/set.json",
    '{"resolution":"1080P50"}', model="n6")

# Tally (V1.0 9; v3.0 8).
_kv("get_tally", {}, "GET", "/api/v1/tally/status")
_v3("get_tally", {}, "GET", "/api/tally/status.json")
_kv("set_tally", {"program": True, "preview": False}, "POST", "/api/v1/tally/set", '{"pgm":1,"pvw":0}')
_v3("set_tally", {"program": False, "preview": True}, "POST", "/api/tally/set.json", '{"pgm":0,"pvw":1}')
_kv("set_tally_program", {"program": True}, "POST", "/api/v1/tally/set", '{"pgm":1}')
_v3("set_tally_program", {"program": False}, "POST", "/api/tally/set.json", '{"pgm":0}')
_kv("set_tally_preview", {"preview": True}, "POST", "/api/v1/tally/set", '{"pvw":1}')
_v3("set_tally_preview", {"preview": False}, "POST", "/api/tally/set.json", '{"pvw":0}')

# System time (v3.0 9).
_kv("get_system_time", {}, "GET", "/api/sys-time/get.json", model="n5")
_kv("set_system_time", {"ntp": "pool.ntp.org time.example.com", "timezone": "Europe/London",
                        "time": "2026-10-23 16:50:50", "sync": "ntp"}, "POST", "/api/sys-time/set.json",
    '{"ntp":"pool.ntp.org time.example.com","Timezone":"Europe/London","time":"2026-10-23 16:50:50","timetype":"ntp"}',
    model="n5")

# System (V1.0 10; v3.0 10).
_kv("get_system_info", {}, "GET", "/api/v1/sys/server_info")
_v3("get_system_info", {}, "GET", "/api/sys/server_info.json")
_kv("get_hostname", {}, "GET", "/api/device/get_hostname.json", model="n6",
    http_reply={"status": 200, "body": '{"result":"ok","data":{"hostname":"N6-123456"}}'},
    expect_result={"ok": {"kind": "value", "value": "N6-123456"}})
_kv("set_hostname", {"hostname": "N6-Stage"}, "POST", "/api/device/set_hostname.json", '{"hostname":"N6-Stage"}',
    model="n6")
_kv("get_discovery_server", {}, "GET", "/api/device/get_discovery_server.json", model="n5")
_kv("set_discovery_server", {"address": "192.168.1.168,192.168.1.169", "enabled": True}, "POST",
    "/api/device/set_discovery_server.json", '{"address":"192.168.1.168,192.168.1.169","enable":true}', model="n5")
_kv("get_version", {}, "GET", "/api/firmware/get.json", model="n5")
_kv("reconnect_ndi", {}, "POST", "/api/v1/sys/reconnect",
    http_reply={"status": 200, "body": _OK}, expect_result={"ok": {"kind": "ack"}})
_kv("reboot", {}, "POST", "/api/v1/sys/reboot")
_v3("reboot", {}, "POST", "/api/sys/reboot.json")
_kv("factory_reset", {}, "POST", "/api/v1/sys/restore")
_v3("factory_reset", {}, "POST", "/api/sys/restore.json")

# Users (v3.0 11).
_kv("list_users", {}, "GET", "/api/users/list.json", model="n5")
_kv("add_user", {"user": "ops", "alias": "Operator", "password": "s3cret", "api": True, "web": False}, "POST",
    "/api/users/add.json", '{"id":"ops","alias":"Operator","api":true,"web":false,"password":"s3cret"}', model="n5")
_kv("set_user_alias", {"user": "ops", "alias": "Show operator"}, "POST", "/api/users/modify.json",
    '{"id":"ops","alias":"Show operator"}', model="n5")
_kv("set_user_permissions", {"user": "ops", "api": False, "web": True}, "POST", "/api/users/modify.json",
    '{"id":"ops","api":false,"web":true}', model="n5")
_kv("set_user_password", {"user": "ops", "password": "n3w"}, "POST", "/api/users/modify.json",
    '{"id":"ops","password":"n3w"}', model="n5")
_kv("remove_user", {"user": "ops"}, "POST", "/api/users/remove.json", '{"id":"ops"}', model="n5")

# Network (V1.0 11; v3.0 12).
_kv("get_network", {}, "GET", "/api/v1/network/get")
_v3("get_network", {}, "GET", "/api/network/get.json")
_kv("set_network_dhcp", {"interface": "eth0"}, "POST", "/api/v1/network/set", '{"device":"eth0","dynamic":"y"}')
_v3("set_network_dhcp", {"interface": "eth0"}, "POST", "/api/network/set.json", '{"device":"eth0","dynamic":"y"}')
_kv("set_network_static", {"interface": "eth0", "ip": "192.168.2.160", "mask": "255.255.255.0", "gateway": "192.168.2.1",
                           "dns": "8.8.8.8;4.4.4.4"}, "POST", "/api/v1/network/set",
    '{"device":"eth0","dynamic":"n","ip":"192.168.2.160","mask":"255.255.255.0","gw":"192.168.2.1","dns":"8.8.8.8;4.4.4.4"}')
_v3("set_network_static", {"interface": "eth0", "ip": "10.1.1.20", "mask": "255.255.0.0", "gateway": "",
                           "dns": ""}, "POST", "/api/network/set.json",
    '{"device":"eth0","dynamic":"n","ip":"10.1.1.20","mask":"255.255.0.0","gw":"","dns":""}')
_kv("set_network_mac", {"interface": "eth0", "mac": "68:3A:7F:8C:A7:96"}, "POST", "/api/v1/network/set",
    '{"device":"eth0","mac":"68:3A:7F:8C:A7:96"}')
_v3("set_network_mac", {"interface": "eth0", "mac": "68:3A:7F:8C:A7:97"}, "POST", "/api/network/set.json",
    '{"device":"eth0","mac":"68:3A:7F:8C:A7:97"}')
_kv("set_failsafe_address", {"interface": "eth0", "ip": "192.168.100.168", "mask": "255.255.255.0", "gateway": ""},
    "POST", "/api/v1/network/set", '{"device":"eth0","ip2":"192.168.100.168","mask2":"255.255.255.0","gw2":""}')
_kv("set_network_address3", {"interface": "eth0", "ip": "0.0.0.0", "mask": "255.255.255.0", "gateway": ""},
    "POST", "/api/v1/network/set", '{"device":"eth0","ip3":"0.0.0.0","mask3":"255.255.255.0","gw3":""}')


# Telemetry: one vector per rule, with the documents' example replies.
_t("mode", "/api/v1/mode/get", {"mode": "encoder"}, {"mode": {"current": "encoder"}})
_t("mode-v3", "/api/mode/get.json", {"mode": "decoder"}, {"mode": {"current": "decoder"}}, model="n5")
_t("mode-ready", "/api/v1/mode/status", {"mode": "decoder", "status": "ready"},
   {"mode": {"current": "decoder", "status": "ready", "ready": True}})
_t("mode-not-ready", "/api/mode/status.json", '{"result":"error","msg":"waitChangeModeError"}',
   {"mode": {"ready": False}}, model="n6")
_t("encoder-config", "/api/v1/encoder/ndi/get_config",
   {"device_group": "public", "device_name": "Spark IO", "channel_name": "1916001002", "ndi_connection": "multicast",
    "netprefix": "239.254.0.0", "netmask": "255.255.0.0", "quality": 100},
   {"encoder": {"config": {"ndi": {"device_group": "public", "device_name": "Spark IO", "channel_name": "1916001002",
                                   "ndi_connection": "multicast", "netprefix": "239.254.0.0",
                                   "netmask": "255.255.0.0", "quality": 100}}}})
_V3_CONFIG = {"device_group": "", "netmask": "255.255.0.0", "resolution": "none",
              "resolution_list": [{"name": "1920x1080P 60Hz", "value": "1080P60"},
                                  {"name": "1920x1080P 50Hz", "value": "1080P50"}],
              "netprefix": "239.255.0.0", "channel_name": "Channel-HX", "ttl": 127, "quality": 2000,
              "ndi_connection": "tcp", "video_source": "HDMI", "codec": "H265", "hx_version": 3}
_t("encoder-config-v3", "/api/encoder/ndi/get_config.json", _V3_CONFIG,
   {"encoder": {"config": {"ndihx": {"device_group": "", "channel_name": "Channel-HX", "ndi_connection": "tcp",
                                     "netprefix": "239.255.0.0", "netmask": "255.255.0.0", "quality": 2000,
                                     "ttl": 127, "scaling": "none", "video_source": "HDMI", "codec": "H265",
                                     "hx_version": 3,
                                     "resolutions": {"0": {"name": "1920x1080P 60Hz", "value": "1080P60"},
                                                     "1": {"name": "1920x1080P 50Hz", "value": "1080P50"}}}}}},
   model="n5", request={"types": "ndihx"})
# A shorter list replaces the longer one.
_t("encoder-resolutions-replaced", "/api/encoder/ndi/get_config.json",
   {"resolution_list": [{"name": "1280x720P 60Hz", "value": "720P60"}]},
   {"encoder": {"config": {"ndifull": {"resolutions": {"0": {"name": "1280x720P 60Hz", "value": "720P60"}}}}}},
   model="n6", request={"types": "ndifull"},
   state_before={"encoder": {"config": {"ndifull": {"resolutions": {
       "0": {"name": "1920x1080P 60Hz", "value": "1080P60"}, "1": {"name": "1920x1080P 50Hz", "value": "1080P50"}}}}}})
_t("encoder-status", "/api/v1/encoder/ndi/status",
   {"video_signal": "hdmi", "resolution": "1920x1080p 59.94Hz", "xRes": 1920, "yRes": 1080, "frame_rate": 59.94,
    "interlaced": False, "bitrate": 125000, "audio_format": "48000 Hz/Stereo", "audio_signal": "embedded",
    "audio_sampling": 48000, "audio_channels": 2},
   {"encoder": {"status": {"ndi": {"video_signal": "hdmi", "video_present": True,
                                   "resolution": "1920x1080p 59.94Hz", "width": 1920, "height": 1080,
                                   "frame_rate": 59.94, "interlaced": False, "bitrate_kbps": 125000,
                                   "audio_format": "48000 Hz/Stereo", "audio_signal": "embedded",
                                   "audio_sample_rate": 48000, "audio_channels": 2}}}})
_t("encoder-status-v3", "/api/encoder/ndi/status.json",
   {"audio_sampling": 48000, "audio_format": "AAC", "audio_channels": 2, "audio_signal": "embedded",
    "samp_chan": "48KHz/2ch", "video_signal": False, "yRes": 1080, "xRes": 1920, "frame_rate": 30,
    "bitrate": 383222, "video_source": "HDMI", "interlaced": False, "serial_number": "1233211234567",
    "resolution": "1920x1080@30", "mode_version": "H.264 NDI|HX2", "src_resolution": "1920x1080@60",
    "src_frame_rate": 60, "src_xRes": 1920, "src_yRes": 1080},
   {"encoder": {"status": {"ndihx": {"video_present": False, "resolution": "1920x1080@30", "width": 1920,
                                     "height": 1080, "frame_rate": 30.0, "interlaced": False,
                                     "bitrate_kbps": 383222, "audio_format": "AAC", "audio_signal": "embedded",
                                     "audio_sample_rate": 48000, "audio_channels": 2, "audio_summary": "48KHz/2ch",
                                     "serial_number": "1233211234567", "video_source": "HDMI",
                                     "video_format": "H.264 NDI|HX2", "source_resolution": "1920x1080@60",
                                     "source_frame_rate": 60.0, "source_width": 1920, "source_height": 1080}}}},
   model="n5", request={"types": "ndihx"})
_t("video-sources", "/api/encoder/ndi/get_video_source.json", ["HDMI", "USB"],
   {"encoder": {"video_sources": {"0": "HDMI", "1": "USB"}}}, model="n6")
_t("ndi-enabled", "/api/encoder/ndi/get_NDI_enable.json", {"enable": True},
   {"encoder": {"ndi_enabled": True}}, model="n5")
_t("audio", "/api/v1/encoder/ndi/get_audio", {"signal": "embedded", "volume": 100},
   {"audio": {"signal": "embedded", "volume": 100}})
_t("audio-v3", "/api/audio/get_audio.json", {"signal": "HDMI", "volume": 100, "channels": 2},
   {"audio": {"signal": "HDMI", "volume": 100, "channels": 2}}, model="n5")
_t("sources", "/api/v1/decoder/discovery/get",
   [{"group": "public", "name": "Spark_IO-1916001002 (Channel 1)", "device_name": "Spark_IO-1916001002",
     "channel_name": "Channel 1", "url": "192.168.100.168:5961"}],
   {"decoder": {"sources": {"0": {"name": "Spark_IO-1916001002 (Channel 1)", "group": "public",
                                  "device_name": "Spark_IO-1916001002", "channel_name": "Channel 1",
                                  "url": "192.168.100.168:5961"}}}},
   state_before={"decoder": {"sources": {"1": {"name": "Gone"}}}})
_t("sources-v3", "/api/decoder/discovery/get.json?force=1",
   [{"enable": 1, "id": "1", "device_name": "E1_NDI-1212-111", "url": "192.168.2.170:5963", "series": "NDI",
     "group_name": "public", "ip": "192.168.2.170", "group": "public", "channel_name": "Chan 1",
     "original_url": "192.168.2.170:5963", "name": "E1_NDI-1212-111 (Chan 1)", "port": 5963}],
   {"decoder": {"sources": {"0": {"name": "E1_NDI-1212-111 (Chan 1)", "group": "public",
                                  "device_name": "E1_NDI-1212-111", "channel_name": "Chan 1",
                                  "url": "192.168.2.170:5963", "ip": "192.168.2.170", "port": 5963,
                                  "original_url": "192.168.2.170:5963", "series": "NDI"}}}},
   model="n5")
_t("manual-ips", "/api/v1/decoder/discovery/get_manual_targets",
   {"ip": ["192.168.100.3", "172.16.10.5"], "group_name": ["public", "my-group"]},
   {"decoder": {"manual_targets": {"ips": {"0": "192.168.100.3", "1": "172.16.10.5"},
                                   "groups": {"0": "public", "1": "my-group"}}}},
   state_before={"decoder": {"manual_targets": {"ips": {"2": "10.0.0.9"}, "groups": {"2": "old"}}}})
_t("manual-groups-v3", "/api/decoder/discovery/get_manual_targets.json", {"ip": [], "group_name": ["private_group"]},
   {"decoder": {"manual_targets": {"groups": {"0": "private_group"}}}}, model="n5",
   state_before={"decoder": {"manual_targets": {"ips": {"0": "10.0.0.9"}}}})
_t("presets", "/api/v1/decoder/preset/status",
   [{"id": "1", "enable": 1, "group": "public", "name": "Spark_IO-1916001002 (Channel 1)",
     "device_name": "Spark_IO-1916001002", "channel_name": "Channel 1", "url": "192.168.100.168:5961",
     "ip": "192.168.100.168", "online": "on", "current": True},
    {"id": "2", "enable": 0, "current": False},
    {"id": "0", "current": False, "color": "#000000"}],
   {"decoder": {"presets": {
       "1": {"enabled": True, "group": "public", "name": "Spark_IO-1916001002 (Channel 1)",
             "device_name": "Spark_IO-1916001002", "channel_name": "Channel 1", "url": "192.168.100.168:5961",
             "ip": "192.168.100.168", "online": True, "current": True},
       "2": {"enabled": False, "current": False},
       "0": {"current": False, "color": "#000000"}}}},
   state_before={"decoder": {"presets": {"2": {"name": "Removed"}}}})
_t("decoding", "/api/v1/decoder/current/status",
   {"name": "Spark_IO-1916001002 (Channel 1)", "url": "192.168.100.168:5961", "ip": "192.168.100.168",
    "online": True, "resolution": "1920x1080p 59.94Hz", "codec": "SHQ7", "xRes": 1920, "yRes": 1080,
    "frame_rate": 59.94, "inst_frame_rate": 59.93, "interlaced": False, "de_interlace": False, "bitrate": 125000,
    "audio_format": "48KHz / 2CH", "audio_sampling": 48000, "audio_channels": 2, "warning": ""},
   {"decoder": {"current": {"name": "Spark_IO-1916001002 (Channel 1)", "url": "192.168.100.168:5961",
                            "ip": "192.168.100.168", "online": True, "resolution": "1920x1080p 59.94Hz",
                            "codec": "SHQ7", "width": 1920, "height": 1080, "frame_rate": 59.94,
                            "inst_frame_rate": 59.93, "interlaced": False, "deinterlaced": False,
                            "bitrate_kbps": 125000, "audio_format": "48KHz / 2CH", "audio_sample_rate": 48000,
                            "audio_channels": 2, "warning": ""}}})
_t("decoding-v3", "/api/decoder/current/status.json",
   {"isFull": False, "name": "(Blank)", "url": "", "ip": "0.0.0.0", "online": False, "resolution": "",
    "codec": "NDI-FULL", "xRes": 0, "yRes": 0, "frame_rate": 0, "inst_frame_rate": 0, "bitrate": 0,
    "audio_format": "", "audio_sampling": 0, "audio_channels": 0, "warning": "WARN:offline", "ptz": 1,
    "video_mode": "hdmi"},
   {"decoder": {"current": {"full_ndi": False, "name": "(Blank)", "url": "", "ip": "0.0.0.0", "online": False,
                            "resolution": "", "codec": "NDI-FULL", "width": 0, "height": 0, "frame_rate": 0.0,
                            "inst_frame_rate": 0.0, "bitrate_kbps": 0, "audio_format": "",
                            "audio_sample_rate": 0, "audio_channels": 0, "warning": "WARN:offline",
                            "ptz_icon": True, "video_mode": "hdmi"}}}, model="n6")
_t("output", "/api/v1/decoder/output/get", {"resolution": "auto", "frame_rate": 59.94, "sample_rate": 48000},
   {"decoder": {"output": {"resolution": "auto", "frame_rate": 59.94, "sample_rate": 48000}}})
_t("output-v3", "/api/decoder/output/get.json",
   {"enable": 1, "frameRate": 0, "audio_format": 48000, "hdmi_mode": "hdmi", "resolution": "1080P50",
    "frame_rate": 59.94, "sample_rate": 48000, "colorspace": "auto", "channels": 2},
   {"decoder": {"output": {"enable": 1, "frame_rate_mode": 0, "audio_format": 48000, "hdmi_mode": "hdmi",
                           "resolution": "1080P50", "frame_rate": 59.94, "sample_rate": 48000,
                           "colorspace": "auto", "channels": 2}}}, model="n5")
_t("output-resolutions", "/api/decoder/output/resolutions.json",
   [{"name": "1920x1080P 60Hz", "value": "1080P60"}, {"name": "1920x1080P 50Hz", "value": "1080P50"}],
   {"decoder": {"output": {"resolutions": {"0": {"name": "1920x1080P 60Hz", "value": "1080P60"},
                                           "1": {"name": "1920x1080P 50Hz", "value": "1080P50"}}}}}, model="n5")
_t("switch-timeout", "/api/v1/decoder/output/get_smoothness", {"timeout": 200},
   {"decoder": {"switch_timeout_ms": 200}})
_t("tally", "/api/v1/tally/status", {"pgm": 1, "pvw": 0}, {"tally": {"program": True, "preview": False}})
_t("tally-v3", "/api/tally/status.json", {"pgm": 0, "pvw": 1}, {"tally": {"program": False, "preview": True}},
   model="n5")
_t("system", "/api/v1/sys/server_info",
   {"addr": "192.168.0.100", "port": 443, "name": "https://192.168.0.100", "persis": "4H 15M 32S",
    "start_time": "2020-05-03 12:10:09", "cpu_cores": 2, "cpu_payload": 33, "mem_used": 124.06, "mem_total": 620.00},
   {"system": {"address": "192.168.0.100", "port": 443, "url": "https://192.168.0.100", "uptime": "4H 15M 32S",
               "start_time": "2020-05-03 12:10:09", "cpu_cores": 2, "cpu_load": 33.0, "memory_used_mb": 124.06,
               "memory_total_mb": 620.0}})
_t("system-v3", "/api/sys/server_info.json",
   {"cpu_cores": 8, "cpu_payload": 22, "mem_used": 300.15, "addr": "192.168.40.161", "persis": "1H 20M 23S",
    "port": "443", "name": "https://192.168.40.161", "start_time": "2023-02-28 15:47:21", "mem_total": 1889.72},
   {"system": {"address": "192.168.40.161", "port": 443, "url": "https://192.168.40.161", "uptime": "1H 20M 23S",
               "start_time": "2023-02-28 15:47:21", "cpu_cores": 8, "cpu_load": 22.0, "memory_used_mb": 300.15,
               "memory_total_mb": 1889.72}}, model="n6")
_t("time", "/api/sys-time/get.json",
   {"ntp": "ntp1.aliyun.com ntp2.aliyun.com", "Timezone": "Asia/Shanghai", "time": "2023-02-28 16:50:50",
    "timetype": "pc"},
   {"time": {"ntp_servers": "ntp1.aliyun.com ntp2.aliyun.com", "timezone": "Asia/Shanghai",
             "current": "2023-02-28 16:50:50", "sync": "pc"}}, model="n5")
_t("hostname", "/api/device/get_hostname.json", {"hostname": "N6-123456"},
   {"device": {"hostname": "N6-123456"}}, model="n6")
_t("discovery-server", "/api/device/get_discovery_server.json", {"address": "192.168.1.168", "enable": True},
   {"discovery_server": {"address": "192.168.1.168", "enabled": True}}, model="n5")
_t("version", "/api/firmware/get.json", {"softwareVersion": "1.01.0001", "hardwareVersion": "1.0", "ndiVersion": "5.1.3"},
   {"device": {"software_version": "1.01.0001", "hardware_version": "1.0", "ndi_version": "5.1.3"}}, model="n5")
_t("users", "/api/users/list.json",
   [{"alias": "Admin", "api": True, "create_time": "1970-01-01 00:00:00", "id": "admin", "web": True},
    {"alias": "Ops", "api": True, "create_time": "2026-10-01 09:00:00", "id": "ops.team", "web": False}],
   {"users": {"admin": {"alias": "Admin", "api": True, "web": True, "created": "1970-01-01 00:00:00"},
              "ops.team": {"alias": "Ops", "api": True, "web": False, "created": "2026-10-01 09:00:00"}}},
   model="n5", state_before={"users": {"gone": {"alias": "Gone"}}})
_t("network", "/api/v1/network/get",
   [{"device": "eth0", "state": "up", "ip": "192.168.2.160", "mask": "255.255.255.0", "mac": "68:3A:7F:8C:A7:96",
     "gw": "192.168.2.1", "dynamic": "y", "dns": "8.8.8.8; 4.4.4.4", "ip2": "192.168.100.168",
     "mask2": "255.255.255.0", "gw2": "", "ip3": "0.0.0.0", "mask3": "255.255.255.0", "gw3": ""}],
   {"network": {"eth0": {"state": "up", "ip": "192.168.2.160", "mask": "255.255.255.0", "mac": "68:3A:7F:8C:A7:96",
                         "gateway": "192.168.2.1", "dhcp": True, "dns": "8.8.8.8; 4.4.4.4",
                         "failsafe_ip": "192.168.100.168", "failsafe_mask": "255.255.255.0",
                         "failsafe_gateway": "", "ip3": "0.0.0.0", "mask3": "255.255.255.0", "gateway3": ""}}})
_t("network-v3", "/api/network/get.json",
   [{"device": "eth0", "state": "down", "ip": "10.1.1.20", "mask": "255.255.0.0", "mac": "68:3A:7F:8C:A7:97",
     "gw": "", "dynamic": "n", "dns": ""}],
   {"network": {"eth0": {"state": "down", "ip": "10.1.1.20", "mask": "255.255.0.0", "mac": "68:3A:7F:8C:A7:97",
                         "gateway": "", "dhcp": False, "dns": ""}}}, model="n5")
