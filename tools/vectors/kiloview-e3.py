# Kiloview E3 / RE-3 (kiloview-e3): one vector per command over the E3 web
# API, from "E3 API Documentation, Version 1.21.0008": requests under /api,
# JSON bodies in the documented field order (written here with json.dumps,
# compact), writes answered {"result":"ok","msg":null}. Telemetry from the
# document's example replies, and the read-backs each successful write queues.
KE3 = "kiloview-e3"
_KE3_OK = {"http_reply": {"status": 200, "body": '{"result":"ok","msg":null}'},
           "expect_result": {"ok": {"kind": "ack"}}}
_KE3_ERR = {"http_reply": {"status": 200, "body": '{"result":"error","msg":"Parameter error"}'},
            "expect_result": {"error": {"error": "device_error"}}}


def _ke3_q(v):
    from urllib.parse import quote
    return quote(str(v), safe="")


def _ke3_j(obj):
    return json.dumps(obj, separators=(",", ":"))


def _ke3(command, input, method, target, body=None, **extra):
    request = {"method": method, "target": target}
    if body is not None:
        request["body"] = body if isinstance(body, str) else _ke3_j(body)
    V.append({"spec": KE3, "command": command, "input": input, "expect_request": request, **extra})


def _ke3_form(parts):
    """multipart/form-data as the core writes it: the boundary derived from
    the parts (SPEC.md §4, "Binary request bodies")."""
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


def _ke3_upload(command, input, target, parts, **extra):
    body, ctype = _ke3_form(parts)
    V.append({"spec": KE3, "command": command, "input": input,
              "expect_request": {"method": "POST", "target": target, "body_hex": body.hex(), "content_type": ctype},
              **extra})


# ── Users and API authentication (1.1) ───────────────────────────────────
_ke3("login", {"username": "admin", "password": "Admin123"}, "POST", "/api/systemctrl/users/login",
     {"username": "admin", "password": "Admin123"},
     http_reply={"status": 200, "body": _ke3_j({"result": "ok", "data": {
         "token": "0621da14f9d6546688cf87c5eab9c2f9", "alias": "Admin", "changed": True}, "msg": None})},
     expect_result={"ok": {"kind": "value", "value": "0621da14f9d6546688cf87c5eab9c2f9"}})
_ke3("login", {"username": "admin", "password": "wrong"}, "POST", "/api/systemctrl/users/login",
     {"username": "admin", "password": "wrong"},
     http_reply={"status": 200, "body": '{"result":"error","msg":"Incorrect username or password"}'},
     expect_result={"error": {"error": "device_error"}}, file="login-refused")
_ke3("list_users", {}, "GET", "/api/systemctrl/users/list")
_ke3("add_user", {"username": "test_user", "password": "qwer1234", "alias": "test_user", "web": True, "api": True},
     "POST", "/api/systemctrl/users/add",
     {"id": "test_user", "password": "qwer1234", "confirmNewPassword": "qwer1234", "alias": "test_user",
      "enable_web": True, "enable_api": True}, **_KE3_OK)
_ke3("add_user", {"username": "test_user", "password": "x", "alias": "t", "web": True, "api": False},
     "POST", "/api/systemctrl/users/add",
     {"id": "test_user", "password": "x", "confirmNewPassword": "x", "alias": "t", "enable_web": True,
      "enable_api": False}, file="add_user-error", **_KE3_ERR)
_ke3("remove_user", {"username": "Jack"}, "POST", "/api/systemctrl/users/remove", {"ids": "Jack"}, **_KE3_OK)
_ke3("modify_user", {"username": "test1", "alias": "test11", "web": True, "api": False}, "POST",
     "/api/systemctrl/users/modify", {"id": "test1", "alias": "test11", "enable_web": True, "enable_api": False})
_ke3("set_user_password", {"username": "test1", "password": "Admin001"}, "POST", "/api/systemctrl/users/modify",
     {"id": "test1", "password": "Admin001"})
_ke3("get_api_auth", {}, "GET", "/api/systemctrl/users/auth_info",
     http_reply={"status": 200, "body": '{"result":"ok","data":{"enable":false}}'},
     expect_result={"ok": {"kind": "value", "value": {"result": "ok", "data": {"enable": False}}}})
_ke3("set_api_auth", {"enabled": True}, "POST", "/api/systemctrl/users/auth_info", {"enable": True}, **_KE3_OK)

# ── Time (1.2) ───────────────────────────────────────────────────────────
_ke3("get_time", {}, "GET", "/api/systemctrl/systime/get")
_ke3("get_ntp_status", {}, "GET", "/api/systemctrl/systime/ntpstatus")
_ke3("set_timezone", {"timezone": "Asia/Shanghai"}, "POST", "/api/systemctrl/systime/setLocation",
     {"timezone": "Asia/Shanghai"})
_ke3("set_timezone_offset", {"timezone": "Etc/Unknown", "offset": 8}, "POST", "/api/systemctrl/systime/setLocation",
     {"timezone": "Etc/Unknown", "offset": 8})
_ke3("set_time", {"time": "2024-10-11 10:29:30"}, "POST", "/api/systemctrl/systime/setTime",
     {"timetype": "manual", "time": "2024-10-11 10:29:30"})
_ke3("set_time", {"mode": "pc", "time": "2024-10-11 10:29:30"}, "POST", "/api/systemctrl/systime/setTime",
     {"timetype": "pc", "time": "2024-10-11 10:29:30"}, file="set_time-pc")
_ke3("set_time_ntp", {"servers": "ntp1.aliyun.com ntp2.aliyun.com"}, "POST", "/api/systemctrl/systime/setTime",
     {"timetype": "ntp", "ntp": "ntp1.aliyun.com ntp2.aliyun.com"})

# ── System (1.3) ─────────────────────────────────────────────────────────
_ke3("get_components", {}, "GET", "/api/systemctrl/system/components")
_ke3("get_hostname", {}, "GET", "/api/systemctrl/system/getHostname",
     http_reply={"status": 200, "body": '{"result":"ok","msg":null,"data":{"hostname":"E3-201412000F1DB"}}'},
     expect_result={"ok": {"kind": "value", "value": {"result": "ok", "msg": None,
                                                      "data": {"hostname": "E3-201412000F1DB"}}}})
_ke3("set_hostname", {"name": "E3 Stage Left"}, "GET",
     "/api/systemctrl/system/setHostname?name=" + _ke3_q("E3 Stage Left"), **_KE3_OK)
_ke3("get_web_ports", {}, "GET", "/api/systemctrl/system/getProtocol")
_ke3("set_web_ports", {"http": True, "https": True}, "POST", "/api/systemctrl/system/setProtocol",
     {"http": True, "https": True, "httpPort": 80, "httpsPort": 443})
_ke3("get_ssl", {}, "GET", "/api/systemctrl/system/ssl")
_ke3_upload("upload_ssl_file", {"file_type": "crt", "file": "LS0tLS1CRUdJTg==", "filename": "server.crt"},
            "/api/systemctrl/system/ssl/crt", [("file", b"-----BEGIN", "server.crt")])
_ke3("apply_ssl", {}, "POST", "/api/systemctrl/system/ssl", **_KE3_OK)
_ke3("reset_ssl", {}, "POST", "/api/systemctrl/system/resetSsl")
_ke3("factory_reset", {}, "GET", "/api/systemctrl/system/reFactory", **_KE3_OK)
_ke3("restart_codec", {}, "GET", "/api/systemctrl/system/restore")
_ke3("reboot", {}, "GET", "/api/systemctrl/system/reboot", **_KE3_OK)
_ke3("ping_host", {"target": "192.168.35.22"}, "POST", "/api/systemctrl/system/ping",
     {"target_addr": "192.168.35.22"},
     http_reply={"status": 200, "body": '{"result":"ok","data":{"loss_rate":100,"reachable":false},"msg":null}'},
     expect_result={"ok": {"kind": "value", "value": {"loss_rate": 100, "reachable": False}}})
_ke3("get_network_speed", {}, "GET", "/api/systemctrl/system/getNetworkSpeed")
_ke3("get_system_info", {}, "GET",
     "/api/systemctrl/system/getSystemInfo?version=true&cpu=true&memory=true&disk=true&persisTime=true")
_ke3_upload("upload_firmware", {"file": "AQIDBA=="}, "/api/systemctrl/system/upload",
            [("file", b"\x01\x02\x03\x04", "firmware.bin")])
_ke3_upload("upload_firmware_chunk", {"identifier": "fw-1-21", "number": 0, "file": "AQIDBA=="},
            "/api/systemctrl/system/uploadfile",
            [("identifier", b"fw-1-21", None), ("number", b"0", None), ("file_type", b"bin", None),
             ("file", b"\x01\x02\x03\x04", "fw-1-21.0")])
_ke3("merge_firmware", {"identifier": "fw-1-21", "count": 3}, "POST", "/api/systemctrl/system/mergefile",
     {"identifier": "fw-1-21", "number": "3", "file_type": "bin"})
_ke3("get_production_service", {}, "GET", "/api/systemctrl/system/production_stat")
_ke3("set_production_service", {"enabled": False}, "POST", "/api/systemctrl/system/production_off", {"value": False})

# ── Presets (1.4) ────────────────────────────────────────────────────────
_ke3("get_preset_modules", {}, "GET", "/api/systemctrl/system_preset/modules")
_ke3("list_presets", {}, "GET", "/api/systemctrl/system_preset/get")
_ke3_upload("import_preset", {"file": "UEsDBA=="}, "/api/systemctrl/system_preset/import",
            [("file", b"PK\x03\x04", "preset.zip")])
_ke3("save_preset", {"name": "Show A", "modules": ["codec_stream", "osd"]}, "POST", "/api/systemctrl/system_preset/save",
     {"name": "Show A", "modules": ["codec_stream", "osd"]}, **_KE3_OK)
_ke3("load_preset", {"pid": "df5bbf9a-3c14-4444-a477-ab77e407cb7f"}, "POST",
     "/api/systemctrl/system_preset/df5bbf9a-3c14-4444-a477-ab77e407cb7f/load", **_KE3_OK)
_ke3("delete_preset", {"pid": "b68bbc3b-a35d-45a7-b552-6b3c264b0797"}, "DELETE",
     "/api/systemctrl/system_preset/b68bbc3b-a35d-45a7-b552-6b3c264b0797")

# ── Logs and maintenance (1.5, 1.6) ──────────────────────────────────────
_ke3("get_log", {"log": "codec"}, "GET", "/api/systemctrl/system_log/get/codec",
     http_reply={"status": 200, "body": '{"result":"ok","data":"[2024-07-18 17:24:23,437] INFO: <codec> started"}'},
     expect_result={"ok": {"kind": "value", "value": "[2024-07-18 17:24:23,437] INFO: <codec> started"}})
_ke3("get_maintenance", {}, "GET", "/api/systemctrl/system_maintenance/get")
_ke3("set_maintenance", {"enabled": True, "policy": "week", "start_date": "2026-11-01 03:00:00", "weeks": [0, 3]},
     "POST", "/api/systemctrl/system_maintenance/set",
     {"enable": True, "policy": "week", "start_date": "2026-11-01 03:00:00", "months": [], "weeks": [0, 3],
      "days": [], "day": 1})

# ── Recording (2) ────────────────────────────────────────────────────────
_REC_PATH = "/run/media/sda1/RECORD/HDMI20231106_171002_497891/REC-HDMI-202311061710024978911.mp4"
_ke3("delete_recording_file", {"path": _REC_PATH}, "POST", "/api/record/record/remove_file", {"path": _REC_PATH})
_ke3("list_recording_files", {"device": "/run/media/sda1"}, "GET",
     "/api/record/record/get_disk_files?device=" + _ke3_q("/run/media/sda1"))
_ke3("list_recording_files_of", {"device": "/run/media/sda1", "stream": "hdmi"}, "GET",
     "/api/record/record/get_disk_files?device=" + _ke3_q("/run/media/sda1") + "&stream=hdmi")
_ke3("select_usb_storage", {"id": "sda1", "selected": True}, "POST", "/api/record/record/choose_usb",
     {"id": "sda1", "choose": True})
_ke3("select_nas_storage", {"id": "123", "selected": False}, "POST", "/api/record/record/choose_nas",
     {"id": "123", "choose": False})
_ke3("get_storage", {}, "GET", "/api/record/record/disk_list")
_ke3("get_record_settings", {}, "GET", "/api/record/record/get_record_info")
_ke3("set_record_settings", {"format": "mp4", "file_prefix": "REC", "limit_type": "time-loop", "limit_time": 14400,
                             "disk_policy": "noSpace"},
     "POST", "/api/record/record/settings",
     {"format": "mp4", "file_prefix": "REC", "limit_type": "time-loop", "limit_size": 512, "limit_time": 14400,
      "disk_policy": "noSpace", "follow_timecode_start": False})
_ke3("start_recording", {"interface": "hdmi"}, "POST", "/api/record/hdmi/recording", {"start": True}, **_KE3_OK)
_ke3("stop_recording", {"interface": "sdi"}, "POST", "/api/record/sdi/recording", {"start": False})
_ke3("get_recording_status", {"interface": "pip"}, "GET", "/api/record/pip/recording",
     http_reply={"status": 200, "body": '{"result":"ok","data":{"status":false}}'},
     expect_result={"ok": {"kind": "value", "value": {"result": "ok", "data": {"status": False}}}})
_ke3("add_nas", {"id": "123", "name": "123", "type": "cifs", "host": "192.168.43.1", "volume": "abc",
                 "options": "user=,password="},
     "POST", "/api/record/nas/add",
     {"id": "123", "name": "123", "type": "cifs", "host": "192.168.43.1", "volume": "abc",
      "options": "user=,password="})
_ke3("remove_nas", {"id": "123"}, "POST", "/api/record/nas/remove", {"id": "123"})

# ── Streams (3.1) ────────────────────────────────────────────────────────
_ke3("list_streams", {"interface": "hdmi"}, "GET", "/api/streamer/hdmi/stream/list")
_ke3("add_rtsp_stream", {"interface": "hdmi", "name": "test", "session": "ch01", "auth": True, "bind_audio": 1},
     "POST", "/api/streamer/hdmi/stream",
     {"type": "rtsp", "name": "test", "port": 554, "httpTunnelPort": 8554, "session": "ch01", "auth": True,
      "multicast_enable": False, "multicast_addr": "224.0.0.1", "multicast_ttl": 127, "multicast_port_min": 31000,
      "multicast_port_max": 31004, "bindAudio": 1, "bindVideo": "main"}, **_KE3_OK)
_ke3("add_rtmp_stream", {"interface": "sdi", "name": "YouTube", "address": "rtmp://a.rtmp.youtube.com/live2/abcd-efgh",
                         "bind_audio": 1},
     "POST", "/api/streamer/sdi/stream",
     {"type": "rtmp", "name": "YouTube", "address": "rtmp://a.rtmp.youtube.com/live2/abcd-efgh", "bindAudio": 1,
      "bindVideo": "main", "connIntv": 3, "connTimeout": 15, "user": "", "password": "", "oldRtmp": False})
_ke3("add_srt_stream", {"interface": "hdmi", "name": "SRT out", "mode": "caller", "address": "srt://192.168.40.2",
                        "port": 5700, "latency": 500, "bind_audio": 1, "key_length": "16", "passphrase": "0123456789ab"},
     "POST", "/api/streamer/hdmi/stream",
     {"type": "srt", "name": "SRT out", "mode": "caller", "address": "srt://192.168.40.2", "port": 5700, "latency": 500,
      "bindAudio": 1, "bindVideo": "main", "streamid": "", "maxbw": -1, "pbkeylen": 16, "passphrase": "0123456789ab"})
_ke3("add_hls_stream", {"interface": "pip", "name": "HLS", "session": "ch02", "bind_audio": 2, "bind_video": "sub"},
     "POST", "/api/streamer/pip/stream",
     {"type": "hls", "name": "HLS", "session": "ch02", "segmentTime": 5, "bindAudio": 2, "bindVideo": "sub"})
_ke3("add_ts_udp_stream", {"interface": "hdmi", "name": "TS", "address": "239.1.1.1", "port": 5700, "bind_audio": 1,
                           "null_packets": "1.5"},
     "POST", "/api/streamer/hdmi/stream",
     {"type": "ts", "name": "TS", "address": "239.1.1.1", "port": 5700, "ttl": 127}, )
# The null packet multiple is a number on the wire: written out by hand.
V[-1]["expect_request"]["body"] = (
    '{"type":"ts","name":"TS","address":"239.1.1.1","port":5700,"ttl":127,"ts_null_multiple":1.5,"bindPort":0,'
    '"bindAudio":1,"bindVideo":"main","ts_service_name":"Encoder","ts_service_provider":"Encoder device",'
    '"ts_transport_stream_id":101,"ts_tables_version":6,"ts_pcr_period":20,"ts_start_pid":480,"ts_pmt_start_pid":481}')
_ke3("add_rtp_stream", {"interface": "uvc", "name": "RTP", "address": "192.168.40.2", "port": 5700, "bind_audio": 1},
     "POST", "/api/streamer/uvc/stream",
     {"type": "rtp", "name": "RTP", "address": "192.168.40.2", "port": 5700, "load_type": "ts", "bindAudio": 1,
      "bindVideo": "main", "ts_service_name": "Encoder", "ts_service_provider": "Encoder device"})
_ke3("add_stream", {"interface": "hdmi", "stream": {"type": "rtmp", "name": "RTMPS", "address": "rtmps://live.example/app/key",
                                                    "bindAudio": 1, "connIntv": 3, "connTimeout": 15, "user": "",
                                                    "password": ""}},
     "POST", "/api/streamer/hdmi/stream",
     {"type": "rtmp", "name": "RTMPS", "address": "rtmps://live.example/app/key", "bindAudio": 1, "connIntv": 3,
      "connTimeout": 15, "user": "", "password": ""})
_ke3("update_ndi_stream", {"interface": "hdmi", "stream_id": "a08290a345356a77", "name": "NDI-HX",
                           "channel_name": "channel-hdmi", "bind_audio": 1},
     "PUT", "/api/streamer/hdmi/stream/a08290a345356a77",
     {"id": "a08290a345356a77", "type": "ndi_hx", "name": "NDI-HX", "channel_name": "channel-hdmi", "group": "",
      "connection": "disable_rudp", "netprefix": "239.255.0.0", "netmask": "255.255.0.0", "ttl": 1,
      "discovery_server": "", "bindAudio": 1, "bindVideo": "main"}, **_KE3_OK)
_ke3("update_rtmp_stream", {"interface": "sdi", "stream_id": "38e19b8c6c15f7c1", "name": "YouTube",
                            "address": "rtmp://a.rtmp.youtube.com/live2/new-key", "bind_audio": 1, "user": "u",
                            "password": "p"},
     "PUT", "/api/streamer/sdi/stream/38e19b8c6c15f7c1",
     {"id": "38e19b8c6c15f7c1", "type": "rtmp", "name": "YouTube", "address": "rtmp://a.rtmp.youtube.com/live2/new-key",
      "bindAudio": 1, "bindVideo": "main", "connIntv": 3, "connTimeout": 15, "user": "u", "password": "p",
      "oldRtmp": False})
_ke3("update_srt_stream", {"interface": "hdmi", "stream_id": "0cc29b8bc74cd729", "name": "SRT in", "mode": "listener",
                           "port": 9000, "latency": 120, "bind_audio": 1, "stream_id_srt": "cam1"},
     "PUT", "/api/streamer/hdmi/stream/0cc29b8bc74cd729",
     {"id": "0cc29b8bc74cd729", "type": "srt", "name": "SRT in", "mode": "listener", "address": "", "port": 9000,
      "latency": 120, "bindAudio": 1, "bindVideo": "main", "streamid": "cam1", "maxbw": -1, "pbkeylen": 0,
      "passphrase": ""})
_ke3("update_stream", {"interface": "hdmi", "stream_id": "d8df2ef3e0b3ca86",
                       "stream": {"id": "d8df2ef3e0b3ca86", "type": "hls", "name": "HLS", "session": "ch03",
                                  "segmentTime": 2, "bindAudio": 1}},
     "PUT", "/api/streamer/hdmi/stream/d8df2ef3e0b3ca86",
     {"id": "d8df2ef3e0b3ca86", "type": "hls", "name": "HLS", "session": "ch03", "segmentTime": 2, "bindAudio": 1})
_ke3("delete_stream", {"interface": "hdmi", "stream_id": "a08290a345356a77"}, "DELETE",
     "/api/streamer/hdmi/stream/a08290a345356a77", **_KE3_OK)
_ke3("start_stream", {"interface": "hdmi", "stream_id": "a08290a345356a77"}, "POST", "/api/streamer/hdmi/stream/enable",
     {"id": "a08290a345356a77", "enable": True}, **_KE3_OK)
_ke3("start_stream", {"interface": "hdmi", "stream_id": "a08290a345356a77"}, "POST", "/api/streamer/hdmi/stream/enable",
     {"id": "a08290a345356a77", "enable": True}, file="start_stream-error", **_KE3_ERR)
_ke3("stop_stream", {"interface": "sdi", "stream_id": "38e19b8c6c15f7c1"}, "POST", "/api/streamer/sdi/stream/enable",
     {"id": "38e19b8c6c15f7c1", "enable": False})
_ke3("reset_ndi", {"interface": "hdmi"}, "POST", "/api/streamer/hdmi/stream/ndi/reset")
_ke3("get_webrtc", {"interface": "hdmi"}, "GET", "/api/streamer/hdmi/stream/get_webrtc")
_ke3("set_webrtc", {"interface": "hdmi", "stream_id": "a08290a345356a77", "mode": "image"}, "GET",
     "/api/streamer/hdmi/stream/set_webrtc?stream_id=a08290a345356a77&mode=1")
_ke3("list_media_users", {"interface": "hdmi"}, "GET", "/api/streamer/hdmi/stream/media_user")
_ke3("add_media_user", {"interface": "hdmi", "username": "viewer", "password": "Admin123"}, "POST",
     "/api/streamer/hdmi/stream/media_user", {"username": "viewer", "password": "Admin123"})
_ke3("delete_media_user", {"interface": "hdmi", "username": "viewer"}, "DELETE",
     "/api/streamer/hdmi/stream/media_user/viewer")

# ── PTZ (3.2) ────────────────────────────────────────────────────────────
_ke3("ptz_move", {"direction": "up"}, "GET", "/api/streamer/ptz/control?action=up&speed=50", **_KE3_OK)
_ke3("ptz_move", {"direction": "right-down", "speed": 80}, "GET", "/api/streamer/ptz/control?action=right-down&speed=80",
     file="ptz_move-diagonal")
_ke3("ptz_action", {"action": "stop-all"}, "GET", "/api/streamer/ptz/control?action=stop-all")
_ke3("ptz_preset", {"action": "call-preset", "preset": 2}, "GET", "/api/streamer/ptz/control?action=call-preset&id=2")
_ke3("ptz_custom", {"action": "zoom-in", "speed": 30}, "GET", "/api/streamer/ptz/control?action=zoom-in&speed=30")
_ke3("get_ptz", {}, "GET", "/api/streamer/ptz/info")
_ke3("get_ptz_protocols", {}, "GET", "/api/streamer/ptz/protocol")
_ke3("get_ptz_serial_devices", {}, "GET", "/api/streamer/ptz/serial_device")
_ke3("set_ptz", {"enabled": True, "connection": "network", "address": "192.168.38.18", "port": 50381}, "POST",
     "/api/streamer/ptz/modify",
     {"enable": 1, "typ": "network", "device": "none", "baudrate": 9600, "parity": "none", "startBits": 8,
      "endBits": 1, "xonxoff": 0, "rtscts": 0, "ptz_protocol": "Sony Visca", "ptz_addr": 1, "protocol": "UDP",
      "addr": "192.168.38.18", "port": 50381}, **_KE3_OK)

# ── Front panel (3.3, 3.4) ───────────────────────────────────────────────
_ke3("get_quick_stream", {}, "GET", "/api/streamer/panel_stream/shortcuts_stream")
_ke3("set_quick_stream", {"enabled": True}, "POST", "/api/streamer/panel_stream/shortcuts_stream", {"enable": True})
_ke3("set_quick_stream_flag", {"enabled": False}, "POST", "/api/streamer/panel_stream/set_shortcuts_enable",
     {"enable": False})
_ke3("get_tally_source", {}, "GET", "/api/streamer/panel_stream/get_tally_setting",
     http_reply={"status": 200, "body": '"sdi"'}, expect_result={"ok": {"kind": "value", "value": "sdi"}})
_ke3("set_tally_source", {"source": "off"}, "POST", "/api/streamer/panel_stream/set_tally_setting", {"param": "off"})
_ke3("get_quick_stream_config", {}, "GET", "/api/streamer/shortcuts/list")
_ke3("set_quick_stream_config", {"record_sources": ["hdmi", "sdi", "pip"],
                                 "streams": {"hdmi": ["d8df2ef3e0b3ca86"], "sdi": ["38e19b8c6c15f7c1", "0cc29b8bc74cd729"],
                                             "pip": ["60c47e02dd3246a2"]}},
     "POST", "/api/streamer/shortcuts/set",
     {"panel_enable": True, "shortcuts_record": ["hdmi", "sdi", "pip"],
      "shortcuts_stream": {"hdmi": ["d8df2ef3e0b3ca86"], "sdi": ["38e19b8c6c15f7c1", "0cc29b8bc74cd729"],
                           "pip": ["60c47e02dd3246a2"]}})

# ── Network (4) ──────────────────────────────────────────────────────────
_ke3("get_network", {}, "GET", "/api/networkmanager/network/GetLinkinfo")
_ke3("get_ethernet", {}, "GET", "/api/networkmanager/network/GetEthernets?iframe=eth0")
_ke3("set_network", {"method": "static", "address": "192.168.35.35", "netmask": "255.255.255.0",
                     "gateway": "192.168.35.254", "mac": "44:07:6A:5F:D4:F7", "dns": "223.5.5.5;224.5.5.5"},
     "POST", "/api/networkmanager/network/ChangeLinkinfo",
     {"ifname": "eth0", "cfg": {"address": "192.168.35.35", "netmask": "255.255.255.0", "gw": "192.168.35.254",
                                "mac": "44:07:6A:5F:D4:F7", "method": "static", "enable": True,
                                "dns": "223.5.5.5;224.5.5.5"}}, **_KE3_OK)
_ke3("set_ethernet", {"method": "dhcp", "address": "192.168.38.217", "netmask": "255.255.255.0",
                      "mac": "56:3D:69:9B:5A:C7"},
     "POST", "/api/networkmanager/network/SetEthernets",
     {"ifname": "eth0", "address": "192.168.38.217", "netmask": "255.255.255.0", "gw": "", "mac": "56:3D:69:9B:5A:C7",
      "method": "dhcp", "dns": ""})
_ke3("set_interface_enabled", {"enabled": True}, "POST", "/api/networkmanager/network/SetSwitch",
     {"ifname": "eth0", "enable": True})
_ke3("reset_mdns", {}, "GET", "/api/networkmanager/network/ResetMDNS")
_ke3("add_static_route", {"target": "192.168.38.12", "netmask": "255.255.255.0", "gateway": "192.168.38.254"},
     "POST", "/api/networkmanager/static/routing/add",
     {"interface": "eth0", "target": "192.168.38.12", "netmask": "255.255.255.0", "gateway": "192.168.38.254",
      "mtu": 1500, "metric": 0})
_ke3("get_static_routes", {}, "GET", "/api/networkmanager/static/routing/get")
_ke3("modify_static_route", {"id": 1, "target": "192.168.38.12", "netmask": "255.255.255.0",
                             "gateway": "192.168.38.254", "metric": 5},
     "POST", "/api/networkmanager/static/routing/modify",
     {"id": 1, "interface": "eth0", "target": "192.168.38.12", "netmask": "255.255.255.0", "gateway": "192.168.38.254",
      "mtu": 1500, "metric": 5})
_ke3("delete_static_route", {"id": 1}, "POST", "/api/networkmanager/static/routing/del", {"ids": 1})
_ke3("set_static_route_enabled", {"id": 1, "enabled": True}, "POST", "/api/networkmanager/static/routing/set",
     {"id": 1, "status": 1})

# ── KiloLink (5) ─────────────────────────────────────────────────────────
_ke3("get_kilolink_interfaces", {}, "GET", "/api/kilolink/networks")
_ke3("get_kilolink", {}, "GET", "/api/kilolink/get")
_ke3("set_kilolink", {"enabled": False, "server": "192.168.1.40", "auth_code": "ASdq41a"}, "POST", "/api/kilolink/set",
     {"cfg": {"enable": False, "key": "ASdq41a", "ip": "192.168.1.40", "port": 50000, "crypto": False,
              "ifname": ["eth0"]}},
     http_reply={"status": 200, "body": '{"result":"ok"}'}, expect_result={"ok": {"kind": "ack"}})

# ── Video input and EDID (6.1) ───────────────────────────────────────────
_ke3("get_edid", {}, "GET", "/api/codec/vin/edid")
_ke3_upload("upload_edid", {"file": "AP///////wA="}, "/api/codec/vin/edid/upload",
            [("file", b"\x00\xff\xff\xff\xff\xff\xff\x00", "edid.bin")])
_ke3("set_edid_priority", {"priority": "1080p"}, "POST", "/api/codec/vin/edid/priority", {"edid_priority": "1"})
_ke3("set_edid_priority", {"priority": "none"}, "POST", "/api/codec/vin/edid/priority", {"edid_priority": ""},
     file="set_edid_priority-none")
_ke3("reset_edid", {}, "GET", "/api/codec/vin/edid/reset")
_ke3("get_active_channels", {}, "GET", "/api/codec/vin/active_interface")
_ke3("get_input", {"interface": "sdi"}, "GET", "/api/codec/sdi/vin/detail")
_ke3("get_uvc_formats", {}, "GET", "/api/codec/uvc/vin/list_format")
_ke3("set_uvc_input", {"encoding": "mjpeg", "resolution": "1920x1080", "fps": 30}, "POST", "/api/codec/uvc/vin/source",
     {"input_encode": "mjpeg", "input_resolution": "1920x1080", "input_fps": 30})
_ke3("set_pip_input", {"width": 1920, "height": 1080, "fps": "25"}, "POST", "/api/codec/pip/vin/source",
     {"width": 1920, "height": 1080, "input_fps": 25})

# ── Audio (6.2) ──────────────────────────────────────────────────────────
_ke3("get_audio_input", {"interface": "hdmi"}, "GET", "/api/codec/hdmi/audio/input")
_ke3("set_audio_input", {"interface": "sdi", "source": "linein", "gain": -6}, "POST", "/api/codec/sdi/audio/input/source",
     {"source": "linein", "gain": -6, "mute": False})
_ke3("list_audio_encoders", {"interface": "hdmi"}, "POST", "/api/codec/hdmi/audio/encode/list")
_ke3("add_audio_encoder", {"interface": "hdmi", "name": "666", "codec": "AAC"}, "POST", "/api/codec/hdmi/audio/encode",
     {"name": "666", "codec": "AAC", "sampling": 48000, "bitrate": 64000, "channels": 2, "resample": "fastest",
      "g711Law": "PCMU", "track_num": 1})
_ke3("update_audio_encoder", {"interface": "hdmi", "audio_id": 1, "name": "123", "codec": "G711", "sampling": 8000,
                              "channels": 1, "g711_law": "PCMA"},
     "PUT", "/api/codec/hdmi/audio/encode/1",
     {"id": 1, "name": "123", "codec": "G711", "resample": "fastest", "sampling": 8000, "channels": 1,
      "bitrate": 64000, "g711Law": "PCMA", "track_num": 1})
_ke3("delete_audio_encoder", {"interface": "hdmi", "audio_id": 2}, "POST", "/api/codec/hdmi/audio/encode/2")
_ke3("list_audio_sources", {"interface": "sdi"}, "GET", "/api/codec/sdi/audio/input/list_source")

# ── Video encoding and PIP (6.3) ─────────────────────────────────────────
_ke3("get_video_encoding", {"interface": "hdmi"}, "GET", "/api/codec/hdmi/venc/settings?venc_type=main")
_ke3("get_video_encoding", {"interface": "hdmi", "venc_type": "sub"}, "GET", "/api/codec/hdmi/venc/settings?venc_type=sub",
     file="get_video_encoding-sub")
_VENC = {"grey": False, "profile": 2, "picWidth": 1, "picHeight": 1, "mode": "CBR", "gop": 60}
_VENC_TAIL = {"maxBitrate": 6000, "minQp": 24, "maxQp": 51, "minIQp": 24, "minIp": 1, "maxIp": 20, "minQfactor": 80,
              "maxQfactor": 85, "deinterlaceMode": "weave", "timecode_type": 0}
_ke3("set_video_encoding", {"interface": "hdmi", "codec": "H265", "fps": 60, "bitrate": 3000}, "POST",
     "/api/codec/hdmi/venc/settings",
     {"venc_type": "main", "codec": "H265", **_VENC, "fps": 60, "bitrate": 3000, **_VENC_TAIL}, **_KE3_OK)
_ke3("set_video_encoding_auto_fps", {"interface": "sdi", "venc_type": "sub", "codec": "H264", "fps_mode": "half",
                                     "bitrate": 1000, "width": 640, "height": 360},
     "POST", "/api/codec/sdi/venc/settings",
     {"venc_type": "sub", "codec": "H264", "grey": False, "profile": 2, "picWidth": 640, "picHeight": 360,
      "mode": "CBR", "gop": 60, "fps": "half", "bitrate": 1000, **_VENC_TAIL})
_ke3("set_video_bitrate", {"interface": "hdmi", "bitrate": 102400}, "POST", "/api/codec/hdmi/venc/bitrate",
     {"venc_type": "main", "bitrate": 102400}, **_KE3_OK)
_ke3("set_encoding_enabled", {"interface": "uvc", "enabled": False}, "POST", "/api/codec/uvc/venc/enable",
     {"enable": False}, http_reply={"status": 200, "body": '{"result":"ok","msg":""}'},
     expect_result={"ok": {"kind": "ack"}})
_ke3("swap_pip", {}, "POST", "/api/codec/pip/venc/pip_back_update")
_ke3("get_pip_layout", {}, "GET", "/api/codec/pip/venc/pip_layout_setting")
_PIP_IN = {"hdmi_x": 0, "hdmi_y": 0, "hdmi_width": 100, "hdmi_height": 100, "hdmi_layer": 1,
           "sdi_x": 65, "sdi_y": 64, "sdi_width": 33.33, "sdi_height": 33.33, "sdi_layer": 2}
_PIP_BODY = ('[{"name":"hdmi","x":0.00,"y":0.00,"width":100.00,"height":100.00,"zIndex":1,"enable":true},'
             '{"name":"sdi","x":65.00,"y":64.00,"width":33.33,"height":33.33,"zIndex":2,"enable":true}]')
_ke3("set_pip_layout", _PIP_IN, "POST", "/api/codec/pip/venc/pip_layout_setting", _PIP_BODY)
_ke3("get_pip_input", {}, "GET", "/api/codec/pip/venc/pip_fps_setting")
_ke3("set_pip_input_format", {"fps": "50", "width": 1920, "height": 1080}, "POST", "/api/codec/pip/venc/pip_fps_setting",
     {"fps": 50, "width": 1920, "height": 1080})
_ke3("get_ndi_compliance", {"interface": "hdmi"}, "GET", "/api/codec/hdmi/venc/complie_NDIstandard",
     http_reply={"status": 200, "body": '{"is_standard": True, "ndi_hx_is_streaming": False}'},
     expect_result={"ok": {"kind": "value", "value": '{"is_standard": True, "ndi_hx_is_streaming": False}'}})
_ke3("restore_ndi_compliance", {"interface": "hdmi"}, "POST", "/api/codec/hdmi/venc/complie_NDIstandard")

# ── Video processing (6.4) ───────────────────────────────────────────────
_ke3("list_images", {}, "GET", "/api/codec/vpss/image")
_ke3_upload("upload_image", {"name": "456", "file": "iVBORw==", "filename": "logo.png"}, "/api/codec/vpss/image",
            [("name", b"456", None), ("file", b"\x89PNG", "logo.png")])
_ke3("delete_image", {"image_id": "3f33d7027c5111eea32f180dab8597db"}, "DELETE",
     "/api/codec/vpss/image/3f33d7027c5111eea32f180dab8597db")
_ke3("get_crop", {"interface": "hdmi"}, "GET", "/api/codec/hdmi/vpss/input/crop")
_ke3("set_crop", {"interface": "sdi", "enabled": True, "x": 25, "y": 25, "width": 50, "height": 50}, "POST",
     "/api/codec/sdi/vpss/input/crop", {"crop_en": True, "crop_x": 25, "crop_y": 25, "crop_w": 50, "crop_h": 50})
_ke3("apply_crop_to_layout", {"interface": "pip", **_PIP_IN}, "POST", "/api/codec/pip/vpss/input/crop_apply", _PIP_BODY)
_ke3("get_orientation", {"interface": "hdmi"}, "GET", "/api/codec/hdmi/vpss/input/rmf")
_ke3("set_orientation", {"interface": "uvc", "rotate": "90", "mirror": True}, "POST", "/api/codec/uvc/vpss/input/rmf",
     {"rotate": 90, "mirror_en": True, "flip_en": False})
_ke3("get_osd", {"interface": "hdmi"}, "GET", "/api/codec/hdmi/vpss/osd")
_ke3("set_osd_enabled", {"interface": "hdmi", "enabled": True}, "PUT", "/api/codec/hdmi/vpss/osd", {"enable": True})
_ke3("add_osd_item", {"interface": "hdmi", "type": "text", "text": "2222222222"}, "POST", "/api/codec/hdmi/vpss/osd",
     {"type": "text", "file_id": "", "text": "2222222222", "x": 0, "y": 0, "position": "LeftTop", "font_size": 1,
      "style": "normal", "outline": 0, "font_color": 1, "outline_color": 0, "enable": True}, **_KE3_OK)
_ke3("add_osd_item", {"interface": "hdmi", "type": "image", "file_id": "66bd63907c5311eea32f180dab8597db"}, "POST",
     "/api/codec/hdmi/vpss/osd",
     {"type": "image", "file_id": "66bd63907c5311eea32f180dab8597db", "text": "", "x": 0, "y": 0, "position": "LeftTop",
      "font_size": 1, "style": "normal", "outline": 0, "font_color": 1, "outline_color": 0, "enable": True},
     file="add_osd_item-image")
_ke3("get_osd_item", {"interface": "hdmi", "osd_id": "897d2f727c6d11eea32f180dab8597db"}, "GET",
     "/api/codec/hdmi/vpss/osd/897d2f727c6d11eea32f180dab8597db")
_ke3("update_osd_item", {"interface": "hdmi", "osd_id": "897d2f727c6d11eea32f180dab8597db", "type": "text",
                         "text": "12313666", "file_id": "3f33d7027c5111eea32f180dab8597db", "position": "Manual",
                         "x": 200, "y": 200, "style": "bold", "outline": "thick", "font_color": "blue",
                         "outline_color": "yellow"},
     "PUT", "/api/codec/hdmi/vpss/osd/897d2f727c6d11eea32f180dab8597db",
     {"id": "897d2f727c6d11eea32f180dab8597db", "type": "text", "file_id": "3f33d7027c5111eea32f180dab8597db",
      "text": "12313666", "x": 200, "y": 200, "position": "Manual", "font_size": 1, "style": "bold", "outline": 2,
      "font_color": 3, "outline_color": 5, "enable": True})
_ke3("delete_osd_item", {"interface": "hdmi", "osd_id": "897d2f727c6d11eea32f180dab8597db"}, "DELETE",
     "/api/codec/hdmi/vpss/osd/897d2f727c6d11eea32f180dab8597db")
_ke3("list_static_frames", {}, "GET", "/api/codec/vpss/static_frame")
_ke3_upload("upload_static_frame", {"file": "iVBORw=="}, "/api/codec/vpss/static_frame",
            [("file", b"\x89PNG", "static_frame.png")])
_ke3("reset_static_frame", {}, "POST", "/api/codec/vpss/static_frame/reset")

# ── Intercom (7) ─────────────────────────────────────────────────────────
_ke3("get_talk_version", {}, "GET", "/api/talk/talkVersion")
_ke3("set_talk_language", {"language": "en"}, "POST", "/api/talk/setLanguage", {"language": "en"})
_ke3("get_talk_serial", {}, "GET", "/api/talk/queryLocalDeviceSerial")
_ke3("list_talk_devices", {}, "GET", "/api/talk/queryDeviceList")
_ke3("add_talk_device", {"type": "device", "ip": "192.168.40.100", "serial": "7116110002301"}, "POST",
     "/api/talk/addDevice",
     {"auth_code": "", "type": "device", "ip": "192.168.40.100", "http_port": 80, "https_port": 443,
      "serial": "7116110002301"})
_ke3("delete_talk_device", {"ip": "192.168.43.176"}, "POST", "/api/talk/deleteDevice", {"device_ip": "192.168.43.176"})
_ke3("set_talk_device_link", {"local_ip": "192.168.38.74", "ip": "192.168.40.100", "enabled": True}, "POST",
     "/api/talk/setLDLinkDEnable", {"l_dev_ip": "192.168.38.74", "dev_ip": "192.168.40.100", "enable": True})
_ke3("set_talk_web_link", {"web_ip": "192.168.40.122", "ip": "192.168.43.176", "enabled": False}, "POST",
     "/api/talk/setWLinkDEnable", {"web_ip": "192.168.40.122", "dev_ip": "192.168.43.176", "enable": False})
_ke3("set_talk_server_link", {"local_ip": "192.168.35.11", "server_ip": "192.168.40.100", "enabled": True}, "POST",
     "/api/talk/setLDLinkSEnable", {"l_dev_ip": "192.168.35.11", "ser_ip": "192.168.40.100", "enable": True})
_ke3("get_talk_connection", {}, "GET", "/api/talk/queryLDeviceConnectStatus")

# ── ONVIF and NDI bridge (8, 9) ──────────────────────────────────────────
_ke3("get_onvif", {}, "GET", "/api/onvifmanage/onvif/get")
_ke3("set_onvif", {"enabled": True, "admin_name": "Admin", "device_name": "E3", "device_type": "E3"}, "POST",
     "/api/onvifmanage/onvif/set",
     {"control": {"enabled": True, "discover_port": 3702, "port": 8080}, "discoverable": True, "auth_req": False,
      "admin_name": "Admin", "admin_password": "", "location": "default", "device_name": "E3", "device_type": "E3"})
_ke3("get_ndi_bridge", {}, "GET", "/api/bridge/get")
_ke3("set_ndi_bridge", {"enabled": True, "address": "192.168.35.70", "source_name": "E3"}, "POST", "/api/bridge/set",
     {"enable": True, "recv": "Public", "address": "192.168.35.70", "port": "5990", "secure": "", "id": "E3"})
# With API authentication on: the token setting goes as the Authorization header.
_ke3("get_hostname", {}, "GET", "/api/systemctrl/system/getHostname", settings={"token": "eyJ0eXAiOi.abc"},
     file="get_hostname-token")


# ── Telemetry: the document's example replies ────────────────────────────
def _ke3t(name, path, body, state, **extra):
    inbound = {"path": path, "body": body if isinstance(body, str) else _ke3_j(body)}
    if "request" in extra:
        inbound["request"] = extra.pop("request")
    telemetry(KE3, name, inbound_http=inbound, expect_state=state, **extra)


_ke3t("hostname", "/api/systemctrl/system/getHostname",
      {"result": "ok", "msg": None, "data": {"hostname": "E3-201412000F1DB"}},
      {"device": {"name": "E3-201412000F1DB"}})
_ke3t("system-info", "/api/systemctrl/system/getSystemInfo?version=true&cpu=true&memory=true&disk=true&persisTime=true",
      {"result": "ok", "data": {
          "version": {"ndiVersion": "5.1.3", "hardwareVersion": "3.0", "softwareVersion": "1.21.0008",
                      "serialNumber": "201412000F1DB", "product": "E3", "oem": "kiloview"},
          "cpu": {"precent": 10.891}, "mem": {"precent": 27.3, "total": 1029500, "used": 280996},
          "disk": {"precent": 27, "free": 1368528, "used": 508904, "total": 1998672},
          "persisTime": {"uptime": 71664.16}, "temp": {"temp": 40}}},
      {"device": {"product": "E3", "oem": "kiloview", "serial": "201412000F1DB", "firmware": "1.21.0008",
                  "hardware": "3.0", "ndi_version": "5.1.3"},
       "system": {"cpu_percent": 10.891, "memory_percent": 27.3, "memory_total": 1029500, "memory_used": 280996,
                  "disk_percent": 27.0, "disk_total": 1998672, "disk_used": 508904, "disk_free": 1368528,
                  "uptime": 71664.16, "temperature": 40.0}})
_ke3t("network-speed", "/api/systemctrl/system/getNetworkSpeed",
      {"result": "ok", "data": {"eth0": {"receive": 71.45, "transmit": 7741.28, "speed": 1000}, "eth1": {}}},
      {"network": {"ports": {"eth0": {"receive": 71.45, "transmit": 7741.28, "link_speed": 1000}}}})
_ke3t("auth-info", "/api/systemctrl/users/auth_info", {"result": "ok", "data": {"enable": False}},
      {"system": {"api_auth": False}})
_ke3t("production-stat", "/api/systemctrl/system/production_stat", {"result": "ok", "data": {"value": True}},
      {"system": {"production_service": True}})
_ke3t("protocol", "/api/systemctrl/system/getProtocol",
      {"result": "ok", "msg": None, "data": {"http": True, "https": True, "httpPort": 80, "httpsPort": 443}},
      {"web": {"http": True, "https": True, "http_port": 80, "https_port": 443}})
_ke3t("ssl", "/api/systemctrl/system/ssl",
      {"result": "ok", "msg": None, "data": {"ssl_crt": "", "ssl_key": "", "enable_reset": False}},
      {"web": {"ssl_certificate": "", "ssl_key": "", "ssl_reset_available": False}})
_ke3t("users", "/api/systemctrl/users/list",
      {"result": "ok", "data": [{"id": "admin", "alias": "Admin", "web": True, "api": True,
                                 "create_time": "1970-01-01 00:00:00"}]},
      {"users": {"admin": {"alias": "Admin", "web": True, "api": True, "created": "1970-01-01 00:00:00"}}},
      state_before={"users": {"jack": {"alias": "Jack", "web": True, "api": False, "created": "2024-01-01 00:00:00"}}})
_ke3t("time", "/api/systemctrl/systime/get",
      {"result": "ok", "msg": None, "data": {"time": "2024-07-31 18:04:56", "timetype": "pc", "Timezone": "Asia/Shanghai",
                                             "offset": 8, "ntp": "ntp1.aliyun.com ntp2.aliyun.com ntp3.aliyun.com"}},
      {"time": {"current": "2024-07-31 18:04:56", "mode": "pc", "timezone": "Asia/Shanghai", "offset": 8,
                "ntp_servers": "ntp1.aliyun.com ntp2.aliyun.com ntp3.aliyun.com"}})
_ke3t("ntp-status", "/api/systemctrl/systime/ntpstatus", '["synchronised to NTP server"]',
      {"time": {"ntp_status": '["synchronised to NTP server"]'}})
_ke3t("preset-modules", "/api/systemctrl/system_preset/modules",
      {"result": "ok", "data": ["time", "user", "ptz", "edid", "maintenance", "codec_stream", "record", "talk", "osd",
                                "panel"]},
      {"presets": {"modules": '["time","user","ptz","edid","maintenance","codec_stream","record","talk","osd","panel"]'}})
_ke3t("presets", "/api/systemctrl/system_preset/get",
      {"result": "ok", "data": [{"pid": "df5bbf9a-3c14-4444-a477-ab77e407cb7f", "name": "888"},
                                {"pid": "b68bbc3b-a35d-45a7-b552-6b3c264b0797", "name": "777"}]},
      {"presets": {"list": {"df5bbf9a-3c14-4444-a477-ab77e407cb7f": {"name": "888"},
                            "b68bbc3b-a35d-45a7-b552-6b3c264b0797": {"name": "777"}}}},
      state_before={"presets": {"list": {"0000": {"name": "Deleted"}}}})
_ke3t("maintenance", "/api/systemctrl/system_maintenance/get",
      {"result": "ok", "data": {"enable": False, "policy": "day", "start_date": "1970-01-01 00:00:00", "months": [],
                                "weeks": [], "days": [], "day": 1}},
      {"maintenance": {"enabled": False, "policy": "day", "start_date": "1970-01-01 00:00:00", "months": "[]",
                       "weeks": "[]", "days": "[]", "day": 1}})
_ke3t("network", "/api/networkmanager/network/GetLinkinfo",
      {"result": "ok", "data": [{"status": "up", "address": "192.168.3.37", "netmask": "255.255.255.0",
                                 "gw": "192.168.3.1", "mac": "18:C3:F4:20:F1:DB", "method": "dhcp", "enable": True,
                                 "dns": "223.5.5.5;223.6.6.6", "device": "eth0"}]},
      {"network": {"ports": {"eth0": {"status": "up", "address": "192.168.3.37", "netmask": "255.255.255.0",
                                      "gateway": "192.168.3.1", "mac": "18:C3:F4:20:F1:DB", "method": "dhcp",
                                      "enabled": True, "dns": "223.5.5.5;223.6.6.6"}}}})
_ke3t("static-routes", "/api/networkmanager/static/routing/get",
      {"interface": "eth0", "target": "192.168.38.12", "netmask": "255.255.255.0", "gateway": "192.168.38.254",
       "mtu": 1500, "metric": 0},
      {"network": {"static_routes": '{"interface":"eth0","target":"192.168.38.12","netmask":"255.255.255.0",'
                                    '"gateway":"192.168.38.254","mtu":1500,"metric":0}'}})
_ke3t("kilolink-networks", "/api/kilolink/networks", {"result": "ok", "list": ["eth0"]},
      {"kilolink": {"interfaces": '["eth0"]'}})
_ke3t("kilolink", "/api/kilolink/get",
      {"result": "ok", "cfg": {"ip": "192.168.1.40", "port": 50000, "ifname": ["eth0"], "key": "ASdq41a",
                               "crypto": False, "enable": True}, "status": 1},
      {"kilolink": {"enabled": True, "server": "192.168.1.40", "port": 50000, "ports": '["eth0"]',
                    "encrypted": False, "status": 1}})
_ke3t("edid", "/api/codec/vin/edid", {"edid_priority": "2", "edid": "AP///////wAmhQMA", "reset": False},
      {"edid": {"priority": "2", "data": "AP///////wAmhQMA", "reset_available": False}})
_ke3t("active-channels", "/api/codec/vin/active_interface", {"hdmi": True, "sdi": False, "pip": False, "uvc": False},
      {"channels": {"hdmi": {"active": True}, "sdi": {"active": False}, "pip": {"active": False},
                    "uvc": {"active": False}}})
_ke3t("input", "/api/codec/hdmi/vin/detail",
      {"signal": "locked", "format": "1920x1080p 60Hz", "width": 1920, "height": 1080, "interlaced": False, "fps": 60,
       "type": "HDMI", "interface": "hdmi", "max_width": 3840, "max_height": 2160, "max_fps": 60},
      {"channels": {"hdmi": {"input": {"signal": "locked", "format": "1920x1080p 60Hz", "width": 1920, "height": 1080,
                                       "interlaced": False, "fps": 60.0, "type": "HDMI", "max_width": 3840,
                                       "max_height": 2160, "max_fps": 60.0}}}})
_ke3t("uvc-formats", "/api/codec/uvc/vin/list_format",
      '[{"label":"MJPEG","value":"mjpeg","children":[{"label":"1920X1080","value":"1920x1080",'
      '"children":[{"label":60,"value":60}]}]}]',
      {"channels": {"uvc": {"input": {"formats": '[{"label":"MJPEG","value":"mjpeg","children":[{"label":"1920X1080",'
                                                 '"value":"1920x1080","children":[{"label":60,"value":60}]}]}]'}}}})
_ke3t("recording", "/api/record/hdmi/recording", {"result": "ok", "data": {"status": True}},
      {"channels": {"hdmi": {"recording": True}}})
_STREAMS = [{"id": "a08290a345356a77", "type": "ndi_hx", "name": "NDI-HX", "enable": False, "status": "stop",
             "bindAudio": 1, "ttl": 1, "channel_name": "channel-hdmi", "group": "", "netmask": "255.255.0.0",
             "connection": "disable_rudp", "netprefix": "239.255.0.0", "discovery_server": "", "addressUrl": "offline"},
            {"id": "d8df2ef3e0b3ca86", "type": "srt", "name": "SRT out", "enable": True, "status": "running",
             "bindAudio": 1, "bindVideo": "main", "mode": "caller", "address": "srt://192.168.40.2", "port": 5700,
             "latency": 500, "streamid": "", "maxbw": -1, "pbkeylen": 16}]
_ke3t("streams", "/api/streamer/hdmi/stream/list", _STREAMS,
      {"channels": {"hdmi": {"streams": {
          "a08290a345356a77": {"type": "ndi_hx", "name": "NDI-HX", "enabled": False, "status": "stop", "bind_audio": 1,
                               "ttl": 1, "ndi_channel": "channel-hdmi", "ndi_group": "",
                               "ndi_multicast_netmask": "255.255.0.0", "ndi_connection": "disable_rudp",
                               "ndi_multicast_prefix": "239.255.0.0", "ndi_discovery_server": "",
                               "address_url": "offline"},
          "d8df2ef3e0b3ca86": {"type": "srt", "name": "SRT out", "enabled": True, "status": "running", "bind_audio": 1,
                               "bind_video": "main", "srt_mode": "caller", "address": "srt://192.168.40.2",
                               "port": 5700, "latency": 500, "stream_id": "", "max_bandwidth": -1,
                               "key_length": 16}}}}},
      # A stream deleted on the device leaves the list.
      state_before={"channels": {"hdmi": {"streams": {"0000000000000000": {"type": "rtmp", "name": "Old"}}}}})
_ke3t("streams-ts", "/api/streamer/sdi/stream/list",
      [{"id": "0cc29b8bc74cd729", "type": "ts", "name": "TS", "enable": False, "status": "stop", "bindAudio": 1,
        "address": "239.1.1.1", "port": 5700, "ttl": 127, "ts_null_multiple": 1.5, "bindPort": 0,
        "ts_service_name": "Encoder", "ts_service_provider": "Encoder device", "ts_transport_stream_id": 101,
        "ts_tables_version": 6, "ts_pcr_period": 20, "ts_start_pid": 480, "ts_pmt_start_pid": 481},
       {"id": "38e19b8c6c15f7c1", "type": "rtsp", "name": "RTSP", "enable": True, "status": "running", "bindAudio": 1,
        "port": 554, "httpTunnelPort": 8554, "session": "ch01", "auth": True, "multicast_enable": False,
        "multicast_addr": "224.0.0.1", "multicast_ttl": 127, "multicast_port_min": 31000, "multicast_port_max": 31004},
       {"id": "60c47e02dd3246a2", "type": "rtmp", "name": "YouTube", "enable": True, "status": "running",
        "bindAudio": 1, "address": "rtmp://a.rtmp.youtube.com/live2/key", "connIntv": 3, "connTimeout": 15,
        "user": "", "oldRtmp": False},
       {"id": "1111aaaa2222bbbb", "type": "hls", "name": "HLS", "enable": False, "status": "stop", "bindAudio": 2,
        "session": "ch02", "segmentTime": 5},
       {"id": "3333cccc4444dddd", "type": "rtp", "name": "RTP", "enable": False, "status": "stop", "bindAudio": 1,
        "address": "192.168.40.2", "port": 5800, "load_type": "ts"}],
      {"channels": {"sdi": {"streams": {
          "0cc29b8bc74cd729": {"type": "ts", "name": "TS", "enabled": False, "status": "stop", "bind_audio": 1,
                               "address": "239.1.1.1", "port": 5700, "ttl": 127, "null_packets": 1.5, "bind_port": 0,
                               "ts_service_name": "Encoder", "ts_service_provider": "Encoder device",
                               "ts_transport_stream_id": 101, "ts_tables_version": 6, "ts_pcr_period": 20,
                               "ts_start_pid": 480, "ts_pmt_start_pid": 481},
          "38e19b8c6c15f7c1": {"type": "rtsp", "name": "RTSP", "enabled": True, "status": "running", "bind_audio": 1,
                               "port": 554, "http_tunnel_port": 8554, "session": "ch01", "auth": True,
                               "multicast": False, "multicast_address": "224.0.0.1", "multicast_ttl": 127,
                               "multicast_port_min": 31000, "multicast_port_max": 31004},
          "60c47e02dd3246a2": {"type": "rtmp", "name": "YouTube", "enabled": True, "status": "running",
                               "bind_audio": 1, "address": "rtmp://a.rtmp.youtube.com/live2/key",
                               "reconnect_interval": 3, "connect_timeout": 15, "user": "", "old_rtmp": False},
          "1111aaaa2222bbbb": {"type": "hls", "name": "HLS", "enabled": False, "status": "stop", "bind_audio": 2,
                               "session": "ch02", "segment_time": 5},
          "3333cccc4444dddd": {"type": "rtp", "name": "RTP", "enabled": False, "status": "stop", "bind_audio": 1,
                               "address": "192.168.40.2", "port": 5800, "load_type": "ts"}}}}})
# An error answer is no list: the streams already known stay.
_ke3t("streams-error", "/api/streamer/uvc/stream/list", '{"result":"error","msg":"interface disabled"}', {},
      state_before={"channels": {"uvc": {"streams": {"1111aaaa2222bbbb": {"name": "Kept"}}}}})
V[-1]["expect_state"] = {"channels": {"uvc": {"streams": {"1111aaaa2222bbbb": {"name": "Kept"}}}}}
_ke3t("quick-list", "/api/streamer/shortcuts/list",
      {"panel_enable": True, "shortcuts_record": ["hdmi"],
       "hdmi": [{"id": "a08290a345356a77", "auto_enable": True}], "sdi": [{"id": "38e19b8c6c15f7c1", "auto_enable": False}],
       "pip": [{"id": "60c47e02dd3246a2", "auto_enable": False}], "uvc": [{"id": "1111aaaa2222bbbb", "auto_enable": True}]},
      {"panel": {"panel_streaming": True, "quick_record_sources": '["hdmi"]'},
       "channels": {"hdmi": {"streams": {"a08290a345356a77": {"quick_stream": True}}},
                    "sdi": {"streams": {"38e19b8c6c15f7c1": {"quick_stream": False}}},
                    "pip": {"streams": {"60c47e02dd3246a2": {"quick_stream": False}}},
                    "uvc": {"streams": {"1111aaaa2222bbbb": {"quick_stream": True}}}}})
_ke3t("media-users", "/api/streamer/hdmi/stream/media_user", {"result": "ok", "user_list": ["admin"]},
      {"channels": {"hdmi": {"media_users": '["admin"]'}}})
_ke3t("quick-stream", "/api/streamer/panel_stream/shortcuts_stream",
      {"shortcutsRecordList": [], "shortcutsRecord": False, "shortcutsStream": True, "shortcutsEnable": True},
      {"panel": {"quick_record_list": "[]", "quick_recording": False, "quick_streaming": True, "quick_enabled": True}})
_ke3t("tally-source", "/api/streamer/panel_stream/get_tally_setting", '"sdi"', {"panel": {"tally_source": "sdi"}})
_ke3t("ptz", "/api/streamer/ptz/info",
      {"result": "ok", "data": {"enable": 0, "typ": "serial",
                                "network": {"protocol": "TCP", "addr": "", "port": 1, "ptz_protocol": "Sony Visca",
                                            "ptz_addr": "1"},
                                "serial": {"device": "none", "baudrate": 9600, "parity": "none", "startBits": 8,
                                           "endBits": 1, "xonxoff": 0, "rtscts": 0, "ptz_protocol": "Sony Visca",
                                           "ptz_addr": "1"}}},
      {"ptz": {"enabled": False, "connection": "serial",
               "network": {"protocol": "TCP", "address": "", "port": 1, "ptz_protocol": "Sony Visca",
                           "camera_address": "1"},
               "serial": {"device": "none", "baud_rate": 9600, "parity": "none", "data_bits": 8, "stop_bits": 1,
                          "xonxoff": False, "rtscts": False, "ptz_protocol": "Sony Visca", "camera_address": "1"}}})
_ke3t("ptz-protocols", "/api/streamer/ptz/protocol",
      {"result": "ok", "msg": None, "data": ["Sony Visca", "Pelco-d", "Pelco-p"]},
      {"ptz": {"protocols": '["Sony Visca","Pelco-d","Pelco-p"]'}})
_ke3t("ptz-serial", "/api/streamer/ptz/serial_device", {"result": "ok", "data": [{"device": "ttyUSB0", "driver": "FTDI"}]},
      {"ptz": {"serial_devices": '[{"device":"ttyUSB0","driver":"FTDI"}]'}})
_ke3t("audio-input", "/api/codec/hdmi/audio/input",
      {"source": "hdmi", "sampling": 44100, "channels": 2, "aichannel": 0, "gain": -20},
      {"channels": {"hdmi": {"audio": {"source": "hdmi", "sample_rate": 44100, "source_channels": 2,
                                       "channel_pair": 0, "gain": -20}}}})
_ke3t("audio-sources", "/api/codec/hdmi/audio/input/list_source",
      [{"label": "HDMI", "value": "hdmi"}, {"label": "Line In", "value": "linein"}],
      {"channels": {"hdmi": {"audio": {"sources": '[{"label":"HDMI","value":"hdmi"},{"label":"Line In","value":"linein"}]'}}}})
_ke3t("audio-encoders", "/api/codec/hdmi/audio/encode/list",
      [{"name": "channel-1", "volume": 0, "resample": "fastest", "channels": 2, "sampleSize": 1024, "sampling": 48000,
        "codec": "AAC", "bitrate": 64000, "sampleWidth": 16, "g711Law": "PCMU", "id": 1},
       {"name": "channel-2", "volume": 0, "resample": "fastest", "channels": 1, "sampleSize": 1024, "sampling": 8000,
        "codec": "G711", "bitrate": 64000, "sampleWidth": 16, "g711Law": "PCMU", "id": 2}],
      {"channels": {"hdmi": {"audio": {"encoders": {
          "1": {"name": "channel-1", "volume": 0, "resample": "fastest", "channels": 2, "sample_size": 1024,
                "sample_rate": 48000, "codec": "AAC", "bitrate": 64000, "sample_width": 16, "g711_law": "PCMU"},
          "2": {"name": "channel-2", "volume": 0, "resample": "fastest", "channels": 1, "sample_size": 1024,
                "sample_rate": 8000, "codec": "G711", "bitrate": 64000, "sample_width": 16, "g711_law": "PCMU"}}}}}},
      state_before={"channels": {"hdmi": {"audio": {"encoders": {"3": {"name": "gone"}}}}}})
_ke3t("video-encoding", "/api/codec/hdmi/venc/settings?venc_type=main",
      {"codec": "H264", "enable": True, "grey": False, "profile": 0, "picHeight": 1, "picWidth": 1, "mode": "CBR",
       "gop": 60, "fps": 60, "real_fps": 60, "real_fps_float": 59.94, "bitrate": 20000, "maxBitrate": 6000,
       "minQp": 24, "maxQp": 51, "minIQp": 24, "minIp": 1, "maxIp": 20, "minQfactor": 80, "maxQfactor": 85,
       "deinterlaceMode": "weave", "srcHeight": 1080, "srcWidth": 1920, "realWidth": 1920, "realHeight": 1080,
       "timecode_type": 0},
      {"channels": {"hdmi": {"video": {"main": {
          "codec": "H264", "enabled": True, "grey": False, "profile": 0, "height": 1, "width": 1, "mode": "CBR",
          "gop": 60, "fps": "60", "real_fps": 60, "real_fps_exact": 59.94, "bitrate": 20000, "max_bitrate": 6000,
          "min_qp": 24, "max_qp": 51, "min_iqp": 24, "min_ip": 1, "max_ip": 20, "min_qfactor": 80, "max_qfactor": 85,
          "deinterlace": "weave", "source_height": 1080, "source_width": 1920, "encoded_width": 1920,
          "encoded_height": 1080, "timecode": 0}}}}})
_ke3t("video-encoding-sub-auto", "/api/codec/sdi/venc/settings?venc_type=sub",
      {"codec": "H264", "enable": True, "fps": "auto", "real_fps": 30, "real_fps_float": 29.97},
      {"channels": {"sdi": {"video": {"sub": {"codec": "H264", "enabled": True, "fps": "auto", "real_fps": 30,
                                              "real_fps_exact": 29.97}}}}})
_ke3t("ndi-compliance", "/api/codec/hdmi/venc/complie_NDIstandard",
      {"is_standard": True, "ndi_hx_is_streaming": False},
      {"channels": {"hdmi": {"ndi_sub_compliant": True, "ndi_streaming": False}}})
_ke3t("pip-layout", "/api/codec/pip/venc/pip_layout_setting",
      [{"height": 100, "width": 100, "name": "hdmi", "x": 0, "y": 0, "zIndex": 1, "enable": True},
       {"height": 33.33, "width": 33.33, "name": "sdi", "x": 65, "y": 64, "zIndex": 2, "enable": True}],
      {"pip": {"layout": {"hdmi": {"height": 100.0, "width": 100.0, "x": 0.0, "y": 0.0, "layer": 1, "shown": True},
                          "sdi": {"height": 33.33, "width": 33.33, "x": 65.0, "y": 64.0, "layer": 2, "shown": True}}}})
# The reply to a layout write is not a layout: the windows stay.
_ke3t("pip-layout-write-reply", "/api/codec/pip/venc/pip_layout_setting", '{"result":"ok","msg":""}',
      {"pip": {"layout": {"hdmi": {"layer": 1}}}}, state_before={"pip": {"layout": {"hdmi": {"layer": 1}}}},
      request=[{"name": "hdmi"}],
      expect_then_send=[{"method": "GET", "target": "/api/codec/pip/venc/pip_layout_setting"}])
_ke3t("pip-input", "/api/codec/pip/venc/pip_fps_setting", {"fps": 50, "width": 1920, "height": 1080, "vertical": False},
      {"pip": {"input": {"fps": 50, "width": 1920, "height": 1080, "vertical": False}}})
_ke3t("images", "/api/codec/vpss/image",
      [{"size_h": 276, "size_w": 342, "url": "/img/3f33d7027c5111eea32f180dab8597db.png", "name": "123",
        "path": "/data/storage/img/3f33d7027c5111eea32f180dab8597db.png", "id": "3f33d7027c5111eea32f180dab8597db"}],
      {"images": {"3f33d7027c5111eea32f180dab8597db": {"name": "123", "width": 342, "height": 276,
                                                       "url": "/img/3f33d7027c5111eea32f180dab8597db.png"}}})
_ke3t("crop", "/api/codec/hdmi/vpss/input/crop", {"crop_en": True, "crop_x": 25, "crop_y": 25, "crop_w": 50, "crop_h": 50},
      {"channels": {"hdmi": {"crop": {"enabled": True, "x": 25, "y": 25, "width": 50, "height": 50}}}})
_ke3t("orientation", "/api/codec/uvc/vpss/input/rmf", {"rotate": 90, "mirror_en": False, "flip_en": True},
      {"channels": {"uvc": {"rotation": 90, "mirror": False, "flip": True}}})
_OSD = {"enable": True, "items": [
    {"enable": True, "type": "image", "position": "LeftTop", "x": 0, "y": 0, "font_size": 1, "font_color": 1,
     "style": "normal", "outline": 0, "outline_color": 0, "text": "", "file_id": "66bd63907c5311eea32f180dab8597db",
     "row_spacing": 1, "id": "830a6f927c6d11eea32f180dab8597db", "name": "456",
     "pic_url": "/img/66bd63907c5311eea32f180dab8597db.png"},
    {"enable": True, "type": "text", "position": "Manual", "x": 200, "y": 200, "font_size": 1, "font_color": 3,
     "style": "bold", "outline": 2, "outline_color": 5, "text": "12313", "file_id": "", "row_spacing": 1,
     "id": "897d2f727c6d11eea32f180dab8597db"}]}
_ke3t("osd", "/api/codec/hdmi/vpss/osd", _OSD,
      {"channels": {"hdmi": {"osd": {"enabled": True, "items": {
          "830a6f927c6d11eea32f180dab8597db": {"enabled": True, "type": "image", "position": "LeftTop", "x": 0, "y": 0,
                                               "font_size": 1, "font_color": "white", "style": "normal",
                                               "outline": "none", "outline_color": "black", "text": "",
                                               "file_id": "66bd63907c5311eea32f180dab8597db", "name": "456",
                                               "image_url": "/img/66bd63907c5311eea32f180dab8597db.png"},
          "897d2f727c6d11eea32f180dab8597db": {"enabled": True, "type": "text", "position": "Manual", "x": 200,
                                               "y": 200, "font_size": 1, "font_color": "blue", "style": "bold",
                                               "outline": "thick", "outline_color": "yellow", "text": "12313",
                                               "file_id": ""}}}}}},
      state_before={"channels": {"hdmi": {"osd": {"items": {"deadbeef": {"text": "gone"}}}}}})
_ke3t("static-frames", "/api/codec/vpss/static_frame",
      [{"size_h": 1080, "size_w": 1920, "url": "/frame/static_frame.png", "path": "/data/storage/frame/static_frame.png",
        "yuv_path": "/data/storage/frame/static_frame.yuv", "id": "static_frame", "pixel_format": 0}],
      {"static_frames": {"static_frame": {"width": 1920, "height": 1080, "url": "/frame/static_frame.png"}}})
_ke3t("record-info", "/api/record/record/get_record_info",
      {"result": "ok", "data": {"disk_type": "largest", "format": "mp4", "file_prefix": "REC", "limit_type": "size-loop",
                                "limit_size": 512, "limit_time": 3600, "disk_policy": "overwrite",
                                "follow_timecode_start": False}},
      {"recording": {"format": "mp4", "file_prefix": "REC", "limit_type": "size-loop", "limit_size": 512,
                     "limit_time": 3600, "disk_policy": "overwrite", "follow_timecode": False}})
_ke3t("storage", "/api/record/record/disk_list",
      {"result": "ok", "data": {
          "nas": [{"id": "123", "name": "123", "type": "cifs", "host": "192.168.43.1", "volume": "abc",
                   "options": "user=,password=", "mounted": "/media/nas/123", "filesys": "", "status": "mounted",
                   "choose": False, "disk_type": "", "total": 283584, "used": 41314, "free": 242270, "percent": 14}],
          "usb": [{"id": "sda1", "name": "sda1", "type": "", "host": "", "volume": "/dev/sda1", "options": "",
                   "mounted": "/run/media/sda1", "filesys": "vfat", "status": "mounted", "choose": True,
                   "disk_type": "usb", "total": 29534, "used": 23948, "free": 5585, "percent": 81}]}},
      {"storage": {
          "nas": {"123": {"name": "123", "type": "cifs", "host": "192.168.43.1", "volume": "abc",
                          "mount_path": "/media/nas/123", "filesystem": "", "status": "mounted", "selected": False,
                          "total": 283584, "used": 41314, "free": 242270, "percent": 14}},
          "usb": {"sda1": {"name": "sda1", "type": "", "host": "", "volume": "/dev/sda1",
                           "mount_path": "/run/media/sda1", "filesystem": "vfat", "status": "mounted",
                           "selected": True, "total": 29534, "used": 23948, "free": 5585, "percent": 81}}}},
      # A USB disk unplugged leaves the list.
      state_before={"storage": {"usb": {"sdb1": {"name": "sdb1"}}}})
_ke3t("talk-version", "/api/talk/talkVersion", {"result": "ok", "msg": {"version": "1.10.0003"}},
      {"talk": {"version": "1.10.0003"}})
_ke3t("talk-serial", "/api/talk/queryLocalDeviceSerial", {"result": "ok", "msg": {"serial": "201412000F1DB"}},
      {"talk": {"serial": "201412000F1DB"}})
_ke3t("talk-devices", "/api/talk/queryDeviceList",
      {"result": "ok", "msg": {"local_device": {"name": "E3-201412000F1DB", "ip": "192.168.35.35", "type": "device",
                                                "usb_status": False, "talk_status": False,
                                                "connect_status": "noconnect", "connect_ip": None,
                                                "connect_type": None, "version": "1.10.0003", "new_version": True,
                                                "http_port": 80, "https_port": 443},
                               "other_device": []}},
      {"talk": {"name": "E3-201412000F1DB", "ip": "192.168.35.35", "usb_headset": False, "talking": False,
                "connect_status": "noconnect", "devices": "[]"}})
_ke3t("talk-connection", "/api/talk/queryLDeviceConnectStatus",
      {"result": "ok", "msg": {"connect_status": "connected", "connect_ip": "192.168.40.100"}},
      {"talk": {"connect_status": "connected", "connect_ip": "192.168.40.100"}})
_ke3t("onvif", "/api/onvifmanage/onvif/get",
      {"discoverable": True, "control": {"enabled": True, "discover_port": 3702, "port": 8080}, "admin_name": "admin",
       "admin_password": "", "auth_req": False, "location": "default", "device_name": "", "device_type": ""},
      {"onvif": {"enabled": True, "discoverable": True, "discover_port": 3702, "port": 8080, "auth_required": False,
                 "admin_name": "admin", "location": "default", "device_name": "", "device_type": ""}})
_ke3t("ndi-bridge", "/api/bridge/get",
      {"enable": True, "recv": "Public", "address": "192.168.35.70", "port": "5990", "secure": "", "id": "E3"},
      {"ndi_bridge": {"enabled": True, "group": "Public", "address": "192.168.35.70", "port": 5990,
                      "source_name": "E3"}})

# ── Read-backs: a write answered ok queues the read of what it changed ───
_KE3_DONE = '{"result":"ok","msg":null}'


def _ke3rb(name, path, then, request=None, state=None):
    extra = {"expect_then_send": [{"method": m, "target": t} for m, t in then]}
    if request is not None:
        extra["request"] = request
    _ke3t("readback-" + name, path, _KE3_DONE, state or {}, **extra)


_ke3rb("users", "/api/systemctrl/users/add", [("GET", "/api/systemctrl/users/list")])
_ke3rb("auth", "/api/systemctrl/users/auth_info", [("GET", "/api/systemctrl/users/auth_info")], {"enable": True})
_ke3rb("time", "/api/systemctrl/systime/setTime", [("GET", "/api/systemctrl/systime/get")])
_ke3rb("hostname", "/api/systemctrl/system/setHostname?name=E3", [("GET", "/api/systemctrl/system/getHostname")])
_ke3rb("protocol", "/api/systemctrl/system/setProtocol", [("GET", "/api/systemctrl/system/getProtocol")])
_ke3rb("ssl", "/api/systemctrl/system/resetSsl", [("GET", "/api/systemctrl/system/ssl")])
_ke3rb("production", "/api/systemctrl/system/production_off", [("GET", "/api/systemctrl/system/production_stat")])
_ke3rb("presets", "/api/systemctrl/system_preset/df5bbf9a-3c14-4444-a477-ab77e407cb7f/load",
       [("GET", "/api/systemctrl/system_preset/get")])
_ke3rb("maintenance", "/api/systemctrl/system_maintenance/set", [("GET", "/api/systemctrl/system_maintenance/get")])
_ke3rb("storage", "/api/record/nas/add", [("GET", "/api/record/record/disk_list")])
_ke3rb("record-settings", "/api/record/record/settings", [("GET", "/api/record/record/get_record_info")])
_ke3rb("recording", "/api/record/sdi/recording", [("GET", "/api/record/sdi/recording")], {"start": True})
_ke3rb("streams", "/api/streamer/hdmi/stream/enable", [("GET", "/api/streamer/hdmi/stream/list")])
_ke3rb("stream-modify", "/api/streamer/pip/stream/a08290a345356a77", [("GET", "/api/streamer/pip/stream/list")])
_ke3rb("media-user-add", "/api/streamer/hdmi/stream/media_user", [("GET", "/api/streamer/hdmi/stream/media_user")],
       {"username": "viewer", "password": "x"})
_ke3rb("media-user-delete", "/api/streamer/hdmi/stream/media_user/viewer",
       [("GET", "/api/streamer/hdmi/stream/media_user")])
_ke3rb("ptz", "/api/streamer/ptz/modify", [("GET", "/api/streamer/ptz/info")])
_ke3rb("quick-stream", "/api/streamer/panel_stream/shortcuts_stream",
       [("GET", "/api/streamer/panel_stream/shortcuts_stream")])
_ke3rb("tally", "/api/streamer/panel_stream/set_tally_setting", [("GET", "/api/streamer/panel_stream/get_tally_setting")])
_ke3rb("quick-config", "/api/streamer/shortcuts/set", [("GET", "/api/streamer/shortcuts/list")])
_ke3rb("network", "/api/networkmanager/network/ChangeLinkinfo", [("GET", "/api/networkmanager/network/GetLinkinfo")])
_ke3rb("routes", "/api/networkmanager/static/routing/del", [("GET", "/api/networkmanager/static/routing/get")])
_ke3rb("kilolink", "/api/kilolink/set", [("GET", "/api/kilolink/get")])
_ke3rb("edid", "/api/codec/vin/edid/priority", [("GET", "/api/codec/vin/edid")])
_ke3rb("vin-source", "/api/codec/uvc/vin/source", [("GET", "/api/codec/uvc/vin/detail")])
_ke3rb("audio-input", "/api/codec/sdi/audio/input/source", [("GET", "/api/codec/sdi/audio/input")])
_ke3rb("audio-encoder", "/api/codec/hdmi/audio/encode/1", [("POST", "/api/codec/hdmi/audio/encode/list")])
_ke3rb("venc", "/api/codec/hdmi/venc/settings",
       [("GET", "/api/codec/hdmi/venc/settings?venc_type=main"), ("GET", "/api/codec/hdmi/venc/settings?venc_type=sub")],
       {"venc_type": "main", "codec": "H264"})
_ke3rb("venc-enable", "/api/codec/uvc/venc/enable",
       [("GET", "/api/codec/uvc/venc/settings?venc_type=main"), ("GET", "/api/codec/uvc/venc/settings?venc_type=sub"),
        ("GET", "/api/codec/vin/active_interface")])
_ke3rb("pip-swap", "/api/codec/pip/venc/pip_back_update", [("GET", "/api/codec/pip/venc/pip_layout_setting")])
_ke3rb("pip-input", "/api/codec/pip/venc/pip_fps_setting", [("GET", "/api/codec/pip/venc/pip_fps_setting")],
       {"fps": 50, "width": 1920, "height": 1080})
_ke3rb("images", "/api/codec/vpss/image/3f33d7027c5111eea32f180dab8597db", [("GET", "/api/codec/vpss/image")])
_ke3rb("crop", "/api/codec/sdi/vpss/input/crop", [("GET", "/api/codec/sdi/vpss/input/crop")], {"crop_en": True})
_ke3rb("crop-apply", "/api/codec/pip/vpss/input/crop_apply", [("GET", "/api/codec/pip/venc/pip_layout_setting")])
_ke3rb("orientation", "/api/codec/uvc/vpss/input/rmf", [("GET", "/api/codec/uvc/vpss/input/rmf")], {"rotate": 90})
_ke3rb("osd-enable", "/api/codec/hdmi/vpss/osd", [("GET", "/api/codec/hdmi/vpss/osd")], {"enable": True})
_ke3rb("osd-item", "/api/codec/hdmi/vpss/osd/897d2f727c6d11eea32f180dab8597db", [("GET", "/api/codec/hdmi/vpss/osd")])
_ke3rb("static-frame", "/api/codec/vpss/static_frame/reset", [("GET", "/api/codec/vpss/static_frame")])
_ke3rb("talk", "/api/talk/addDevice",
       [("GET", "/api/talk/queryDeviceList"), ("GET", "/api/talk/queryLDeviceConnectStatus")])
_ke3rb("onvif", "/api/onvifmanage/onvif/set", [("GET", "/api/onvifmanage/onvif/get")])
_ke3rb("ndi-bridge", "/api/bridge/set", [("GET", "/api/bridge/get")])
# A read answered on the same path queues nothing: the auth_info read is no write.
_ke3t("readback-none-on-read", "/api/systemctrl/users/auth_info", {"result": "ok", "data": {"enable": True}},
      {"system": {"api_auth": True}}, expect_then_send=[])
# A write the device refused queues nothing.
_ke3t("readback-none-on-error", "/api/streamer/hdmi/stream/enable", '{"result":"error","msg":"Parameter error"}', {},
      expect_then_send=[])
