# Kiloview D-series decoders (kiloview-decoder): one vector per command, an
# error reply, and telemetry vectors for every read and every read-back rule.
# Requests are as the Dxx Decoder API Documentation v2.2 gives them: paths
# under /api, JSON bodies written here by hand in the key order of the
# document's examples.
KD = "kiloview-decoder"


def _post(command, input, path, body, **extra):
    V.append({"spec": KD, "command": command, "input": input,
              "expect_request": {"method": "POST", "target": path, "body": body}, **extra})


def _get(command, input, target, **extra):
    V.append({"spec": KD, "command": command, "input": input,
              "expect_request": {"method": "GET", "target": target}, **extra})


def _t(name, path, body, state, request=None, **extra):
    inbound = {"path": path, "body": json.dumps(body)}
    if request is not None:
        inbound["request"] = request
    V.append({"spec": KD, "telemetry": name, "inbound_http": inbound, "expect_state": state, **extra})


OK = {"status": 200, "body": '{"result":"ok"}'}

# Users (section 6).
_post("login", {"password": "admin"}, "/api/users/login", '{"username":"admin","password":"admin"}',
      http_reply={"status": 200, "body": '{"result":"ok","data":{"username":"admin","token":"3515d2e8d3615fc68c84f00b361c5d8c"}}'},
      expect_result={"ok": {"kind": "value", "value": "3515d2e8d3615fc68c84f00b361c5d8c"}})
V.append({"spec": KD, "command": "logout", "input": {}, "settings": {"token": "abc123"},
          "expect_request": {"method": "POST", "target": "/api/users/logout", "body": '{"token":"abc123"}'},
          "http_reply": OK, "expect_result": {"ok": {"kind": "ack"}}})
_get("list_users", {}, "/api/users/list")
_post("add_user", {"username": "op", "password": "pw1", "alias": "Operator"}, "/api/users/add",
      '{"username":"op","password":"pw1","alias":"Operator","role":"user","phone":"","descript":""}')
_post("remove_user", {"id": 2}, "/api/users/remove", '{"id":2}')
_post("change_password", {"username": "admin", "password": "Admin123"}, "/api/users/modify",
      '{"username":"admin","password":"Admin123","passwordAgain":"Admin123"}')

# Info and system (sections 5, 13, 14).
_get("get_info", {}, "/api/info/get",
     http_reply={"status": 200, "body": '{"result":"ok","data":{"device_name":"D350"}}'},
     expect_result={"ok": {"kind": "value", "value": {"device_name": "D350"}}})
_get("get_device_name", {}, "/api/sys/device/get")
_post("set_device_name", {"name": "D350-Stage"}, "/api/sys/device/set", '{"device_name":"D350-Stage"}',
      http_reply={"status": 200, "body": '{"result":"error","msg":"500"}'},
      expect_result={"error": {"error": "device_error"}})
_get("reboot", {}, "/api/sys/reboot", http_reply=OK, expect_result={"ok": {"kind": "ack"}})
_get("reset", {}, "/api/sys/reset")
_get("restore_factory", {}, "/api/sys/restore")
_post("parse_url", {"url": "ndi://10.20.30.21:5961"}, "/api/sys/ip/get", '{"url":"ndi://10.20.30.21:5961"}')
_post("replace_url_address", {"url": "srt://192.168.1.1:13?x", "address": "192.168.1.124", "port": "1234"},
      "/api/sys/ip/replace", '{"url":"srt://192.168.1.1:13?x","target_ip":"192.168.1.124","target_port":"1234"}',
      http_reply={"status": 200, "body": '{"result":"ok","data":{"url":"srt://192.168.1.124:1234?x"}}'},
      expect_result={"ok": {"kind": "value", "value": "srt://192.168.1.124:1234?x"}})
# multipart/form-data with the file in the field upload (section 14); the
# boundary is the core's, derived from the parts (SPEC.md §4). The file is a
# made-up three bytes.
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


_FW_BODY, _FW_TYPE = _form([("upload", b"\x00\x01\x02", "fw.bin")])
V.append({"spec": KD, "command": "upgrade_firmware", "input": {"file": "AAEC", "filename": "fw.bin"},
          "expect_request": {"method": "POST", "target": "/api/firmware/upgrade", "body_hex": _FW_BODY.hex(),
                             "content_type": _FW_TYPE},
          "http_reply": OK, "expect_result": {"ok": {"kind": "ack"}}})

# Overlays (section 7).
_get("list_overlays", {"output": 1}, "/api/overlay/list?output_id=1")
_post("select_overlay", {"output": 2, "overlay": 1}, "/api/overlay/select", '{"overlay_id":1,"output_id":2}')

# Sources (section 8).
G = "05b8ba7a42f01af957d2e8483857962f"
S = "834fd00666b91015b23ee336bc2d605a"
_post("list_source_groups", {}, "/api/source/groups/list", '{"is_need_stream":true}')
_post("add_source_group", {"name": "Cameras"}, "/api/source/groups/add", '{"name":"Cameras"}')
_post("rename_source_group", {"group": G, "name": "Stage"}, "/api/source/groups/rename",
      '{"group_id":"%s","name":"Stage"}' % G)
_post("remove_source_group", {"group": G}, "/api/source/groups/remove", '{"group_id":"%s"}' % G)
_post("add_streams", {"body": {"group_id": G, "name": "udp", "Type": "udp", "url": "udp://239.1.1.1:5000"}},
      "/api/source/groups/streams/add",
      '{"group_id":"%s","name":"udp","Type":"udp","url":"udp://239.1.1.1:5000"}' % G)
_post("add_rtsp_stream", {"group": G, "name": "rtsp1", "url": "rtsp://192.168.36.222:554/ch01"},
      "/api/source/groups/streams/add",
      '{"group_id":"%s","name":"rtsp1","Type":"rtsp","url":"rtsp://192.168.36.222:554/ch01","buffer":"live:0:0:0:0:0",'
      '"trans_mode":"tcp","connect_speed":5000,"audio_sync_compst":0,"user":"","password":""}' % G)
_post("add_rtmp_stream", {"group": G, "type": "rtmps", "name": "cdn", "url": "rtmps://10.0.0.7:1935/live/vv",
                          "user": "u", "password": "p"},
      "/api/source/groups/streams/add",
      '{"group_id":"%s","name":"cdn","Type":"rtmps","url":"rtmps://10.0.0.7:1935/live/vv","buffer":"live:0:0:0:0:0",'
      '"connect_speed":5000,"audio_sync_compst":0,"user":"u","password":"p"}' % G)
_post("add_plain_stream", {"group": G, "type": "http", "name": "http", "url": "http://10.0.0.8/ch01",
                           "audio_sync": -40},
      "/api/source/groups/streams/add",
      '{"group_id":"%s","name":"http","Type":"http","url":"http://10.0.0.8/ch01","buffer":"live:0:0:0:0:0",'
      '"connect_speed":5000,"audio_sync_compst":-40}' % G)
_post("add_srt_stream", {"group": G, "name": "srt_caller", "url": "srt://192.168.36.59:1025",
                         "address": "192.168.36.59", "port": 1025, "encryption": "AES-128", "passphrase": "secretpass"},
      "/api/source/groups/streams/add",
      '{"group_id":"%s","name":"srt_caller","Type":"srt","url":"srt://192.168.36.59:1025","buffer":"live:0:0:0:0:0",'
      '"connection_mode":"Caller","address":"192.168.36.59","listener_port":1025,"latency":125,"connect_speed":5000,'
      '"audio_sync_compst":0,"show_advanced":"1","srt_id":"","encryption":"AES-128","passphrase":"secretpass",'
      '"bandwidth":25,"payload_size":1316}' % G)
_post("add_srt_listener_stream", {"group": G, "name": "srt_listener", "port": 1025, "latency": 200},
      "/api/source/groups/streams/add",
      '{"group_id":"%s","name":"srt_listener","Type":"srt","url":"srt://:1025","buffer":"live:0:0:0:0:0",'
      '"connection_mode":"Listener","listener_port":1025,"latency":200,"connect_speed":5000,"audio_sync_compst":0,'
      '"show_advanced":"1","srt_id":"","encryption":"none","passphrase":"","bandwidth":25,"payload_size":1316}' % G)
_post("add_rtmp_server_stream", {"group": G, "name": "rtmp server1", "url": "rtmp://192.168.36.59:1935/live/11111",
                                 "session": "11111", "security": "token", "token": "xxx"},
      "/api/source/groups/streams/add",
      '{"group_id":"%s","name":"rtmp server1","Type":"rtmp_server","url":"rtmp://192.168.36.59:1935/live/11111",'
      '"buffer":"live:0:0:0:0:0","application":"live","session":"11111","connect_speed":5000,"audio_sync_compst":0,'
      '"security":"token","token":"xxx","enable":1,"rtmp_server_port":1935}' % G)
_post("add_rtp_stream", {"group": G, "name": "rtp1", "sdp": "SDP:\nv=0\nm=video 1234 RTP/AVP 96"},
      "/api/source/groups/streams/add",
      '{"group_id":"%s","name":"rtp1","Type":"rtp","connect_speed":5000,"audio_sync_compst":0,'
      '"buffer":"live:200:200:500:0:0","sdp_content":"SDP:\\nv=0\\nm=video 1234 RTP/AVP 96"}' % G)
_post("add_ndi_stream", {"group": G, "name": "N60_238", "ndi_name": "N60_238 (HB)", "address": "192.168.36.238",
                         "port": 5965, "channel": "HB"},
      "/api/source/groups/streams/add",
      '{"group_id":"%s","Type":"ndi","data":[{"channel":"HB","group_name":"public","ip":"192.168.36.238",'
      '"name":"N60_238","ndi_name":"N60_238 (HB)","connect_speed":5000,"audio_sync_compst":0,"port":5965,'
      '"url":"192.168.36.238:5965"}]}' % G)
_post("modify_stream", {"body": {"stream_id": S, "Type": "udp", "name": "udp2"}},
      "/api/source/groups/streams/modify", '{"stream_id":"%s","Type":"udp","name":"udp2"}' % S)
_post("modify_rtsp_stream", {"stream": S, "name": "rtsp1", "url": "rtsp://10.0.0.9/ch01", "transport": "udp"},
      "/api/source/groups/streams/modify",
      '{"stream_id":"%s","name":"rtsp1","Type":"rtsp","url":"rtsp://10.0.0.9/ch01","buffer":"live:0:0:0:0:0",'
      '"trans_mode":"udp","connect_speed":5000,"audio_sync_compst":0,"user":"","password":""}' % S)
_post("modify_rtmp_stream", {"stream": S, "name": "rtmp1", "url": "rtmp://10.0.0.7:1935/live/vv"},
      "/api/source/groups/streams/modify",
      '{"stream_id":"%s","name":"rtmp1","Type":"rtmp","connect_speed":5000,"audio_sync_compst":0,'
      '"url":"rtmp://10.0.0.7:1935/live/vv","buffer":"live:0:0:0:0:0","user":"","password":""}' % S)
_post("modify_plain_stream", {"stream": S, "type": "udp", "name": "udp", "url": "udp://239.1.1.1:5000"},
      "/api/source/groups/streams/modify",
      '{"stream_id":"%s","name":"udp","Type":"udp","connect_speed":5000,"audio_sync_compst":0,'
      '"url":"udp://239.1.1.1:5000","buffer":"live:0:0:0:0:0"}' % S)
_post("modify_srt_stream", {"stream": S, "name": "srt3", "mode": "Rendezvous", "url": "srt://192.168.36.51:1025",
                            "address": "192.168.36.51", "port": 1025, "stream_id": "111"},
      "/api/source/groups/streams/modify",
      '{"stream_id":"%s","name":"srt3","Type":"srt","url":"srt://192.168.36.51:1025","buffer":"live:0:0:0:0:0",'
      '"connection_mode":"Rendezvous","address":"192.168.36.51","listener_port":1025,"latency":125,'
      '"show_advanced":"1","srt_id":"111","encryption":"none","passphrase":"","connect_speed":5000,'
      '"audio_sync_compst":0,"bandwidth":25,"payload_size":1316}' % S)
_post("modify_srt_listener_stream", {"stream": S, "name": "srt_listener", "port": 9000},
      "/api/source/groups/streams/modify",
      '{"stream_id":"%s","name":"srt_listener","Type":"srt","url":"srt://:9000","buffer":"live:0:0:0:0:0",'
      '"connection_mode":"Listener","listener_port":9000,"latency":125,"show_advanced":"1","srt_id":"",'
      '"encryption":"none","passphrase":"","bandwidth":25,"connect_speed":5000,"audio_sync_compst":0,'
      '"payload_size":1316}' % S)
_post("modify_rtmp_server_stream", {"stream": S, "name": "srv", "url": "rtmp://10.0.0.5:1936/app/s1",
                                    "application": "app", "session": "s1", "port": 1936},
      "/api/source/groups/streams/modify",
      '{"stream_id":"%s","name":"srv","Type":"rtmp_server","url":"rtmp://10.0.0.5:1936/app/s1",'
      '"buffer":"live:0:0:0:0:0","application":"app","session":"s1","security":"","token":"","connect_speed":5000,'
      '"audio_sync_compst":0,"enable":1,"rtmp_server_port":1936}' % S)
_post("modify_rtp_stream", {"stream": S, "name": "rtp1", "sdp": "SDP:\nv=0"},
      "/api/source/groups/streams/modify",
      '{"stream_id":"%s","name":"rtp1","Type":"rtp","connect_speed":5000,"audio_sync_compst":0,'
      '"buffer":"live:0:0:0:0:0","sdp_content":"SDP:\\nv=0"}' % S)
_post("modify_ndi_stream", {"stream": S, "name": "N6-21-1080P60", "ndi_name": "N6-21-1080P60 (Channel-NDI)",
                            "url": "192.168.36.155:5963", "transport": "multicast", "preview_mode": "Full"},
      "/api/source/groups/streams/modify",
      '{"stream_id":"%s","Type":"ndi","url":"192.168.36.155:5963","buffer":"live:1000:1000:1500:0:0",'
      '"trans_mode":"multicast","ndi_name":"N6-21-1080P60 (Channel-NDI)","connect_speed":5000,"audio_sync_compst":0,'
      '"name":"N6-21-1080P60","preview_mode":"Full"}' % S)
_post("remove_stream", {"group": G, "stream": S}, "/api/source/groups/streams/remove",
      '{"group_id":"%s","stream_id":"%s"}' % (G, S))
_post("copy_stream", {"from_group": G, "stream": S, "to_group": "354ee7bf"}, "/api/source/groups/streams/copy",
      '{"from_group_id":"%s","from_stream_id":"%s","to_group_id":"354ee7bf"}' % (G, S))
_post("move_stream", {"from_group": G, "stream": S, "to_group": "354ee7bf"}, "/api/source/groups/streams/move",
      '{"from_group_id":"%s","from_stream_id":"%s","to_group_id":"354ee7bf"}' % (G, S))

# Layouts (section 9).
_get("list_layout_templates", {}, "/api/layout/template/list")
_get("list_layouts", {}, "/api/layout/list")
_post("add_layout", {"template": 1, "name": "AddSingle2"}, "/api/layout/add", '{"template_id":1,"name":"AddSingle2"}')
_post("remove_layout", {"layout": 7}, "/api/layout/remove", '{"layout_id":7}')
_post("select_layout", {"output": 1, "layout": 2}, "/api/layout/select", '{"output_id":"1","layout_id":2}')
_post("modify_layout", {"output": 1, "layout": 6, "panes": [{"width": 30, "height": 30, "number": 2, "z_index": 1,
                                                            "id": 2, "x": 0, "y": 0}]},
      "/api/layout/modify",
      '{"layout_id":6,"position":[{"width":30,"height":30,"number":2,"z_index":1,"id":2,"x":0,"y":0}],"output_id":"1"}')
_post("modify_layout_pane", {"output": 1, "layout": 6, "pane": 2, "number": 2, "x": 0, "y": 0, "width": 30,
                             "height": 30, "z_index": 1},
      "/api/layout/modify",
      '{"layout_id":6,"position":[{"width":30,"height":30,"number":2,"z_index":1,"id":2,"x":0,"y":0}],"output_id":"1"}')
_post("save_layout", {"output": 1, "layout": 6}, "/api/layout/save", '{"output_id":"1","layout_id":6}')
_post("save_layout_as", {"output": 2, "layout": 6, "name": "Test"}, "/api/layout/resave",
      '{"output_id":"2","layout_id":6,"layout_name":"Test"}')
_post("reload_layout", {"output": 1, "layout": 6, "name": "5"}, "/api/layout/reload",
      '{"output_id":1,"layout_name":"5","layout_id":6}')
_post("rename_layout", {"layout": 1, "name": "Main"}, "/api/layout/rename", '{"layout_id":1,"name":"Main"}')

# Outputs (section 10).
_get("list_outputs", {}, "/api/output/list")
_get("get_output", {"output": 2}, "/api/output/get?output_id=2")
_post("get_output_interfaces", {"output": 1, "type": "audio"}, "/api/output/interfaces/get",
      '{"output_id":1,"Type":"audio"}')
_post("set_audio_interface", {"output": 1, "interface": 1, "label": "HDMI1", "enabled": True, "muted": True,
                              "volume": 14},
      "/api/output/interfaces/set",
      '{"channels":2,"enable":true,"id":1,"label":"HDMI1","mute":true,"samplerate":48000,"visible":true,'
      '"volume":14,"output_id":"1","Type":"audio","intf_id":1}')
_post("set_window_source", {"output": 1, "window": 2, "stream": S, "stream_name": "N60_238 (HX)",
                            "stream_url": "192.168.36.238:5964", "type": "ndi"},
      "/api/output/source/set",
      '{"from":{"Type":"source","stream_id":"%s","stream_name":"N60_238 (HX)","stream_url":"192.168.36.238:5964",'
      '"pos_id":"","output_id":"1","layout_id":""},"to":{"Type":"ndi","stream_id":"%s","stream_name":"N60_238 (HX)",'
      '"stream_url":"192.168.36.238:5964","pos_id":2,"output_id":"1","layout_id":""}}' % (S, S))
_post("set_window_source_raw", {"body": {"from": {"output_id": "1", "pos_id": 1},
                                         "to": {"output_id": "2", "pos_id": 2, "stream_id": ""}}},
      "/api/output/source/set", '{"from":{"output_id":"1","pos_id":1},"to":{"output_id":"2","pos_id":2,"stream_id":""}}')
_post("move_window_source", {"from_output": 1, "from_window": 1, "to_output": 2, "to_window": 2},
      "/api/output/source/set", '{"from":{"output_id":"1","pos_id":1},"to":{"output_id":"2","pos_id":2,"stream_id":""}}')
_post("clear_window", {"output": 1, "window": 1}, "/api/output/source/remove", '{"output_id":"1","pos_id":1}')
_get("list_resolutions", {}, "/api/output/resolution/list")
_post("set_resolution", {"output": 1, "resolution": 14}, "/api/output/resolution/set",
      '{"output_id":"1","res_id":14,"res_index":14}')
_get("get_audio_mix", {"output": 1}, "/api/output/audiomix/get?output_id=1")
_post("set_audio_mix", {"output": 1, "stream": S, "enabled": True}, "/api/output/audiomix/set",
      '{"output_id":"1","stream_id":"%s","enable":true,"volume":0,"mute":false,"Type":"output"}' % S)
_post("remove_audio_mix", {"output": 1, "stream": S, "list": "preview"}, "/api/output/audiomix/remove",
      '{"output_id":"1","stream_id":"%s","Type":"preview"}' % S)
_post("add_audio_mix", {"output": 1, "stream": S}, "/api/output/audiomix/add",
      '{"output_id":"1","stream_id":"%s"}' % S)
_post("get_audio_levels", {"output": 1}, "/api/output/audio/sound", '{"output_id":"1"}')
_post("set_border", {"output": 1, "enabled": True, "color": "#FF0000"}, "/api/output/border/set",
      '{"output_id":"1","enable":true,"color":"#FF0000"}')
_post("get_border", {"output": 1}, "/api/output/border/get", '{"output_id":1}')
_post("set_background", {"output": 2, "enabled": False, "color": "#000000"}, "/api/output/background/set",
      '{"output_id":"2","enable":false,"background":"#000000"}')
_get("get_background", {"output": 2}, "/api/output/background/get?output_id=2")
_post("check_audio", {"id": "1"}, "/api/output/audio/check", '{"id":"1"}',
      http_reply={"status": 200, "body": '{"data":true,"result":"ok"}'},
      expect_result={"ok": {"kind": "value", "value": True}})

# PTZ (section 10 (12)).
PTZ = '{"output_id":"1","pos_id":1,"Type":"%s","data":%s}'
_post("ptz_zoom", {"output": 1, "window": 1, "value": 0.5}, "/api/ptz/set", PTZ % ("ptz_zoom", '{"zoom_value":0.500}'))
_post("ptz_zoom_speed", {"output": 1, "window": 1, "speed": -1.0}, "/api/ptz/set",
      PTZ % ("ptz_zoom_speed", '{"zoom_speed":-1.000}'))
_post("ptz_pan_tilt", {"output": 1, "window": 1, "pan": -0.25, "tilt": 0.125}, "/api/ptz/set",
      PTZ % ("ptz_pan_tilt", '{"pan_value":-0.250,"tilt_value":0.125}'))
_post("ptz_pan_tilt_speed", {"output": 1, "window": 1, "pan_speed": 0.0, "tilt_speed": 0.0}, "/api/ptz/set",
      PTZ % ("ptz_pan_tilt_speed", '{"pan_speed":0.000,"tilt_speed":0.000}'))
_post("ptz_store_preset", {"output": 1, "window": 1, "preset": 3}, "/api/ptz/set",
      PTZ % ("ptz_store_preset", '{"preset_no":3}'))
_post("ptz_recall_preset", {"output": 1, "window": 1, "preset": 1}, "/api/ptz/set",
      PTZ % ("ptz_recall_preset", '{"preset_no":1,"speed":0.500}'))
_post("ptz_auto_focus", {"output": 1, "window": 1}, "/api/ptz/set", PTZ % ("ptz_auto_focus", "{}"))
_post("ptz_focus", {"output": 1, "window": 1, "value": 1.0}, "/api/ptz/set", PTZ % ("ptz_focus", '{"focus_value":1.000}'))
_post("ptz_focus_speed", {"output": 1, "window": 1, "speed": 0.5}, "/api/ptz/set",
      PTZ % ("ptz_focus_speed", '{"focus_speed":0.500}'))
_post("ptz_white_balance", {"output": 1, "window": 1, "mode": "oneshot"}, "/api/ptz/set",
      PTZ % ("ptz_white_balance_oneshot", "{}"))
_post("ptz_white_balance_manual", {"output": 1, "window": 1, "red": 0.25, "blue": 0.75}, "/api/ptz/set",
      PTZ % ("ptz_white_balance_manual", '{"red":0.250,"blue":0.750}'))
_post("ptz_exposure_auto", {"output": 1, "window": 1}, "/api/ptz/set", PTZ % ("ptz_exposure_auto", "{}"))
_post("ptz_exposure_manual", {"output": 1, "window": 1, "level": 0.6}, "/api/ptz/set",
      PTZ % ("ptz_exposure_manual", '{"exposure_level":0.600}'))
_post("ptz_is_supported", {"output": 1, "window": 1}, "/api/ptz/set", PTZ % ("ptz_is_supported", "{}"))

# Network (section 11).
_get("get_network", {}, "/api/network/get")
_post("set_network", {"interface": "eth0", "dhcp": False, "ip": "192.168.36.6", "mask": "255.255.255.0",
                      "gateway": "192.168.36.254", "mac": "18:C3:F4:25:66:66"},
      "/api/network/set",
      '{"device":"eth0","dynamic":"n","ip":"192.168.36.6","mask":"255.255.255.0","gw":"192.168.36.254","dns":"",'
      '"mac":"18:C3:F4:25:66:66"}')
_post("set_maintenance_address", {"interface": "eth0", "enabled": True, "ip": "192.168.1.168",
                                  "mask": "255.255.255.0"},
      "/api/network/modify", '{"device":"eth0","enable2":true,"ip2":"192.168.1.168","mask2":"255.255.255.0","gw2":""}')
_get("get_http_ports", {}, "/api/network/http/get")
_post("set_http_ports", {"http": True, "https": True, "https_port": 8443}, "/api/network/http/set",
      '{"http":true,"httpPort":80,"https":true,"httpsPort":8443}')
_post("ping", {"address": "192.168.36.124"}, "/api/network/ping", '{"ip":"192.168.36.124"}',
      http_reply={"status": 200, "body": '{"data":{"lossRate":0,"reachable":true},"result":"ok"}'},
      expect_result={"ok": {"kind": "value", "value": {"lossRate": 0, "reachable": True}}})
_get("list_routes", {}, "/api/network/routing/get")
_post("add_route", {"interface": "eth1", "ip": "192.168.1.1", "netmask": "255.255.255.255", "gateway": "192.168.36.6"},
      "/api/network/routing/add",
      '{"device":"eth1","ip":"192.168.1.1","netmask":"255.255.255.255","gw":"192.168.36.6","mtu":1500,"metric":0}')
_post("modify_route", {"id": "e762817b", "interface": "eth1", "ip": "192.168.1.2", "netmask": "255.255.255.255",
                       "gateway": "192.168.36.6", "metric": 5},
      "/api/network/routing/modify",
      '{"id":"e762817b","device":"eth1","ip":"192.168.1.2","netmask":"255.255.255.255","gw":"192.168.36.6",'
      '"mtu":1500,"metric":5}')
_post("set_route_enabled", {"id": "c26163a6", "enabled": True}, "/api/network/routing/set",
      '{"id":"c26163a6","status":1}')
_post("delete_route", {"id": "e762817b"}, "/api/network/routing/del", '{"ids":"e762817b"}')

# Time (section 12).
_get("get_time_settings", {}, "/api/timeregion/settings")
_post("set_time", {"mode": "ntp", "time": "2026-10-23 09:20:46", "offset": 1, "ntp_servers": "pool.ntp.org"},
      "/api/timeregion/time/set",
      '{"time":"2026-10-23 09:20:46","timeType":"ntp","offset":1,"ntp_servers":"pool.ntp.org"}')
_post("set_timezone", {"timezone": "Europe/London", "offset": 0}, "/api/timeregion/timezone/set",
      '{"timezone":"Europe/London","offset":0}')

# Preview (section 15).
_get("get_preview", {}, "/api/preview/get")
_post("add_preview_source", {"output": 1, "stream": S, "stream_name": "N60_238 (HX)",
                             "stream_url": "192.168.36.238:5964"},
      "/api/preview/source/modify",
      '{"from":{"Type":"source","stream_id":"%s","stream_name":"N60_238 (HX)","stream_url":"192.168.36.238:5964",'
      '"pos_id":"","output_id":"1","layout_id":""},"to":{"Type":"preview","stream_id":"%s",'
      '"stream_name":"N60_238 (HX)","stream_url":"192.168.36.238:5964","output_id":"1","layout_id":""}}' % (S, S))
_post("remove_preview_source", {"position": 1}, "/api/preview/source/remove", '{"id":1,"pos_id":1}')

# NDI discovery (section 16).
_get("get_ndi_discovery", {}, "/api/ndi/discovery/all")
_get("list_ndi_sources", {"method": "server", "group": "public", "server": "192.168.43.141"},
     "/api/ndi/discovery/sources?dis_method=4&dis_g_name=public&dis_s_ip=192.168.43.141")
_post("set_ndi_manual_discovery", {"entries": [{"dis_d_ip": "192.168.43.141", "dis_g_name": "public"}]},
      "/api/ndi/discovery/manual/add", '{"manuals":[{"dis_d_ip":"192.168.43.141","dis_g_name":"public"}]}')
_post("add_ndi_group_discovery", {"group": "studio"}, "/api/ndi/discovery/auto/add", '{"dis_g_name":"studio"}')
_post("remove_ndi_group_discovery", {"group": "studio"}, "/api/ndi/discovery/auto/del",
      '{"dis_d_ip":"","dis_g_name":"studio"}')
_post("add_ndi_discovery_server", {"server": "192.168.43.141"}, "/api/ndi/discovery/server/add",
      '{"dis_g_name":"public","dis_s_ip":"192.168.43.141"}')
_post("remove_ndi_discovery_server", {"server": "192.168.43.146"}, "/api/ndi/discovery/server/del",
      '{"dis_s_ip":"192.168.43.146","dis_g_name":"public"}')

# --- Telemetry: the reads ---
_t("info", "/api/info/get",
   {"result": "ok", "data": {"device_name": "D350-Stage", "firmware_version": "1.21.0010",
                             "hardware_version": "V1.0", "serial_number": "D3501598", "software_version": "2.0.0"}},
   {"device": {"name": "D350-Stage", "firmware_version": "1.21.0010", "hardware_version": "V1.0",
               "serial_number": "D3501598", "software_version": "2.0.0"}})
_t("device-name", "/api/sys/device/get", {"result": "ok", "data": {"device_name": "D350-211111"}},
   {"device": {"name": "D350-211111"}})
_t("output-list", "/api/output/list",
   {"result": "ok", "data": [{"background": "#000000", "bg_enable": False,
                              "border": {"bottom_width": 2, "color": "#FF0000", "enable": False, "hex_color": 16711680,
                                         "left_width": 2, "right_width": 2, "top_width": 2},
                              "id": "1", "layout_id": 2, "modified": False, "name": "output1", "res_id": "5",
                              "res_name": "1920x1080P60", "vumeter": True}]},
   {"outputs": {"1": {"name": "output1", "layout": 2, "layout_modified": False, "resolution_id": "5",
                      "resolution": "1920x1080P60", "vu_meter": True, "background": "#000000",
                      "background_enabled": False,
                      "border": {"enabled": False, "color": "#FF0000", "top_width": 2, "bottom_width": 2,
                                 "left_width": 2, "right_width": 2}}}})
_t("output-windows", "/api/output/get?output_id=1",
   {"result": "ok", "data": {"layout_id": 2, "name": "output1", "vumeter": True, "position": [
       {"enable": True, "height": 30, "id": 1, "mute": False, "number": 1, "resolution": "1920x1080p60", "status": 2,
        "stream_id": "e893d03a", "stream_ip": "192.168.36.238:5965", "stream_name": "N60_238 (HB)", "Type": "ndi",
        "video_bitrate": "55479.4", "video_format": "NDI | HB", "volume": 0, "vu_meter": True, "width": 30,
        "x": 0, "y": 0, "z_index": 0}]}},
   {"outputs": {"1": {"name": "output1", "layout": 2, "vu_meter": True, "windows": {"1": {
       "stream_id": "e893d03a", "stream_name": "N60_238 (HB)", "stream_address": "192.168.36.238:5965",
       "type": "ndi", "status": "connected", "enabled": True, "muted": False, "volume": 0, "vu_meter": True,
       "resolution": "1920x1080p60", "video_format": "NDI | HB", "video_bitrate": 55479.4, "number": 1,
       "x": 0.0, "y": 0.0, "width": 30.0, "height": 30.0, "z_index": 0}}}}},
   state_before={"outputs": {"1": {"windows": {"2": {"stream_id": "old"}}}}})
_t("video-interfaces", "/api/output/interfaces/get",
   {"result": "ok", "data": [{"colorspace": "RGB444", "enable": False, "id": 2, "label": "HDMI2", "mode": "HDMI",
                              "support": True, "visible": True}]},
   {"outputs": {"1": {"video_interfaces": {"2": {"label": "HDMI2", "mode": "HDMI", "colorspace": "RGB444",
                                                 "enabled": False, "supported": True, "visible": True}}}}},
   request={"output_id": 1, "Type": "video"})
_t("audio-interfaces", "/api/output/interfaces/get",
   {"result": "ok", "data": [{"id": 4, "label": "LineOut", "enable": True, "samplerate": 44.1, "channels": 2,
                              "volume": -3, "mute": False, "visible": True}]},
   {"outputs": {"2": {"audio_interfaces": {"4": {"label": "LineOut", "enabled": True, "sample_rate": 44.1,
                                                 "channels": 2, "volume": -3, "muted": False, "visible": True}}}}},
   request={"output_id": 2, "Type": "audio"})
_t("audio-mix", "/api/output/audiomix/get?output_id=1",
   {"result": "ok", "data": {"output": [{"stream_id": "a1", "name": "n60", "enable": True, "volume": -6,
                                         "mute": False, "pos_id": 1, "Type": "output"}],
                             "preview": [{"stream_id": "b2", "name": "n50", "enable": False, "volume": 0,
                                          "mute": True, "Type": "preview"}]}},
   {"outputs": {"1": {"audio_mix": {"output": {"a1": {"name": "n60", "enabled": True, "volume": -6, "muted": False,
                                                      "window": 1}},
                                    "preview": {"b2": {"name": "n50", "enabled": False, "volume": 0,
                                                       "muted": True}}}}}})
_t("audio-levels", "/api/output/audio/sound",
   {"result": "ok", "data": [{"dB": [-12, -14], "stream_id": "e893d03a"}]},
   {"outputs": {"1": {"audio_levels": {"0": {"stream_id": "e893d03a", "channels": {"1": -12.0, "2": -14.0}}}}}},
   request={"output_id": "1"})
_t("border", "/api/output/border/get",
   {"result": "ok", "data": {"bottom_width": 2, "color": "#00FF00", "enable": True, "hex_color": 65280,
                             "left_width": 3, "right_width": 3, "top_width": 2}},
   {"outputs": {"2": {"border": {"enabled": True, "color": "#00FF00", "top_width": 2, "bottom_width": 2,
                                 "left_width": 3, "right_width": 3}}}},
   request={"output_id": 2})
_t("background", "/api/output/background/get?output_id=1",
   {"result": "ok", "data": {"background": "#101010", "enable": True}},
   {"outputs": {"1": {"background": "#101010", "background_enabled": True}}})
_t("overlays", "/api/overlay/list?output_id=1",
   {"result": "ok", "data": [{"background": "transparent", "editing": True, "id": 1,
                              "img_path": "./data/decoder_config/overlay/preview/overlay_1.png", "modified": False,
                              "name": "template_1", "selected": True, "status": 0}]},
   {"outputs": {"1": {"overlays": {"1": {"name": "template_1", "selected": True, "status": 0,
                                         "background": "transparent", "editing": True, "modified": False,
                                         "image": "./data/decoder_config/overlay/preview/overlay_1.png"}}}}})
_t("source-groups", "/api/source/groups/list",
   {"result": "ok", "data": [{"id": "2ed1f715", "name": "NDI", "streams": [
       {"address": "192.168.36.238", "audio_sync_compst": 0, "buffer": "live:0:0:0:0:0", "connect_speed": 0,
        "id": "0f79952d", "name": "N60_238 (HX)", "ndi_name": "N60_238 (HX)", "preview_mode": "Auto", "status": 2,
        "trans_mode": "tcp", "Type": "ndi", "url": "192.168.36.238:5964", "user": "", "password": ""}]},
       {"id": "0093969758ac42c99245e89f9108ad08", "name": "Other", "streams": [
           {"address": "192.168.36.236", "bandwidth": 25, "connection_mode": "Caller", "encryption": "none",
            "id": "834fd006", "latency": 125, "listener_port": 1025, "name": "SRT_Caller", "payload_size": 1316,
            "srt_id": "", "status": 3, "Type": "srt", "url": "srt://192.168.36.236:1025", "passphrase": "x"}]}]},
   {"sources": {"groups": {"2ed1f715": {"name": "NDI"}, "0093969758ac42c99245e89f9108ad08": {"name": "Other"}},
                "streams": {"0f79952d": {"group": "2ed1f715", "name": "N60_238 (HX)", "type": "ndi",
                                         "url": "192.168.36.238:5964", "status": "connected",
                                         "ndi_name": "N60_238 (HX)", "address": "192.168.36.238",
                                         "buffer": "live:0:0:0:0:0", "transport": "tcp", "preview_mode": "Auto",
                                         "connect_speed": 0, "audio_sync": 0, "user": ""},
                            "834fd006": {"group": "0093969758ac42c99245e89f9108ad08", "name": "SRT_Caller", "type": "srt",
                                         "url": "srt://192.168.36.236:1025", "status": "failed",
                                         "address": "192.168.36.236", "srt_mode": "Caller", "srt_port": 1025,
                                         "srt_latency": 125, "srt_stream_id": "", "srt_encryption": "none",
                                         "srt_bandwidth": 25, "srt_payload_size": 1316}}}},
   state_before={"sources": {"streams": {"gone": {"name": "deleted"}}}})
_t("layouts", "/api/layout/list",
   {"result": "ok", "data": [{"layout_count": 2, "layout_id": 2, "name": "PIP", "used": True, "position": [
       {"height": 100, "id": 1, "number": 1, "stream_id": "", "width": 100, "x": 0, "y": 0, "z_index": 0},
       {"height": 30, "id": 2, "number": 2, "stream_id": "e893d03a", "width": 30, "x": 69, "y": 69, "z_index": 1}]}]},
   {"layouts": {"2": {"name": "PIP", "pane_count": 2, "used": True, "panes": {
       "1": {"stream_id": "", "number": 1, "x": 0.0, "y": 0.0, "width": 100.0, "height": 100.0, "z_index": 0},
       "2": {"stream_id": "e893d03a", "number": 2, "x": 69.0, "y": 69.0, "width": 30.0, "height": 30.0,
             "z_index": 1}}}}})
_t("layout-templates", "/api/layout/template/list",
   {"result": "ok", "count": 1, "data": [{"name": "3x3", "template_count": 9, "template_id": 4, "position": [
       {"height": 33.33, "id": 2, "number": 2, "stream_id": "", "width": 33.33, "x": 33.33, "y": 0, "z_index": 1}]}]},
   {"layout_templates": {"4": {"name": "3x3", "pane_count": 9, "panes": {
       "2": {"x": 33.33, "y": 0.0, "width": 33.33, "height": 33.33, "z_index": 1}}}}})
_t("resolutions", "/api/output/resolution/list",
   {"result": "ok", "data": [{"id": "5", "name": "1920x1080P60", "value": "1920x1080P60"},
                             {"id": "6", "name": "1920x1080P59.94(SDI only)", "value": "1920x1080P59.94(SDI only)"}]},
   {"resolutions": {"5": {"name": "1920x1080P60", "value": "1920x1080P60"},
                    "6": {"name": "1920x1080P59.94(SDI only)", "value": "1920x1080P59.94(SDI only)"}}})
_t("preview", "/api/preview/get",
   {"result": "ok", "data": {"position": [{"height": 144, "id": 1, "resolution": "1920x1080p60", "status": 1,
                                           "stream_id": "e893d03a", "stream_name": "N60_238 (HB)",
                                           "stream_url": "192.168.36.238:5965", "Type": "ndi", "width": 320,
                                           "x": 0, "y": 0}]}},
   {"preview": {"windows": {"1": {"stream_id": "e893d03a", "stream_name": "N60_238 (HB)",
                                  "stream_url": "192.168.36.238:5965", "type": "ndi", "status": "connecting",
                                  "resolution": "1920x1080p60", "x": 0, "y": 0, "width": 320, "height": 144}}}})
_t("network", "/api/network/get",
   {"result": "ok", "data": [{"device": "eth0", "dns": "", "dynamic": "n", "enable": 1, "enable2": True,
                              "gw": "192.168.36.254", "gw2": "", "ip": "192.168.36.6", "ip2": "192.168.1.163",
                              "mac": "18:C3:F4:25:66:66", "mask": "255.255.255.0", "mask2": "255.255.255.0",
                              "state": "up"}]},
   {"network": {"interfaces": {"eth0": {"enabled": True, "link": "up", "dhcp": False, "ip": "192.168.36.6",
                                        "mask": "255.255.255.0", "gateway": "192.168.36.254", "dns": "",
                                        "mac": "18:C3:F4:25:66:66",
                                        "maintenance": {"enabled": True, "ip": "192.168.1.163",
                                                        "mask": "255.255.255.0", "gateway": ""}}}}})
_t("http-ports", "/api/network/http/get",
   {"result": "ok", "data": {"http": True, "httpPort": 80, "https": False, "httpsPort": 443}},
   {"network": {"http": {"enabled": True, "port": 80}, "https": {"enabled": False, "port": 443}}})
_t("routes", "/api/network/routing/get",
   {"result": "ok", "data": [{"id": 1, "device": "eth0", "ip": "192.168.38.12", "netmask": "255.255.255.0",
                              "gw": "192.168.38.254", "mtu": 1500, "metric": 0, "status": 1}]},
   {"network": {"routes": {"1": {"interface": "eth0", "ip": "192.168.38.12", "netmask": "255.255.255.0",
                                 "gateway": "192.168.38.254", "mtu": 1500, "metric": 0, "enabled": True}}}})
_t("time", "/api/timeregion/settings",
   {"result": "ok", "data": {"ntp_servers": "ntp1.example.org", "offset": 8, "time": "2024-09-09 10:40:46",
                             "timeType": "pc", "timezone": "Asia/Shanghai"}},
   {"time": {"ntp_servers": "ntp1.example.org", "offset": 8, "current": "2024-09-09 10:40:46", "source": "pc",
             "timezone": "Asia/Shanghai"}})
_t("users", "/api/users/list",
   {"count": 1, "list": [{"alias": "admin", "create_time": "1715070431000", "descript": "Initial", "id": 1,
                          "phone": "", "role": "admin", "token": "caf27900", "username": "admin"}]},
   {"users": {"1": {"username": "admin", "alias": "admin", "role": "admin", "phone": "", "description": "Initial",
                    "created": "1715070431000"}}})
_t("ndi-discovery", "/api/ndi/discovery/all",
   {"result": "ok", "data": {"auto": [{"dis_d_ip": "", "dis_g_name": "123", "dis_method": 3, "dis_s_ip": ""}],
                             "discovery_server": [{"dis_d_ip": "", "dis_g_name": "public", "dis_method": 4,
                                                   "dis_s_ip": "192.168.43.141"}],
                             "manual": [{"dis_d_ip": "192.168.35.178", "dis_g_name": "mhb", "dis_method": 2,
                                         "dis_s_ip": ""}],
                             "public": [{"dis_d_ip": "", "dis_g_name": "public", "dis_method": 1, "dis_s_ip": ""}]}},
   {"ndi_discovery": {
       "auto": {"0": {"group": "123", "address": "", "server": "", "method": 3}},
       "discovery_server": {"0": {"group": "public", "address": "", "server": "192.168.43.141", "method": 4}},
       "manual": {"0": {"group": "mhb", "address": "192.168.35.178", "server": "", "method": 2}},
       "public": {"0": {"group": "public", "address": "", "server": "", "method": 1}}}},
   state_before={"ndi_discovery": {"manual": {"1": {"group": "old"}}}})
_t("ndi-sources", "/api/ndi/discovery/sources?dis_method=1&dis_g_name=&dis_s_ip=",
   {"result": "ok", "data": [{"channel": "FULL", "group_name": "public", "ip": "192.168.35.178",
                              "name": "N60-N6020240531", "ndi_name": "N60.lab (FULL)", "port": 5961,
                              "url": "192.168.35.178:5961"}]},
   {"ndi": {"sources": {"N60.lab (FULL)": {"name": "N60-N6020240531", "url": "192.168.35.178:5961",
                                           "ip": "192.168.35.178", "port": 5961, "channel": "FULL",
                                           "group": "public"}}}})
_t("error-reply-ignored", "/api/info/get", {"result": "error", "msg": "2"}, {})

# --- Telemetry: read-back after writes ---
OKB = {"result": "ok"}
O1 = {"method": "GET", "target": "/api/output/get?output_id=1"}
O2 = {"method": "GET", "target": "/api/output/get?output_id=2"}
GROUPS = {"method": "POST", "target": "/api/source/groups/list", "body": '{"is_need_stream":true}'}
for name, path, sends, request in [
    ("readback-sources", "/api/source/groups/streams/modify", [GROUPS], None),
    ("readback-layout", "/api/layout/select",
     [{"method": "GET", "target": "/api/layout/list"}, {"method": "GET", "target": "/api/output/list"}, O1, O2], None),
    ("readback-window", "/api/output/source/set", [O1, O2, {"method": "GET", "target": "/api/layout/list"}], None),
    ("readback-resolution", "/api/output/resolution/set", [{"method": "GET", "target": "/api/output/list"}], None),
    ("readback-interface", "/api/output/interfaces/set",
     [{"method": "POST", "target": "/api/output/interfaces/get", "body": '{"output_id":2,"Type":"audio"}'}],
     {"output_id": "2", "Type": "audio", "intf_id": 1}),
    ("readback-audio-mix", "/api/output/audiomix/set",
     [{"method": "GET", "target": "/api/output/audiomix/get?output_id=1"}], {"output_id": "1", "stream_id": "a1"}),
    ("readback-border", "/api/output/border/set",
     [{"method": "POST", "target": "/api/output/border/get", "body": '{"output_id":2}'}], {"output_id": "2"}),
    ("readback-background", "/api/output/background/set",
     [{"method": "GET", "target": "/api/output/background/get?output_id=1"}], {"output_id": "1"}),
    ("readback-overlay", "/api/overlay/select",
     [{"method": "GET", "target": "/api/overlay/list?output_id=2"}], {"overlay_id": 1, "output_id": 2}),
    ("readback-preview", "/api/preview/source/remove",
     [{"method": "GET", "target": "/api/preview/get"},
      {"method": "GET", "target": "/api/output/audiomix/get?output_id=1"},
      {"method": "GET", "target": "/api/output/audiomix/get?output_id=2"}], None),
    ("readback-network", "/api/network/set", [{"method": "GET", "target": "/api/network/get"}], None),
    ("readback-routes", "/api/network/routing/del", [{"method": "GET", "target": "/api/network/routing/get"}], None),
    ("readback-http", "/api/network/http/set", [{"method": "GET", "target": "/api/network/http/get"}], None),
    ("readback-time", "/api/timeregion/timezone/set", [{"method": "GET", "target": "/api/timeregion/settings"}], None),
    ("readback-device-name", "/api/sys/device/set", [{"method": "GET", "target": "/api/sys/device/get"}], None),
    ("readback-users", "/api/users/add", [{"method": "GET", "target": "/api/users/list"}], None),
    ("readback-ndi-discovery", "/api/ndi/discovery/server/add",
     [{"method": "GET", "target": "/api/ndi/discovery/all"},
      {"method": "GET", "target": "/api/ndi/discovery/sources?dis_method=1&dis_g_name=&dis_s_ip="}], None),
]:
    _t(name, path, OKB, {}, request=request, expect_then_send=sends)
_t("readback-not-on-error", "/api/layout/select", {"result": "error", "msg": "104"}, {}, expect_then_send=[])
