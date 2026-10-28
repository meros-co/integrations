# Kiloview N60 / N50 (kiloview-n60): one vector per command, over the HTTP
# API under /api. Targets and bodies are written here from Kiloview's "N60 &
# N50 API (EN)" (2026-03 upload): each interface's URL, method and example
# request, bodies as compact JSON in the field order of the examples. Query
# forms the document leaves implicit (setHostname, remove_nas, the PTZ
# controls) are the spec's quirk and VERIFICATION items. Replies are the
# document's example replies; telemetry vectors feed them to the state rules.
import json as _json

KN = "kiloview-n60"
_OK = {"ok": {"kind": "ack"}}
_OKR = {"status": 200, "body": '{"result":"ok"}'}


def _kn(command, input, method, target, body=None, **extra):
    request = {"method": method, "target": target}
    if body is not None:
        request["body"] = body
    V.append({"spec": KN, "command": command, "input": input, "expect_request": request, **extra})


def _val(v):
    return {"ok": {"kind": "value", "value": v}}


def _j(obj):
    return _json.dumps(obj, separators=(",", ":"))


def _tel(name, path, body, state, **extra):
    telemetry(KN, name, inbound_http={"path": path, "body": body if isinstance(body, str) else _j(body)},
              expect_state=state, **extra)


def _get(path):
    return {"method": "GET", "target": path}


def _post(path):
    return {"method": "POST", "target": path}


# ── 1.1 Users and login ─────────────────────────────────────────────────
_kn("login", {"username": "admin", "password": "Admin123"}, "POST", "/api/systemctrl/users/login",
    '{"username":"admin","password":"Admin123"}',
    http_reply={"status": 200, "body": '{"result":"ok","data":{"token":"0621da14f9d6546688cf87c5eab9c2f9",'
                '"alias":"Admin","changed":true},"msg":null}'},
    expect_result=_val("0621da14f9d6546688cf87c5eab9c2f9"))
_kn("login", {"username": "admin", "password": "wrong"}, "POST", "/api/systemctrl/users/login",
    '{"username":"admin","password":"wrong"}',
    http_reply={"status": 200, "body": '{"result":"error","msg":"Incorrect username or password"}'},
    expect_result={"error": {"error": "device_error"}}, file="login-refused")
_kn("check_session", {}, "GET", "/api/systemctrl/users/session", http_reply=_OKR, expect_result=_OK)
_kn("list_users", {}, "GET", "/api/systemctrl/users/list",
    http_reply={"status": 200, "body": '{"result":"ok","data":[{"id":"admin","alias":"Admin","web":true,'
                '"api":true,"create_time":"1970-01-01 00:00:00"}]}'},
    expect_result=_val([{"id": "admin", "alias": "Admin", "web": True, "api": True,
                         "create_time": "1970-01-01 00:00:00"}]))
_kn("add_user", {"id": "test_user", "password": "qwer1234", "alias": "test_user", "enable_web": True,
                 "enable_api": True}, "POST", "/api/systemctrl/users/add",
    '{"id":"test_user","password":"qwer1234","confirmNewPassword":"qwer1234","alias":"test_user",'
    '"enable_web":true,"enable_api":true}', http_reply=_OKR, expect_result=_OK)
_kn("remove_user", {"id": "Jack"}, "POST", "/api/systemctrl/users/remove", '{"ids":"Jack"}')
_kn("modify_user", {"id": "test1", "alias": "test11", "enable_web": True, "enable_api": False}, "POST",
    "/api/systemctrl/users/modify", '{"id":"test1","alias":"test11","enable_web":true,"enable_api":false}')

# ── 1.2 System time ─────────────────────────────────────────────────────
_kn("set_timezone", {"timezone": "Asia/Shanghai"}, "POST", "/api/systemctrl/systime/setLocation",
    '{"timezone":"Asia/Shanghai"}')
_kn("set_timezone_with_offset", {"timezone": "Asia/Shanghai", "offset": 8}, "POST",
    "/api/systemctrl/systime/setLocation", '{"timezone":"Asia/Shanghai","offset":8}')
_kn("set_time", {"method": "pc", "time": "2023-08-07 17:37:14"}, "POST", "/api/systemctrl/systime/setTime",
    '{"timetype":"pc","time":"2023-08-07 17:37:14"}', http_reply=_OKR, expect_result=_OK)
_kn("set_time_ntp", {"servers": "ntp1.aliyun.com ntp2.aliyun.com"}, "POST", "/api/systemctrl/systime/setTime",
    '{"timetype":"ntp","ntp":"ntp1.aliyun.com ntp2.aliyun.com"}')

# ── 1.3 System configuration ────────────────────────────────────────────
_kn("get_open_source_licenses", {}, "GET", "/api/systemctrl/system/components",
    http_reply={"status": 200, "body": '[{"name":"vue","url":"MIT","software":"MIT License",'
                '"address":"https://github.com/vuejs/vue"}]'},
    expect_result=_val([{"name": "vue", "url": "MIT", "software": "MIT License",
                         "address": "https://github.com/vuejs/vue"}]))
_kn("get_hostname", {}, "GET", "/api/systemctrl/system/getHostname",
    http_reply={"status": 200, "body": '{"result":"ok", "data":{"hostname": "N60-123456"}}'},
    expect_result=_val("N60-123456"))
_kn("set_hostname", {"name": "N60 Stage"}, "GET", "/api/systemctrl/system/setHostname?name=N60%20Stage",
    http_reply=_OKR, expect_result=_OK)
_kn("get_web_protocols", {}, "GET", "/api/systemctrl/system/getProtocol")
_kn("set_web_protocols", {"http": True, "https": True}, "POST", "/api/systemctrl/system/setProtocol",
    '{"http":true,"https":true,"httpPort":80,"httpsPort":443}')
_kn("factory_reset", {}, "GET", "/api/systemctrl/system/reFactory", http_reply=_OKR, expect_result=_OK)
_kn("restart_codec", {}, "GET", "/api/systemctrl/system/restore")
_kn("reboot", {}, "GET", "/api/systemctrl/system/reboot", http_reply=_OKR, expect_result=_OK)
_kn("get_network_speed", {}, "GET", "/api/systemctrl/system/getNetworkSpeed")
_kn("get_system_info", {}, "GET",
    "/api/systemctrl/system/getSystemInfo?version=true&cpu=true&memory=true&disk=true&persisTime=true")
_kn("set_serial_number", {"serial_number": "7116110002301"}, "GET",
    "/api/systemctrl/system/Rpj1Tn92uH3UiCCLPQeyLMd73hElBj6G/setSerial?serial_number=7116110002301")
# multipart/form-data with one part, file, as the core frames it (SPEC.md §4,
# "Binary request bodies"): the boundary is derived from the parts' content.
def _form(parts):
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


_FW_BODY, _FW_TYPE = _form([("file", b"\x00\x01\x02\x03", "N60.bin")])

V.append({"spec": KN, "command": "upload_firmware", "input": {"file": "AAECAw==", "filename": "N60.bin"},
          "expect_request": {"method": "POST", "target": "/api/systemctrl/system/upload", "body_hex": _FW_BODY.hex(),
                             "content_type": _FW_TYPE},
          "http_reply": _OKR, "expect_result": _OK})
_kn("set_production_service", {"enabled": True}, "POST", "/api/systemctrl/system/production_off",
    '{"value":true}', http_reply={"status": 200, "body": '{"result": "ok", "data": {"value": true}}'},
    expect_result=_OK)
_kn("get_production_service", {}, "GET", "/api/systemctrl/system/production_stat",
    http_reply={"status": 200, "body": '{"result": "ok", "data": {"value": true}}'}, expect_result=_val(True))

# ── 2.1 Decoding ────────────────────────────────────────────────────────
_kn("select_source", {"name": "N60 (Decoding Channel)", "url": "192.168.42.249:5961", "group": "public"}, "POST",
    "/api/codec/decode/addSpec", '{"name":"N60 (Decoding Channel)","url":"192.168.42.249:5961","group":"public"}',
    http_reply=_OKR, expect_result=_OK)
_kn("select_preset", {"id": 2}, "POST", "/api/codec/decode/add", '{"id":2}', http_reply=_OKR, expect_result=_OK)
_kn("select_preset", {"id": 0}, "POST", "/api/codec/decode/add", '{"id":0}', file="select_preset-blank")
_kn("get_decode_info", {}, "GET", "/api/codec/decode/get")
_ident = ",".join('{"output_channel":%d,"source_channel":%d}' % (i, i) for i in range(1, 9))
_kn("set_decode_output", {"output_resolution": "1920x1080", "hdmi_channels": "2", "line_out_channels": 2,
                          "hdcp": True, "out_colorspace": "rgb444"}, "POST", "/api/codec/decode/output_set",
    '{"output_resolution":"1920x1080","hdmi_channels":2,"output_framerate":0,"hdmi_channels_mapping":[' + _ident
    + '],"line_out_channels":2,"line_out_channels_mapping":[' + _ident + '],"hdcp":1,"out_colorspace":0}',
    http_reply=_OKR, expect_result=_OK)
_swap = ('{"output_channel":1,"source_channel":2},{"output_channel":2,"source_channel":1},'
         + ",".join('{"output_channel":%d,"source_channel":%d}' % (i, i) for i in range(3, 9)))
_kn("set_decode_output", {"output_resolution": "auto", "hdmi_channels": "8", "line_out_channels": 2, "hdcp": False,
                          "out_colorspace": "yuv422", "hdmi_1": 2, "hdmi_2": 1, "line_1": 2, "line_2": 1},
    "POST", "/api/codec/decode/output_set",
    '{"output_resolution":"auto","hdmi_channels":8,"output_framerate":0,"hdmi_channels_mapping":[' + _swap
    + '],"line_out_channels":2,"line_out_channels_mapping":[' + _swap + '],"hdcp":0,"out_colorspace":1}',
    file="set_decode_output-swapped")
_kn("set_decode_ndi_connection", {"connection": "rudp"}, "POST", "/api/codec/decode/setConnection",
    '{"ndi_connection":"rudp"}')

# ── 2.2 Discovery ───────────────────────────────────────────────────────
_kn("set_discovery_server", {"enabled": True, "address": "192.168.4.66"}, "POST",
    "/api/codec/discovery/setDiscoveryServer", '{"enable":true,"address":"192.168.4.66"}')
_kn("get_discovery_server", {}, "GET", "/api/codec/discovery/getDiscoveryServer",
    http_reply={"status": 200, "body": '{"result":"ok","data":{"enable":false,"address":""}}'},
    expect_result=_val({"enable": False, "address": ""}))
_kn("scan_ndi_sources", {}, "GET", "/api/codec/discovery/scan")
_kn("set_manual_sources", {"groups": ["public"], "manuals": ["128.168.38.207"]}, "POST",
    "/api/codec/discovery/addManualIpsGroups", '{"groups":["public"],"manuals":["128.168.38.207"]}')
_kn("get_manual_sources", {}, "GET", "/api/codec/discovery/getManualIpsGroups")
_kn("set_decode_discovery_name", {"group": "public", "channel": "Decoding Channel"}, "POST",
    "/api/codec/discovery/setDiscoveryGroupChannel", '{"group_name":"public","channel_name":"Decoding Channel"}',
    http_reply={"status": 200, "body": ""}, expect_result=_OK)
_kn("get_decode_discovery_name", {}, "GET", "/api/codec/discovery/getDiscoveryGroupChannel")

# ── 2.3 Presets ─────────────────────────────────────────────────────────
_kn("get_presets", {}, "GET", "/api/codec/preset/get")
_kn("add_preset", {"position": 2, "name": "N60X (Chan1)", "url": "192.168.38.61:5961", "group": "public"}, "POST",
    "/api/codec/preset/add", '{"position":2,"name":"N60X (Chan1)","url":"192.168.38.61:5961","group":"public"}')
_kn("remove_preset", {"id": 2}, "POST", "/api/codec/preset/remove", '{"id":2}')
_kn("set_blank_color", {"color": "#ffffff"}, "POST", "/api/codec/preset/set_blank_color", '{"BlankColor":"#ffffff"}')

# ── 2.4 Decoding PTZ ────────────────────────────────────────────────────
_kn("decode_ptz_control", {"action": "up"}, "GET", "/api/codec/ptz_decode/ptzControl?action=up&speed=50",
    http_reply=_OKR, expect_result=_OK)
_kn("decode_ptz_control", {"action": "left"}, "GET", "/api/codec/ptz_decode/ptzControl?action=left&speed=50",
    http_reply={"status": 200, "body": '{"result": "error", "msg": "Invalid PTZ Motion Parameter"}'},
    expect_result={"error": {"error": "device_error"}}, file="decode_ptz_control-refused")

# ── 2.5 Encoding ────────────────────────────────────────────────────────
_kn("list_encoders", {}, "POST", "/api/codec/encoders/list",
    http_reply={"status": 200, "body": '["main","sub","main_full"]'}, expect_result=_val(["main", "sub", "main_full"]))
_kn("set_encode_streams", {"hx": True, "hb": True}, "POST", "/api/codec/encoders/choose_encode",
    '{"main":1,"main_full":1,"srt":0}', http_reply=_OKR, expect_result=_OK)
_kn("get_encoder_params", {"stream": "main"}, "POST", "/api/codec/encoder/main/get",
    http_reply={"status": 200, "body": '{"yuv":"NV12","max_bitrate":10000,"mode":"x-h264","started":true}'},
    expect_result=_val({"yuv": "NV12", "max_bitrate": 10000, "mode": "x-h264", "started": True}))
_kn("get_encoder_input", {"stream": "main_full"}, "POST", "/api/codec/encoder/main_full/get_capture")
_kn("set_encoder_quality", {"stream": "main", "quality": "150"}, "POST", "/api/codec/encoder/main/set",
    '{"target_bitrate":150,"key":"main"}', http_reply=_OKR, expect_result=_OK)
_kn("set_encoder_quality", {"stream": "main_full", "quality": "200"}, "POST", "/api/codec/encoder/main_full/set",
    '{"target_bitrate":200,"key":"main_full"}', file="set_encoder_quality-hb")
_kn("set_encoder_quality", {"stream": "multi_protocol", "quality": "75"}, "POST",
    "/api/codec/encoder/multi_protocol/set", '{"target_bitrate":75,"key":"multi_protocol"}',
    file="set_encoder_quality-srt")
_kn("set_hx_codec", {"codec": "h265"}, "POST", "/api/codec/encoder/main/set", '{"mode_version":2,"key":"main"}')
_kn("set_hx_version", {"version": "3"}, "POST", "/api/codec/encoder/main/set", '{"hx_version":3,"key":"main"}')
_kn("set_hx_color_format", {"format": "NV16_10LE32"}, "POST", "/api/codec/encoder/main/set",
    '{"yuv":"NV16_10LE32","key":"main"}')
_kn("get_stream_list", {"stream": "main"}, "GET", "/api/codec/streams/main/list")
_kn("restart_stream", {"stream": "main_full"}, "GET", "/api/codec/streams/main_full/reset",
    http_reply=_OKR, expect_result=_OK)
_kn("get_stream_config", {"stream": "main"}, "GET", "/api/codec/streams/main/ndi-hx/get")
_kn("get_stream_config", {"stream": "multi_protocol"}, "GET", "/api/codec/streams/multi_protocol/srt/get",
    file="get_stream_config-srt")
_kn("get_stream_state", {"stream": "main_full"}, "GET", "/api/codec/streams/main_full/ndi-full/get_dynamic_state",
    http_reply={"status": 200, "body": '{"bitrate": 0, "connections": 0}'},
    expect_result=_val({"bitrate": 0, "connections": 0}))
_kn("set_stream_config", {"stream": "main", "group": "", "channel_name": "Chan1-248", "connection": "rudp"}, "POST",
    "/api/codec/streams/main/ndi-hx/set", '{"group":"","channel_name":"Chan1-248","connection":"rudp","types":"ndi-hx"}',
    http_reply=_OKR, expect_result=_OK)

# ── 2.6 Mode ────────────────────────────────────────────────────────────
_kn("set_mode", {"mode": "decode"}, "POST", "/api/codec/mode/set?mode=decode",
    http_reply={"status": 200, "body": "true"}, expect_result=_OK)
_kn("get_mode", {}, "GET", "/api/codec/mode/get", http_reply={"status": 200, "body": "encode"},
    expect_result=_val("encode"))

# ── 2.7 Encoding PTZ ────────────────────────────────────────────────────
_kn("ptz_control", {"action": "stop-move"}, "GET", "/api/codec/ptz/ptzControl?action=stop-move&speed=50",
    http_reply=_OKR, expect_result=_OK)
_kn("get_ptz_info", {}, "GET", "/api/codec/ptz/getPtzInfo")
_kn("get_ptz_protocols", {}, "GET", "/api/codec/ptz/getPtzProtocol",
    http_reply={"status": 200, "body": '{"result":"ok","msg":null,"data":["Sony Visca","Pelco-d","Pelco-p"]}'},
    expect_result=_val(["Sony Visca", "Pelco-d", "Pelco-p"]))
_kn("get_ptz_serial_devices", {}, "GET", "/api/codec/ptz/getSerialDevice")
_kn("set_ptz_network", {"enabled": True, "transport": "UDP", "host": "192.168.38.18", "port": 50381}, "POST",
    "/api/codec/ptz/modify",
    '{"enable":1,"typ":"network","ptz_protocol":"Sony Visca","ptz_addr":1,"protocol":"UDP","addr":"192.168.38.18",'
    '"port":50381}', http_reply={"status": 200, "body": '{"result":"ok","msg":null}'}, expect_result=_OK)
_kn("set_ptz_serial", {"enabled": True, "device": "ttyUSB0", "protocol": "Pelco-d", "address": 2}, "POST",
    "/api/codec/ptz/modify",
    '{"enable":1,"typ":"serial","device":"ttyUSB0","baudrate":9600,"parity":"none","startBits":8,"endBits":1,'
    '"xonxoff":0,"rtscts":0,"ptz_protocol":"Pelco-d","ptz_addr":2}')

# ── 2.8 QR code ─────────────────────────────────────────────────────────
_kn("qr_virtual_login", {}, "GET", "/api/codec/qr_code/virtual_login",
    http_reply={"status": 200, "body": '{"result":"ok","data":{"current_virtual_user":"1e1ffd2ec06c43e6a9efe6022162ac74"}}'},
    expect_result=_val("1e1ffd2ec06c43e6a9efe6022162ac74"))
_kn("qr_get_id", {}, "GET", "/api/codec/qr_code/qrcode_get",
    http_reply={"status": 200, "body": '{"result":"ok","data":{"qrcode_id":"84598cea1a878c6f2b7e3db89977b0e2"}}'},
    expect_result=_val("84598cea1a878c6f2b7e3db89977b0e2"))
_kn("qr_verify", {"virtual_user": "1e1ffd2ec06c43e6a9efe6022162ac74", "qrcode_id": "84598cea1a878c6f2b7e3db89977b0e2"},
    "POST", "/api/codec/qr_code/qrcode_session",
    '{"current_virtual_user":"1e1ffd2ec06c43e6a9efe6022162ac74","qrcode_id":"84598cea1a878c6f2b7e3db89977b0e2"}')
_kn("qr_check_decode_mode", {}, "GET", "/api/codec/qr_code/check_mode",
    http_reply={"status": 200, "body": '{"result":"verficationError","msg":"Please set the device to decoding mode '
                'and re-scan the QR!"}'},
    expect_result={"error": {"error": "device_error"}})
_kn("qr_get_image_path", {}, "GET", "/api/codec/qr_code/get_qrcode",
    http_reply={"status": 200, "body": '{"result":"ok","data":{"image_path":'
                '"192.168.38.217/api/codec/qrcode_img/84598cea1a878c6f2b7e3db89977b0e2_PC.png"}}'},
    expect_result=_val("192.168.38.217/api/codec/qrcode_img/84598cea1a878c6f2b7e3db89977b0e2_PC.png"))

# ── 3.1 Network ─────────────────────────────────────────────────────────
_kn("get_network", {}, "GET", "/api/networkmanager/network/GetLinkinfo")
_kn("get_interface", {"interface": "eth0"}, "GET", "/api/networkmanager/network/GetEthernets?ifname=eth0")
_kn("set_interface_enabled", {"interface": "eth1", "enabled": False}, "POST", "/api/networkmanager/network/SetSwitch",
    '{"ifname":"eth1","enable":false}')
_kn("set_interface", {"interface": "eth0", "method": "static", "address": "192.168.38.217", "netmask": "255.255.255.0",
                      "gateway": "192.168.38.254", "dns": "223.5.5.5;224.5.5.5", "mac": "56:3D:69:9B:5A:C7"}, "POST",
    "/api/networkmanager/network/SetEthernets",
    '{"ifname":"eth0","address":"192.168.38.217","netmask":"255.255.255.0","gw":"192.168.38.254",'
    '"mac":"56:3D:69:9B:5A:C7","method":"static","dns":"223.5.5.5;224.5.5.5"}', http_reply=_OKR, expect_result=_OK)
_kn("set_bonding", {"mode": "round-robin", "method": "dhcp", "address": "192.168.43.77", "netmask": "255.255.255.0",
                    "gateway": "192.168.43.254", "dns": ["223.5.5.5", "223.6.6.6"], "mac": "96:82:70:FA:36:98"}, "POST",
    "/api/networkmanager/network/SetBonding",
    '{"mac":"96:82:70:FA:36:98","address":"192.168.43.77","netmask":"255.255.255.0","gw":"192.168.43.254",'
    '"dns":["223.5.5.5","223.6.6.6"],"mode":"0","method":"dhcp"}', http_reply={"status": 200, "body": ""},
    expect_result=_OK)
_kn("get_bonding", {}, "GET", "/api/networkmanager/network/GetBondingInfo")
_kn("restart_mdns", {}, "GET", "/api/networkmanager/network/ResetMDNS", http_reply=_OKR, expect_result=_OK)

# ── 3.2 Static routes ───────────────────────────────────────────────────
_kn("add_route", {"interface": "eth0", "target": "192.168.38.12", "netmask": "255.255.255.0",
                  "gateway": "192.168.38.254"}, "POST", "/api/networkmanager/static/routing/add",
    '{"interface":"eth0","target":"192.168.38.12","netmask":"255.255.255.0","gateway":"192.168.38.254","mtu":1500,'
    '"metric":0}')
_kn("get_routes", {}, "GET", "/api/networkmanager/static/routing/get")
_kn("modify_route", {"id": 1, "interface": "eth0", "target": "192.168.38.12", "netmask": "255.255.255.0",
                     "gateway": "192.168.38.254", "metric": 10}, "POST", "/api/networkmanager/static/routing/modify",
    '{"id":1,"interface":"eth0","target":"192.168.38.12","netmask":"255.255.255.0","gateway":"192.168.38.254",'
    '"mtu":1500,"metric":10}')
_kn("delete_route", {"id": 1}, "POST", "/api/networkmanager/static/routing/del", '{"ids":1}')
_kn("set_route_enabled", {"id": 1, "enabled": True}, "POST", "/api/networkmanager/static/routing/set",
    '{"id":1,"status":1}')

# ── 4.1 Kilolink ────────────────────────────────────────────────────────
_kn("get_kilolink_interfaces", {}, "GET", "/api/kilolink/networks",
    http_reply={"status": 200, "body": '{"result":"ok","list":["bond0"]}'}, expect_result=_val(["bond0"]))
_kn("get_kilolink", {}, "GET", "/api/kilolink/get")
_kn("set_kilolink", {"enabled": True, "server": "192.168.41.211", "port": 5421, "interface": "eth0",
                     "key": "is8931ji"}, "POST", "/api/kilolink/set",
    '{"ip":"192.168.41.211","port":5421,"ifname":["eth0"],"key":"is8931ji","crypto":false,"enable":true}')
_kn("validate_kilolink_token", {"token": "abc123"}, "GET", "/api/kilolink/check_proxy_token?token=abc123",
    http_reply={"status": 200, "body": '{"result": "ok", "valid": true}'}, expect_result=_val(True))

# ── 5.1 Talk ────────────────────────────────────────────────────────────
_kn("add_talk_device", {"type": "device", "ip": "192.168.40.100"}, "POST", "/api/talk/addDevice",
    '{"type":"device","ip":"192.168.40.100","http_port":80,"https_port":443}')
_kn("set_talk_device_link", {"local_ip": "192.168.38.74", "device_ip": "192.168.40.100", "enabled": True}, "POST",
    "/api/talk/setLDLinkDEnable", '{"l_dev_ip":"192.168.38.74","dev_ip":"192.168.40.100","enable":true}')
_kn("set_talk_web_link", {"device_ip": "192.168.43.176", "enabled": False}, "POST", "/api/talk/setWLinkDEnable",
    '{"dev_ip":"192.168.43.176","enable":false}')
_kn("set_talk_server_link", {"local_ip": "192.168.35.11", "server_ip": "192.168.40.100", "enabled": True}, "POST",
    "/api/talk/setLDLinkSEnable", '{"l_dev_ip":"192.168.35.11","ser_ip":"192.168.40.100","enable":true}')
_kn("get_talk_status", {}, "GET", "/api/talk/queryLDeviceConnectStatus",
    http_reply={"status": 200, "body": '{"result":"ok","msg":{"connect_status":"noconnect","connect_ip":"0.0.0.0"}}'},
    expect_result=_val({"connect_status": "noconnect", "connect_ip": "0.0.0.0"}))
_kn("get_talk_devices", {}, "GET", "/api/talk/queryDeviceList")
_kn("delete_talk_device", {"ip": "192.168.43.176"}, "POST", "/api/talk/deleteDevice", '{"device_ip":"192.168.43.176"}')
_kn("get_talk_serial", {}, "GET", "/api/talk/queryLocalDeviceSerial",
    http_reply={"status": 200, "body": '{"result":"ok","msg":{"serial":"012354640124"}}'},
    expect_result=_val("012354640124"))
_kn("get_talk_version", {}, "GET", "/api/talk/talkVersion",
    http_reply={"status": 200, "body": '{"result":"ok","msg":{"version":"1.10.0001"}}'},
    expect_result=_val("1.10.0001"))
_kn("set_talk_language", {"language": "cn"}, "POST", "/api/talk/setLanguage", '{"language":"cn"}')

# ── 6 Storage and recording ─────────────────────────────────────────────
_kn("add_nas", {"id": "public", "name": "public", "type": "nfs", "host": "192.168.0.21",
                "volume": "/mnt/disk-data/public"}, "POST", "/api/record/storage/add_nas",
    '{"id":"public","name":"public","type":"nfs","host":"192.168.0.21","volume":"/mnt/disk-data/public","options":""}',
    http_reply=_OKR, expect_result=_OK)
_kn("remove_nas", {"id": "public"}, "GET", "/api/record/storage/remove_nas?id=public")
_kn("get_disks", {}, "GET", "/api/record/storage/disk_list")
_kn("get_recorded_files", {"device": "/run/media/sda1"}, "GET",
    "/api/record/storage/get_disk_files?device=%2Frun%2Fmedia%2Fsda1&stream=ndi_hx")
_kn("delete_recorded_file", {"path": "/run/media/sda1/RECORD/M20230922_165622_761459/REC-NDI_HX-202309221656227614590.mov"},
    "POST", "/api/record/storage/remove_file",
    '{"path":"/run/media/sda1/RECORD/M20230922_165622_761459/REC-NDI_HX-202309221656227614590.mov"}',
    http_reply=_OKR, expect_result=_OK)
_kn("set_record_settings", {"disk_choose": {"nas": ["public"], "usb": ["sda1"]}, "limit_type": "size-loop",
                            "limit_size": 102400, "limit_time": 3600, "disk_policy": "overwrite"}, "POST",
    "/api/record/record/settings",
    '{"disk_choose":{"nas":["public"],"usb":["sda1"]},"auto_record":false,"format":"mov","limit_type":"size-loop",'
    '"limit_size":102400,"limit_time":3600,"disk_policy":"overwrite"}',
    http_reply={"status": 200, "body": ""}, expect_result=_OK)
_kn("get_record_settings", {}, "GET", "/api/record/record/get_record_info")
_kn("start_recording", {}, "POST", "/api/record/ndi_hx/recording", '{"start":true}',
    http_reply={"status": 200, "body": ""}, expect_result=_OK)
_kn("stop_recording", {}, "POST", "/api/record/ndi_hx/recording", '{"start":false}')


# ── Telemetry: the document's example replies ───────────────────────────
_tel("mode", "/api/codec/mode/get", "encode", {"mode": "encode"})
# The Companion module reads a JSON reply holding the mode on newer firmware.
_tel("mode-json", "/api/codec/mode/get", '{"result":"ok","data":{"mode":"decode"}}', {"mode": "decode"})
_tel("hostname", "/api/systemctrl/system/getHostname", '{"result":"ok", "data":{"hostname": "N60-123456"}}',
     {"system": {"hostname": "N60-123456"}})
_tel("web-protocols", "/api/systemctrl/system/getProtocol",
     {"result": "ok", "msg": None, "data": {"http": True, "https": False, "httpPort": 80, "httpsPort": 443}},
     {"system": {"http_enabled": True, "https_enabled": False, "http_port": 80, "https_port": 443}})
_tel("production-service", "/api/systemctrl/system/production_stat", {"result": "ok", "data": {"value": True}},
     {"system": {"production_service": True}})
_tel("system-info", "/api/systemctrl/system/getSystemInfo?version=true&cpu=true&memory=true&disk=true&persisTime=true",
     {"result": "ok", "data": {
         "version": {"ndiVersion": "5.1.3", "hardwareVersion": "1.0", "softwareVersion": "2.02.0008",
                     "serialNumber": "00000000", "product": "N60", "oem": ""},
         "cpu": {"precent": 2.624}, "mem": {"precent": 16.282, "total": 4039368, "used": 248088},
         "disk": {"precent": 28.5, "free": 1350944, "used": 513328, "total": 1983056},
         "persisTime": {"uptime": 1933.73}}},
     {"system": {"firmware": "2.02.0008", "hardware_version": "1.0", "ndi_version": "5.1.3",
                 "serial_number": "00000000", "product": "N60", "oem": "", "cpu_percent": 2.624,
                 "memory_percent": 16.282, "memory_total_kb": 4039368, "memory_used_kb": 248088,
                 "disk_percent": 28.5, "disk_free_kb": 1350944, "disk_used_kb": 513328, "disk_total_kb": 1983056,
                 "uptime_s": 1933.73}})
_tel("network-speed", "/api/systemctrl/system/getNetworkSpeed",
     {"result": "ok", "data": {"eth0": {"receive": 18.55, "transmit": 433.52, "speed": 1000}, "eth1": {}}},
     {"network": {"traffic": {"eth0": {"receive": 18.55, "transmit": 433.52, "link_speed": 1000}}}})
_tel("users", "/api/systemctrl/users/list",
     {"result": "ok", "data": [{"id": "admin", "alias": "Admin", "web": True, "api": True,
                                "create_time": "1970-01-01 00:00:00"}]},
     {"users": {"admin": {"alias": "Admin", "web": True, "api": True, "created": "1970-01-01 00:00:00"}}},
     # The user "old" was deleted on the web page: the list replaces the users.
     state_before={"users": {"old": {"alias": "Old", "web": True, "api": False, "created": "2023-01-01 00:00:00"}}})
_map = [{"output_channel": i, "source_channel": i} for i in range(1, 9)]
_tel("decode-info", "/api/codec/decode/get",
     {"result": "ok", "data": {
         "name": "DESKTOP-R6TEOQE (vMix - Output 1)", "ip": "192.168.38.238", "online": True,
         "resolution": "1920x1080p 60Hz", "codec": "SHQ2", "bitrate": 132056, "deInterlace": False, "interlaced": 0,
         "frame_rate": 60, "audio": "48.0KHz / 2CH", "url": "192.168.38.238:5961", "xRes": 1920, "yRes": 1080,
         "audio_sampling": 48000, "audio_channels": 2, "hdcp": 1, "out_colorspace": 0, "supported": True,
         "output_resolution_choose": "1920x1080", "output_resolution": "1920x1080p 60Hz", "hdmi_channels": 2,
         "hdmi_channels_mapping": _map, "line_out_channels": 2, "line_out_channels_mapping": _map,
         "ndi_connection": "tcp"}},
     {"decoder": {"source_name": "DESKTOP-R6TEOQE (vMix - Output 1)", "source_ip": "192.168.38.238",
                  "source_url": "192.168.38.238:5961", "source_online": True, "source_supported": True,
                  "resolution": "1920x1080p 60Hz", "width": 1920, "height": 1080, "frame_rate": 60,
                  "interlaced": False, "deinterlace": False, "codec": "SHQ2", "bitrate_kbps": 132056,
                  "audio": "48.0KHz / 2CH", "audio_sample_rate": 48000, "audio_channels": 2, "hdcp": True,
                  "output_colorspace": "rgb444", "output_resolution_setting": "1920x1080",
                  "output_resolution": "1920x1080p 60Hz", "hdmi_channels": 2, "hdmi_channel_map": _j(_map),
                  "line_out_channels": 2, "line_out_channel_map": _j(_map), "ndi_connection": "tcp"}})
_tel("discovery-server", "/api/codec/discovery/getDiscoveryServer",
     '{"result":"ok","data":{"enable":true,"address":"192.168.4.66"}}',
     {"discovery": {"server_enabled": True, "server_address": "192.168.4.66"}})
_tel("manual-sources", "/api/codec/discovery/getManualIpsGroups",
     '{"result":"ok","data":{"groups":["public"],"manuals":["128.168.38.207","10.0.0.9"]}}',
     {"discovery": {"manual_groups": '["public"]', "manual_addresses": '["128.168.38.207","10.0.0.9"]'}})
_tel("decoder-discovery-name", "/api/codec/discovery/getDiscoveryGroupChannel",
     {"result": "ok", "data": {"group_name": "public", "channel_name": "Decoding Channel"}},
     {"discovery": {"decoder_group": "public", "decoder_channel": "Decoding Channel"}})
_tel("ndi-sources", "/api/codec/discovery/scan",
     {"result": "ok", "data": [{"name": "DESKTOP-R6TEOQE (vMix - Output 1)", "group": "public",
                                "original_url": "192.168.38.238:5961", "ip": "192.168.38.238", "port": 5961, "id": 1,
                                "group_name": "public", "device_name": "DESKTOP-R6TEOQE",
                                "channel_name": "vMix - Output 1", "series": "NDI", "enable": 1,
                                "url": "192.168.38.238:5961"}], "data_size": 1},
     {"ndi": {"sources": {"1": {"name": "DESKTOP-R6TEOQE (vMix - Output 1)", "group": "public",
                                "url": "192.168.38.238:5961", "ip": "192.168.38.238", "port": 5961,
                                "device_name": "DESKTOP-R6TEOQE", "channel_name": "vMix - Output 1"}}}},
     state_before={"ndi": {"sources": {"2": {"name": "GONE (Cam)", "url": "10.0.0.5:5961"}}}})
_tel("presets", "/api/codec/preset/get",
     {"result": "ok", "data": [
         {"id": 1, "name": "N60X (Chan1)", "url": "192.168.38.61:5961", "group": "public", "online": "on",
          "warning": "", "device_name": "N60X", "channel_name": "Chan1", "ip": "192.168.38.61", "port": 5961,
          "enable": 1},
         {"id": 2, "name": "DESKTOP-R6TEOQE (vMix - Output 1)", "url": "192.168.38.238:5961", "group": "public",
          "online": "off", "warning": "", "device_name": "DESKTOP-R6TEOQE", "channel_name": "vMix - Output 1",
          "ip": "192.168.38.238", "port": 5961, "enable": 1}]},
     {"presets": {
         "1": {"defined": True, "name": "N60X (Chan1)", "url": "192.168.38.61:5961", "group": "public",
               "online": True, "warning": "", "device_name": "N60X", "channel_name": "Chan1",
               "ip": "192.168.38.61", "port": 5961},
         "2": {"defined": True, "name": "DESKTOP-R6TEOQE (vMix - Output 1)", "url": "192.168.38.238:5961",
               "group": "public", "online": False, "warning": "", "device_name": "DESKTOP-R6TEOQE",
               "channel_name": "vMix - Output 1", "ip": "192.168.38.238", "port": 5961}}},
     state_before={"presets": {"3": {"defined": True, "name": "OLD (Cam)"}}})
_tel("encoder-params", "/api/codec/encoder/main/get",
     {"yuv": "NV12", "max_bitrate": 10000, "gdr_mode": 0, "gop_length": 60, "b_frames": 0, "target_bitrate": 10000,
      "cpb_size": 1000, "initial_delay": 500, "periodicity_idr": 60, "prefetch_buffer": True, "low_bandwidth": False,
      "filler_data": True, "control_rate": "cbr", "hx_version": 2, "mode": "x-h264", "profile": "Main",
      "alignment": "nal", "mode_version": 1, "started": True},
     {"encoders": {"main": {"color_format": "NV12", "max_bitrate": 10000, "target_bitrate": 10000,
                            "rate_control": "cbr", "gdr_mode": 0, "gop_length": 60, "b_frames": 0, "cpb_size": 1000,
                            "initial_delay_ms": 500, "idr_period": 60, "prefetch_buffer": True,
                            "low_bandwidth": False, "filler_data": True, "hx_version": 2, "codec_name": "x-h264",
                            "codec": "h264", "profile": "Main", "alignment": "nal", "started": True}}})
_tel("encoder-input", "/api/codec/encoder/main/get_capture",
     {"width": 1920, "height": 1080, "framerate": 60, "interlaced": 0, "io_mode": 4, "device": "/dev/video0",
      "framerate_D": 1000, "enable": 1, "started": True, "signal": "none", "resolution": "1920x1080p 60Hz",
      "source": "hdmi"},
     {"encoders": {"main": {"enabled": True, "pipeline_started": True, "input": {
         "width": 1920, "height": 1080, "frame_rate": 60, "frame_rate_denominator": 1000, "interlaced": False,
         "io_mode": 4, "device": "/dev/video0", "signal": "none", "resolution": "1920x1080p 60Hz",
         "source": "hdmi"}}}})
_tel("stream-config", "/api/codec/streams/main/ndi-hx/get",
     {"type": "NDI-HX", "name": "ndi-hx", "enable": True, "group": "1212222", "ttl": "127", "channel_name": "Chan1",
      "connection": "disable_rudp", "netmask": "255.255.0.0", "netprefix": "239.255.0.0",
      "discovery_server": {"enable": False, "address": ""}, "location": None, "started": True, "source": "hdmi"},
     {"streams": {"main": {"type": "NDI-HX", "name": "ndi-hx", "enabled": True, "started": True, "group": "1212222",
                           "channel_name": "Chan1", "connection": "disable_rudp", "multicast_ttl": 127,
                           "multicast_netmask": "255.255.0.0", "multicast_prefix": "239.255.0.0",
                           "discovery_server_enabled": False, "discovery_server_address": "", "source": "hdmi"}}})
_tel("stream-state", "/api/codec/streams/multi_protocol/srt/get_dynamic_state", '{"bitrate": 8123, "connections": 0}',
     {"streams": {"multi_protocol": {"bitrate": 8123, "connections": 0}}})
_tel("ptz-info", "/api/codec/ptz/getPtzInfo",
     {"result": "ok", "data": {"enable": 1, "typ": "network",
                               "network": {"protocol": "TCP", "addr": "192.168.38.18", "port": 50381,
                                           "ptz_protocol": "Sony Visca", "ptz_addr": "1"},
                               "serial": {"device": "none", "baudrate": 9600, "parity": "none", "startBits": 8,
                                          "endBits": 1, "xonxoff": 0, "rtscts": 0, "ptz_protocol": "Sony Visca",
                                          "ptz_addr": "1"}}},
     {"ptz": {"enabled": True, "link": "network",
              "network": {"transport": "TCP", "host": "192.168.38.18", "port": 50381, "protocol": "Sony Visca",
                          "address": "1"},
              "serial": {"device": "none", "baudrate": 9600, "parity": "none", "data_bits": 8, "stop_bits": 1,
                         "xonxoff": False, "rtscts": False, "protocol": "Sony Visca", "address": "1"}}})
_tel("ptz-protocols", "/api/codec/ptz/getPtzProtocol",
     '{"result":"ok","msg":null,"data":["Sony Visca","Pelco-d","Pelco-p"]}',
     {"ptz": {"protocols": '["Sony Visca","Pelco-d","Pelco-p"]'}})
_tel("ptz-serial-devices", "/api/codec/ptz/getSerialDevice",
     '{"result":"ok","data":[{"device":"ttyUSB0","driver":"FTDI"}]}',
     {"ptz": {"serial_devices": '[{"device":"ttyUSB0","driver":"FTDI"}]'}})
_iface = {"status": "up", "address": "192.168.38.217", "netmask": "255.255.255.0", "gw": "192.168.38.254",
          "mac": "56:3D:69:9B:5A:C7", "method": "static", "enable": True, "dns": "223.5.5.5;224.5.5.5",
          "device": "eth0"}
_iface_state = {"link": "up", "address": "192.168.38.217", "netmask": "255.255.255.0", "gateway": "192.168.38.254",
                "mac": "56:3D:69:9B:5A:C7", "method": "static", "enabled": True, "dns": "223.5.5.5;224.5.5.5"}
_tel("network-interfaces", "/api/networkmanager/network/GetLinkinfo",
     {"result": "ok", "data": [_iface, {"status": "down", "address": "0.0.0.0", "netmask": "255.255.255.0", "gw": "",
                                        "mac": "44:07:6A:5F:D4:F7", "method": "dhcp", "enable": True,
                                        "dns": "223.5.5.5;224.5.5.5", "device": "eth1"}]},
     {"network": {"interfaces": {"eth0": _iface_state,
                                 "eth1": {"link": "down", "address": "0.0.0.0", "netmask": "255.255.255.0",
                                          "gateway": "", "mac": "44:07:6A:5F:D4:F7", "method": "dhcp",
                                          "enabled": True, "dns": "223.5.5.5;224.5.5.5"}}}},
     state_before={"network": {"interfaces": {"bond0": {"link": "up"}}}})
_tel("network-interface", "/api/networkmanager/network/GetEthernets?ifname=eth0", {"result": "ok", "data": _iface},
     {"network": {"interfaces": {"eth0": _iface_state}}})
_tel("bonding", "/api/networkmanager/network/GetBondingInfo",
     {"status": True, "desc": "", "method": "dhcp", "mac": "56:3D:69:9B:5A:C7", "address": "192.168.38.217",
      "netmask": "255.255.255.0", "gw": "192.168.38.254", "dns": ["223.5.5.5", "223.6.6.6"], "mode": "1",
      "eth0_status": "up", "eth1_status": "down"},
     {"network": {"bonding": {"enabled": True, "description": "", "method": "dhcp", "mac": "56:3D:69:9B:5A:C7",
                              "address": "192.168.38.217", "netmask": "255.255.255.0", "gateway": "192.168.38.254",
                              "dns": '["223.5.5.5","223.6.6.6"]', "mode": "active-backup", "eth0_link": "up",
                              "eth1_link": "down"}}})
_route = {"interface": "eth0", "target": "192.168.38.12", "netmask": "255.255.255.0", "gateway": "192.168.38.254",
          "mtu": 1500, "metric": 0}
_tel("static-routes", "/api/networkmanager/static/routing/get", _route,
     {"network": {"static_routes": _j(_route)}})
_tel("kilolink-interfaces", "/api/kilolink/networks", '{"result":"ok","list":["eth0","eth1"]}',
     {"kilolink": {"available_interfaces": '["eth0","eth1"]'}})
_tel("kilolink", "/api/kilolink/get",
     {"result": "ok", "cfg": {"ip": "192.168.41.211", "port": 50000, "ifname": ["eth0"], "key": "is8931ji",
                              "crypto": False, "enable": True}, "status": 1},
     {"kilolink": {"server": "192.168.41.211", "port": 50000, "interfaces": '["eth0"]', "crypto": False,
                   "enabled": True, "status": 1}})
_tel("talk-status", "/api/talk/queryLDeviceConnectStatus",
     '{"result":"ok","msg":{"connect_status":"connected","connect_ip":"192.168.40.100"}}',
     {"talk": {"connect_status": "connected", "connect_ip": "192.168.40.100"}})
_tel("talk-devices", "/api/talk/queryDeviceList",
     {"result": "ok", "msg": {
         "local_device": {"name": "N60-248", "ip": "192.168.43.248", "type": "device", "usb_status": False,
                          "talk_status": False, "connect_status": "noconnect", "connect_ip": "0.0.0.0",
                          "connect_type": None, "version": "1.10.0001", "http_port": 80, "https_port": 443},
         "other_device": [{"name": "E3-1234567891206", "ip": "192.168.43.141", "type": "device",
                           "discovery_mode": "auto", "selected": False, "connect_status": "noconnect",
                           "talk_status": "ready", "connect_error_info": None, "version": "1.10.0001",
                           "http_port": 80, "https_port": 443, "auth_code": None}]}},
     {"talk": {"local": {"name": "N60-248", "ip": "192.168.43.248", "type": "device", "usb_audio": False,
                         "talking": False, "connect_status": "noconnect", "connect_ip": "0.0.0.0",
                         "version": "1.10.0001", "http_port": 80, "https_port": 443},
               "devices": {"192.168.43.141": {"name": "E3-1234567891206", "type": "device", "discovery": "auto",
                                              "selected": False, "connect_status": "noconnect",
                                              "talk_status": "ready", "version": "1.10.0001", "http_port": 80,
                                              "https_port": 443}}}},
     state_before={"talk": {"devices": {"192.168.43.176": {"name": "Removed", "type": "device"}}}})
_tel("talk-version", "/api/talk/talkVersion", '{"result":"ok","msg":{"version":"1.10.0001"}}',
     {"talk": {"version": "1.10.0001"}})
_tel("talk-serial", "/api/talk/queryLocalDeviceSerial", '{"result":"ok","msg":{"serial":"012354640124"}}',
     {"talk": {"serial": "012354640124"}})
_tel("disks", "/api/record/storage/disk_list",
     {"result": "ok", "data": {
         "nas": [{"id": "public", "name": "public", "type": "nfs", "host": "192.168.0.21",
                  "volume": "/mnt/disk-data/public", "options": "", "mounted": "/run/media/nas-public",
                  "filesys": "nfs", "status": "mounted", "choose": False, "disk_type": "nas", "total": 100000,
                  "used": 1000, "free": 99000, "percent": 1}],
         "usb": [{"id": "sda1", "name": "sda1", "type": None, "host": None, "volume": "/dev/sda1", "options": None,
                  "mounted": "/run/media/sda1", "filesys": "vfat", "status": "mounted", "choose": True,
                  "disk_type": "usb", "total": 29494, "used": 9139, "free": 20354, "percent": 31}]}},
     {"storage": {"disks": {
         "sda1": {"kind": "usb", "name": "sda1", "volume": "/dev/sda1", "mount_point": "/run/media/sda1",
                  "file_system": "vfat", "status": "mounted", "chosen": True, "disk_type": "usb", "total_mb": 29494,
                  "used_mb": 9139, "free_mb": 20354, "used_percent": 31},
         "public": {"kind": "nas", "name": "public", "nas_type": "nfs", "host": "192.168.0.21",
                    "volume": "/mnt/disk-data/public", "mount_point": "/run/media/nas-public", "file_system": "nfs",
                    "status": "mounted", "chosen": False, "disk_type": "nas", "total_mb": 100000, "used_mb": 1000,
                    "free_mb": 99000, "used_percent": 1}}}},
     # The SD card was taken out: the list replaces the disks.
     state_before={"storage": {"disks": {"mmcblk0p1": {"kind": "usb", "name": "mmcblk0p1"}}}})
_tel("recorded-files", "/api/record/storage/get_disk_files?device=%2Frun%2Fmedia%2Fsda1&stream=ndi_hx",
     {"result": "ok", "data": [
         {"path": "/run/media/sda1/RECORD/M20230922_172235_00232/REC-NDI_HX-20230922172235002320.mov",
          "name": "REC-NDI_HX-20230922172235002320.mov", "size": 66.35, "current": True, "stream": "ndi_hx",
          "start_time": "2023-09-22 17:23:30", "end_time": "2023-09-22 17:24:28"}]},
     {"storage": {"files_device": "/run/media/sda1", "files": {"0": {
         "path": "/run/media/sda1/RECORD/M20230922_172235_00232/REC-NDI_HX-20230922172235002320.mov",
         "name": "REC-NDI_HX-20230922172235002320.mov", "size_mb": 66.35, "recording": True, "stream": "ndi_hx",
         "start_time": "2023-09-22 17:23:30", "end_time": "2023-09-22 17:24:28"}}}},
     state_before={"storage": {"files": {"1": {"name": "deleted.mov"}}}})
_tel("record-settings", "/api/record/record/get_record_info",
     {"result": "ok", "data": {"auto_record": True, "format": "mov", "limit_type": "size-loop", "limit_size": 4000000,
                               "limit_time": 3600, "disk_policy": "overwrite"}},
     {"recording": {"auto_record": True, "format": "mov", "limit_type": "size-loop", "limit_size_mb": 4000000,
                    "limit_time_s": 3600, "disk_policy": "overwrite"}})

# ── Telemetry: read-backs after a change ────────────────────────────────
_OKB = '{"result":"ok"}'
for name, path, body, sends in [
    ("readback-users", "/api/systemctrl/users/add", _OKB, [_get("/api/systemctrl/users/list")]),
    ("readback-hostname", "/api/systemctrl/system/setHostname?name=N60", _OKB,
     [_get("/api/systemctrl/system/getHostname")]),
    ("readback-web-protocols", "/api/systemctrl/system/setProtocol", _OKB,
     [_get("/api/systemctrl/system/getProtocol")]),
    ("readback-decode", "/api/codec/decode/addSpec", _OKB, [_get("/api/codec/decode/get")]),
    ("readback-discovery-server", "/api/codec/discovery/setDiscoveryServer", _OKB,
     [_get("/api/codec/discovery/getDiscoveryServer")]),
    ("readback-manual-sources", "/api/codec/discovery/addManualIpsGroups", _OKB,
     [_get("/api/codec/discovery/getManualIpsGroups"), _get("/api/codec/discovery/scan")]),
    ("readback-decoder-discovery-name", "/api/codec/discovery/setDiscoveryGroupChannel", "",
     [_get("/api/codec/discovery/getDiscoveryGroupChannel")]),
    ("readback-presets", "/api/codec/preset/add", _OKB, [_get("/api/codec/preset/get")]),
    ("readback-encode-streams", "/api/codec/encoders/choose_encode", _OKB,
     [_post("/api/codec/encoder/main/get_capture"), _post("/api/codec/encoder/main_full/get_capture"),
      _post("/api/codec/encoder/multi_protocol/get_capture"), _get("/api/codec/streams/main/ndi-hx/get"),
      _get("/api/codec/streams/main_full/ndi-full/get"), _get("/api/codec/streams/multi_protocol/srt/get")]),
    ("readback-encoder", "/api/codec/encoder/main_full/set", _OKB, [_post("/api/codec/encoder/main_full/get")]),
    ("readback-stream-config", "/api/codec/streams/main/ndi-hx/set", _OKB,
     [_get("/api/codec/streams/main/ndi-hx/get")]),
    ("readback-stream-reset", "/api/codec/streams/multi_protocol/reset", _OKB,
     [_get("/api/codec/streams/multi_protocol/srt/get")]),
    ("readback-ptz", "/api/codec/ptz/modify", '{"result":"ok","msg":null}', [_get("/api/codec/ptz/getPtzInfo")]),
    ("readback-network", "/api/networkmanager/network/SetEthernets", _OKB,
     [_get("/api/networkmanager/network/GetLinkinfo")]),
    ("readback-routes", "/api/networkmanager/static/routing/del", _OKB,
     [_get("/api/networkmanager/static/routing/get")]),
    ("readback-kilolink", "/api/kilolink/set", _OKB, [_get("/api/kilolink/get")]),
    ("readback-talk", "/api/talk/setLDLinkDEnable", _OKB,
     [_get("/api/talk/queryDeviceList"), _get("/api/talk/queryLDeviceConnectStatus")]),
    ("readback-nas", "/api/record/storage/remove_nas?id=public", _OKB, [_get("/api/record/storage/disk_list")]),
    ("readback-record-settings", "/api/record/record/settings", "",
     [_get("/api/record/record/get_record_info"), _get("/api/record/storage/disk_list")]),
]:
    _tel(name, path, body, {}, expect_then_send=sends)
# A refused change is not read again.
_tel("readback-refused", "/api/codec/preset/add", '{"result":"error","msg":"Invalid position"}', {},
     expect_then_send=[])

# With a password, the session logs in first (the engine's session tests
# cover the token in Authorization and in the Cookie and App headers).
V.append({"spec": "kiloview-n60", "command": "check_session", "input": {}, "file": "check_session-login",
          "settings": {"password": "Admin123"},
          "expect_request": {"method": "POST", "target": "/api/systemctrl/users/login",
                             "body": '{"username":"admin","password":"Admin123"}', "content_type": "application/json"}})
