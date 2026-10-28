# Kiloview MG300 V2 gateway and decoder (kiloview-mg300): one vector per
# command, an error reply, and telemetry vectors for every read and every
# read-back rule. Requests are as the MG300V2 API document V1.0.0 gives
# them: paths under /api, JSON bodies written here by hand in the key order
# of the document's examples. Every module-level name carries the _MG_ /
# _mg_ prefix (this file shares its namespace with every other generator).
_MG_ = "kiloview-mg300"


def _mg(command, input, method, target, body=None, **extra):
    request = {"method": method, "target": target}
    if body is not None:
        request["body"] = body
    V.append({"spec": _MG_, "command": command, "input": input, "expect_request": request, **extra})


def _mg_post(command, input, target, body, **extra):
    _mg(command, input, "POST", target, body, **extra)


def _mg_get(command, input, target, body=None, **extra):
    _mg(command, input, "GET", target, body, **extra)


def _mg_t(name, path, body, state, request=None, **extra):
    inbound = {"path": path, "body": json.dumps(body)}
    if request is not None:
        inbound["request"] = request
    V.append({"spec": _MG_, "telemetry": name, "inbound_http": inbound, "expect_state": state, **extra})


def _mg_upload(command, input, target, parts, **extra):
    body, ctype = _ke3_form(parts)
    V.append({"spec": _MG_, "command": command, "input": input,
              "expect_request": {"method": "POST", "target": target, "body_hex": body.hex(), "content_type": ctype},
              **extra})


_MG_OK = {"status": 200, "body": '{"result":"ok"}'}
_MG_G = "05b8ba7a42f01af957d2e8483857962f"
_MG_S = "834fd00666b91015b23ee336bc2d605a"
_MG_GW = "82c83493f98abf452b7b6edccca98a52"

# ── Users and authentication (1.1, 3) ────────────────────────────────────
_mg_post("login", {"password": "admin"}, "/api/users/login", '{"username":"admin","password":"admin"}',
         http_reply={"status": 200, "body": '{"data":{"alias":"admin","changed":true,"role":"admin","role_id":1,'
                                            '"token":"901fcecb0640576e2002e52e32eb8022","user_id":1,'
                                            '"username":"admin"},"result":"ok"}'},
         expect_result={"ok": {"kind": "value", "value": "901fcecb0640576e2002e52e32eb8022"}})
_mg_post("login", {"username": "admin", "password": "wrong"}, "/api/users/login",
         '{"username":"admin","password":"wrong"}',
         http_reply={"status": 200, "body": '{"result":"error","msg":"302"}'},
         expect_result={"error": {"error": "device_error"}}, file="login-refused")
V.append({"spec": _MG_, "command": "logout", "input": {},
          "expect_request": {"method": "POST", "target": "/api/users/logout"},
          "http_reply": {"status": 200, "body": '{"result":"ok","data":{}}'}, "expect_result": {"ok": {"kind": "ack"}}})
_mg_get("list_users", {}, "/api/users/list")
_mg_get("search_users", {"search": "gen role"}, "/api/users/list?search=gen%20role")
_mg_get("get_user", {"username": "admin1"}, "/api/users/info", '{"username":"admin1"}')
_mg_post("add_user", {"username": "gen1", "password": "gen1", "alias": "test1", "role": 3}, "/api/users/add",
         '{"username":"gen1","password":"gen1","alias":"test1","role_id":3,"phone":"","descript":""}')
_mg_post("remove_user", {"id": 3}, "/api/users/remove", '{"id":3}')
_mg_post("modify_user", {"id": 3, "username": "gen1", "alias": "gen1", "role": 3, "phone": "124666",
                         "description": "Stage"},
         "/api/users/modify",
         '{"id":3,"username":"gen1","alias":"gen1","role_id":3,"phone":"124666","descript":"Stage"}')
_mg_post("set_user_password", {"id": 3, "password": "Admin123"}, "/api/users/modify", '{"id":3,"password":"Admin123"}')
_mg_get("list_roles", {}, "/api/roles/permissions")
_mg_post("set_role_permissions", {"role": 3, "permissions": [2, 3, 4, 5, 6, 7, 8]}, "/api/roles/permupdate",
         '{"role_id":3,"permission_ids":[2,3,4,5,6,7,8]}')
_mg_post("set_role_info", {"role": 3, "name": "gen_role_1", "description": "Operators"}, "/api/roles/permupdate",
         '{"role_id":3,"name":"gen_role_1","description":"Operators"}')
_mg("get_auth_settings", {}, "POST", "/api/users/auth_info_get",
    http_reply={"status": 200, "body": '{"result":"ok","data":{"enable":true,"canModify":true}}'},
    expect_result={"ok": {"kind": "value", "value": {"enable": True, "canModify": True}}})
_mg_post("set_auth_enabled", {"enabled": False}, "/api/users/auth_info_set", '{"enable":false}')
_mg("check_session", {}, "POST", "/api/users/session/check")

# ── Info (2) ─────────────────────────────────────────────────────────────
_mg_get("get_info", {}, "/api/info/get",
        http_reply={"status": 200, "body": '{"result":"ok","data":{"device_name":"MG300"}}'},
        expect_result={"ok": {"kind": "value", "value": {"device_name": "MG300"}}})

# ── Overlays (4) ─────────────────────────────────────────────────────────
_mg_get("list_overlays", {}, "/api/overlay/list")
_mg("add_overlay", {}, "POST", "/api/overlay/add", http_reply=_MG_OK, expect_result={"ok": {"kind": "ack"}})
_mg_post("remove_overlay", {"overlay": 1}, "/api/overlay/remove", '{"overlay_id":1}')
_mg_post("copy_overlay", {"overlay": 1}, "/api/overlay/copy", '{"overlay_id":1}')
_mg_post("rename_overlay", {"overlay": 1, "name": "Logo"}, "/api/overlay/rename", '{"id":1,"name":"Logo"}')
_mg_post("save_overlay", {"overlay": 1}, "/api/overlay/save", '{"overlay_id":1}')
_mg_post("cancel_overlay_changes", {"overlay": 1}, "/api/overlay/cancel", '{"overlay_id":1}')
_mg_post("select_overlay", {"overlay": 1, "output": 2}, "/api/overlay/select", '{"overlay_id":1,"output_id":2}')
_mg_get("get_overlay_items", {"overlay": 1}, "/api/overlay/items/get?overlay_id=1")
_mg_post("set_overlay_items", {"overlay": 1, "mode": "z_index", "items": [
    {"id": 1, "img_id": 1, "width": 1000, "height": 500, "x": 10, "y": 0, "z_index": 0, "type": "image",
     "opacity": 1.0}]},
         "/api/overlay/items/set",
         '{"overlay_id":1,"mode":"z_index","list":[{"id":1,"img_id":1,"width":1000,"height":500,"x":10,"y":0,'
         '"z_index":0,"type":"image","opacity":1.0}]}')
_mg_post("add_overlay_item", {"overlay": 1, "type": "text"}, "/api/overlay/items/add", '{"overlay_id":1,"type":"text"}')
_mg_post("remove_overlay_item", {"overlay": 1, "item": 2}, "/api/overlay/items/remove", '{"overlay_id":1,"item_id":2}')
_mg_get("list_overlay_images", {}, "/api/overlay/images/list")
_mg_post("remove_overlay_image", {"image": 2}, "/api/overlay/images/remove", '{"img_id":2}')
# A made-up three-byte file in the field img_url (4.15).
_mg_upload("upload_overlay_image", {"file": "AAEC", "filename": "logo.png"}, "/api/overlay/images/add",
           [("img_url", b"\x00\x01\x02", "logo.png")], http_reply=_MG_OK, expect_result={"ok": {"kind": "ack"}})
_mg_get("get_overlay_editing", {"overlay": 1}, "/api/overlay/editing/get", '{"overlay_id":1}')
_mg_post("select_overlay_editing", {"overlay": 1}, "/api/overlay/editing/select", '{"overlay_id":1}')
_mg_get("get_overlay_preview", {"output": 1}, "/api/overlay/preview/get", '{"output_id":1}')
_mg_get("list_overlay_datetime_formats", {}, "/api/overlay/datetime/format/list")
_mg_get("list_overlay_fonts", {}, "/api/overlay/font/list")

# ── Sources (5) ──────────────────────────────────────────────────────────
_mg_get("list_source_groups", {}, "/api/source/groups/list", '{"is_need_stream":true}')
_mg_post("add_source_group", {"name": "group1"}, "/api/source/groups/add", '{"name":"group1"}')
_mg_post("rename_source_group", {"group": _MG_G, "name": "Stage"}, "/api/source/groups/rename",
         '{"group_id":"%s","name":"Stage"}' % _MG_G)
_mg_post("remove_source_group", {"group": _MG_G}, "/api/source/groups/remove", '{"group_id":"%s"}' % _MG_G)
_mg_post("reorder_source_groups", {"order": [{"group_id": "2"}, {"group_id": "1"}]}, "/api/source/groups/reorder",
         '{"data":[{"group_id":"2"},{"group_id":"1"}]}')
_mg_post("add_stream", {"body": {"group_id": _MG_G, "name": "udp", "type": "udp", "url": "udp://239.1.1.1:5000"}},
         "/api/source/groups/streams/add",
         '{"group_id":"%s","name":"udp","type":"udp","url":"udp://239.1.1.1:5000"}' % _MG_G)
_mg_post("add_rtsp_stream", {"group": _MG_G, "name": "rtsp1", "url": "rtsp://192.168.36.222:554/ch01"},
         "/api/source/groups/streams/add",
         '{"group_id":"%s","name":"rtsp1","type":"rtsp","url":"rtsp://192.168.36.222:554/ch01",'
         '"buffer":"live:0:0:0:0:0","trans_mode":"tcp","connect_speed":5000,"audio_sync_compst":0,'
         '"user":"","password":""}' % _MG_G)
_mg_post("add_rtmp_stream", {"group": _MG_G, "type": "rtmps", "name": "rtmps", "url": "rtmps://192.168.0.73:1935/live/vv"},
         "/api/source/groups/streams/add",
         '{"group_id":"%s","name":"rtmps","type":"rtmps","url":"rtmps://192.168.0.73:1935/live/vv",'
         '"buffer":"live:0:0:0:0:0","connect_speed":5000,"audio_sync_compst":0,"user":"","password":""}' % _MG_G)
_mg_post("add_plain_stream", {"group": _MG_G, "type": "https", "name": "https", "url": "https://192.168.36.222:554/ch01"},
         "/api/source/groups/streams/add",
         '{"group_id":"%s","name":"https","type":"https","connect_speed":5000,"audio_sync_compst":0,'
         '"url":"https://192.168.36.222:554/ch01","buffer":"live:0:0:0:0:0"}' % _MG_G)
_mg_post("add_srt_stream", {"group": _MG_G, "name": "srt3", "mode": "Rendezvous", "url": "srt://192.168.36.51:1025",
                            "address": "192.168.36.51", "port": 1025, "stream_id": "111"},
         "/api/source/groups/streams/add",
         '{"group_id":"%s","name":"srt3","type":"srt","url":"srt://192.168.36.51:1025","buffer":"live:0:0:0:0:0",'
         '"connection_mode":"Rendezvous","address":"192.168.36.51","listener_port":1025,"latency":125,'
         '"connect_speed":5000,"audio_sync_compst":0,"show_advanced":"1","srt_id":"111","encryption":"none",'
         '"passphrase":"","bandwidth":25,"payload_size":1316}' % _MG_G)
_mg_post("add_srt_listener_stream", {"group": _MG_G, "name": "srt_listener", "port": 1025, "stream_id": "111"},
         "/api/source/groups/streams/add",
         '{"group_id":"%s","name":"srt_listener","type":"srt","url":"srt://:1025","buffer":"live:0:0:0:0:0",'
         '"connection_mode":"Listener","listener_port":1025,"latency":125,"connect_speed":5000,'
         '"audio_sync_compst":0,"show_advanced":"1","srt_id":"111","encryption":"none","passphrase":"",'
         '"bandwidth":25,"payload_size":1316}' % _MG_G)
_mg_post("add_rtmp_server_stream", {"group": _MG_G, "name": "rtmp server1",
                                    "url": "rtmp://192.168.36.59:1935/live/xxx/11111", "session": "11111",
                                    "security": "token", "token": "xxx"},
         "/api/source/groups/streams/add",
         '{"group_id":"%s","name":"rtmp server1","type":"rtmp_server","url":"rtmp://192.168.36.59:1935/live/xxx/11111",'
         '"buffer":"live:0:0:0:0:0","application":"live","session":"11111","connect_speed":5000,'
         '"audio_sync_compst":0,"security":"token","token":"xxx","enable":1,"rtmp_server_port":1935}' % _MG_G)
_mg_post("add_rtp_stream", {"group": _MG_G, "name": "rtp_no_audio", "sdp": "SDP:\nv=0\nm=video 1234 RTP/AVP 96"},
         "/api/source/groups/streams/add",
         '{"group_id":"%s","name":"rtp_no_audio","type":"rtp","connect_speed":5000,"audio_sync_compst":0,'
         '"buffer":"live:200:200:500:0:0","sdp_content":"SDP:\\nv=0\\nm=video 1234 RTP/AVP 96"}' % _MG_G)
_mg_post("add_zixi_stream", {"group": _MG_G, "name": "zixi", "url": "zixi://zixi.example.com:2077/clocks",
                             "provider_id": "clocks"},
         "/api/source/groups/streams/add",
         '{"group_id":"%s","name":"zixi","type":"zixi","url":"zixi://zixi.example.com:2077/clocks","stream_id":"",'
         '"buffer":"live:200:200:500:0:0","connect_speed":5000,"audio_sync_compst":0,"zixi_udp_out_port":2077,'
         '"zixi_max_latency":500,"encryption":"none","passphrase":"","zixi_stream_provider_id":"clocks",'
         '"zixi_back_url":"","zixi_fec_overhead":30,"zixi_fec_block_ms":50,"user":"","password":"",'
         '"newUrl":"zixi://zixi.example.com:2077/clocks"}' % _MG_G)
_mg_post("add_ndi_stream", {"group": _MG_G, "name": "N60_238 ", "ndi_name": "N60_238 (HB)", "address": "192.168.36.238",
                            "port": 5965, "channel": "HB"},
         "/api/source/groups/streams/add",
         '{"group_id":"%s","type":"ndi","data":[{"channel":"HB","group_name":"public","ip":"192.168.36.238",'
         '"name":"N60_238 ","ndi_name":"N60_238 (HB)","connect_speed":5000,"audio_sync_compst":0,"port":5965,'
         '"url":"192.168.36.238:5965"}]}' % _MG_G)
_mg_post("modify_stream", {"body": {"stream_id": _MG_S, "type": "udp", "name": "udp2"}},
         "/api/source/groups/streams/modify", '{"stream_id":"%s","type":"udp","name":"udp2"}' % _MG_S)
_mg_post("modify_rtsp_stream", {"stream": _MG_S, "name": "rtsp1", "url": "rtsp://192.168.36.222:554/ch01",
                                "transport": "udp", "user": "u", "password": "p"},
         "/api/source/groups/streams/modify",
         '{"stream_id":"%s","name":"rtsp1","type":"rtsp","url":"rtsp://192.168.36.222:554/ch01",'
         '"buffer":"live:0:0:0:0:0","trans_mode":"udp","connect_speed":5000,"audio_sync_compst":0,'
         '"user":"u","password":"p"}' % _MG_S)
_mg_post("modify_rtmp_stream", {"stream": _MG_S, "name": "rtmp1", "url": "rtmp://192.168.0.73:1935/live/vv"},
         "/api/source/groups/streams/modify",
         '{"stream_id":"%s","name":"rtmp1","type":"rtmp","connect_speed":5000,"audio_sync_compst":0,'
         '"url":"rtmp://192.168.0.73:1935/live/vv","buffer":"live:0:0:0:0:0","user":"","password":""}' % _MG_S)
_mg_post("modify_plain_stream", {"stream": _MG_S, "type": "udp", "name": "udp", "url": "udp://192.168.36.222:554/ch01"},
         "/api/source/groups/streams/modify",
         '{"stream_id":"%s","name":"udp","type":"udp","connect_speed":5000,"audio_sync_compst":0,'
         '"url":"udp://192.168.36.222:554/ch01","buffer":"live:0:0:0:0:0"}' % _MG_S)
_mg_post("modify_srt_stream", {"stream": _MG_S, "name": "srt_caller", "url": "srt://192.168.36.59:1025",
                               "address": "192.168.36.51", "port": 1025, "stream_id": "111", "audio_sync": -20},
         "/api/source/groups/streams/modify",
         '{"stream_id":"%s","name":"srt_caller","type":"srt","url":"srt://192.168.36.59:1025",'
         '"buffer":"live:0:0:0:0:0","connection_mode":"Caller","address":"192.168.36.51","listener_port":1025,'
         '"latency":125,"show_advanced":"1","srt_id":"111","encryption":"none","passphrase":"",'
         '"connect_speed":5000,"audio_sync_compst":-20,"bandwidth":25,"payload_size":1316}' % _MG_S)
_mg_post("modify_srt_listener_stream", {"stream": _MG_S, "name": "srt_listener", "port": 1025, "encryption": "AES-256",
                                        "passphrase": "secretpass1"},
         "/api/source/groups/streams/modify",
         '{"stream_id":"%s","name":"srt_listener","type":"srt","url":"srt://:1025","buffer":"live:0:0:0:0:0",'
         '"connection_mode":"Listener","listener_port":1025,"latency":125,"show_advanced":"1","srt_id":"",'
         '"encryption":"AES-256","passphrase":"secretpass1","bandwidth":25,"connect_speed":5000,'
         '"audio_sync_compst":0,"payload_size":1316}' % _MG_S)
_mg_post("modify_rtmp_server_stream", {"stream": _MG_S, "name": "rtmp server1",
                                       "url": "rtmp://192.168.36.59:1935/live/xxx/11111", "session": "11111"},
         "/api/source/groups/streams/modify",
         '{"stream_id":"%s","name":"rtmp server1","type":"rtmp_server","url":"rtmp://192.168.36.59:1935/live/xxx/11111",'
         '"buffer":"live:0:0:0:0:0","application":"live","session":"11111","security":"none","token":"",'
         '"connect_speed":5000,"audio_sync_compst":0,"enable":1,"rtmp_server_port":1935}' % _MG_S)
_mg_post("modify_rtp_stream", {"stream": _MG_S, "name": "rtp1", "sdp": "SDP:\nv=0"},
         "/api/source/groups/streams/modify",
         '{"stream_id":"%s","name":"rtp1","type":"rtp","connect_speed":5000,"audio_sync_compst":0,'
         '"buffer":"live:0:0:0:0:0","sdp_content":"SDP:\\nv=0"}' % _MG_S)
_mg_post("modify_zixi_stream", {"stream": _MG_S, "name": "zixi", "url": "zixi://zixi.example.com:2077/clocks"},
         "/api/source/groups/streams/modify",
         '{"stream_id":"%s","name":"zixi","type":"zixi","url":"zixi://zixi.example.com:2077/clocks",'
         '"buffer":"live:200:200:500:0:0","connect_speed":5000,"audio_sync_compst":0,"encryption":"none",'
         '"passphrase":"","user":"","password":""}' % _MG_S)
_mg_post("modify_ndi_stream", {"stream": _MG_S, "name": "N6-21-1080P60", "ndi_name": "N6-21-1080P60 (Channel-NDI)",
                               "url": "192.168.36.155:5963", "transport": "multicast", "preview_mode": "Full"},
         "/api/source/groups/streams/modify",
         '{"stream_id":"%s","type":"ndi","url":"192.168.36.155:5963","buffer":"live:1000:1000:1500:0:0",'
         '"trans_mode":"multicast","ndi_name":"N6-21-1080P60 (Channel-NDI)","connect_speed":5000,'
         '"audio_sync_compst":0,"name":"N6-21-1080P60","preview_mode":"Full"}' % _MG_S)
_mg_post("remove_stream", {"group": _MG_G, "stream": _MG_S}, "/api/source/groups/streams/remove",
         '{"group_id":"%s","stream_id":"%s"}' % (_MG_G, _MG_S))
_mg_post("copy_stream", {"from_group": "83b50ca8", "stream": "17dd78a3", "to_group": "354ee7bf"},
         "/api/source/groups/streams/copy",
         '{"from_group_id":"83b50ca8","from_stream_id":"17dd78a3","to_group_id":"354ee7bf"}')
_mg_post("move_stream", {"from_group": "83b50ca8", "stream": "17dd78a3", "to_group": "354ee7bf"},
         "/api/source/groups/streams/move",
         '{"from_group_id":"83b50ca8","from_stream_id":"17dd78a3","to_group_id":"354ee7bf"}')

# ── Layouts (6) ──────────────────────────────────────────────────────────
_mg_get("list_layout_templates", {}, "/api/layout/template/list")
_mg_get("list_layouts", {}, "/api/layout/list")
_mg_post("add_layout", {"template": 1, "name": "AddSingle2"}, "/api/layout/add", '{"template_id":1,"name":"AddSingle2"}')
_mg_post("remove_layout", {"layout": 7}, "/api/layout/remove", '{"layout_id":7}')
_mg_post("select_layout", {"output": 1, "layout": 2}, "/api/layout/select", '{"output_id":"1","layout_id":2}')
_mg_post("modify_layout", {"output": 1, "layout": 6, "panes": [{"width": 30, "height": 30, "number": 2, "z_index": 1,
                                                                 "id": 2, "x": 0, "y": 0}]},
         "/api/layout/modify",
         '{"layout_id":6,"position":[{"width":30,"height":30,"number":2,"z_index":1,"id":2,"x":0,"y":0}],"output_id":"1"}')
_mg_post("modify_layout_pane", {"output": 1, "layout": 6, "pane": 2, "number": 2, "x": 0, "y": 0, "width": 30,
                                "height": 30, "z_index": 1},
         "/api/layout/modify",
         '{"layout_id":6,"position":[{"width":30,"height":30,"number":2,"z_index":1,"id":2,"x":0,"y":0}],"output_id":"1"}')
_mg_post("save_layout", {"output": 1, "layout": 6}, "/api/layout/save", '{"output_id":"1","layout_id":6}')
_mg_post("save_layout_as", {"output": 1, "name": "Test"}, "/api/layout/resave", '{"output_id":"1","layout_name":"Test"}')
_mg_post("reload_layout", {"output": 1, "layout": 6, "name": "5"}, "/api/layout/reload",
         '{"output_id":1,"layout_name":"5","layout_id":6}')
_mg_post("rename_layout", {"layout": 1, "name": "Wide"}, "/api/layout/rename", '{"layout_id":1,"name":"Wide"}')

# ── Outputs (7) ──────────────────────────────────────────────────────────
_mg_get("get_output_interfaces", {"output": 2, "type": "audio"}, "/api/output/interfaces/get?output_id=2&type=audio")
_mg_post("set_audio_interface", {"output": 1, "interface": 1, "label": "HDMI1", "enabled": True, "muted": True,
                                 "volume": 14},
         "/api/output/interfaces/set",
         '{"channels":2,"enable":true,"id":1,"label":"HDMI1","mute":true,"samplerate":48000,"visible":true,'
         '"volume":14,"output_id":"1","type":"audio","intf_id":1}')
_mg_post("set_video_interface", {"output": 2, "interface": 3, "enabled": True}, "/api/output/interfaces/set",
         '{"enable":true,"output_id":"2","type":"video","intf_id":3}')
_mg_get("list_outputs", {}, "/api/output/list")
_mg_get("get_output", {"output": 2}, "/api/output/get?output_id=2")
_mg_post("set_window_source", {"output": 1, "layout": 3, "window": 1, "stream": "7faa9bc2", "stream_name": "df",
                               "stream_url": "udp://172.30.47.230:2555"},
         "/api/output/source/set",
         '{"from":{"type":"source","stream_id":"7faa9bc2","stream_name":"df","stream_url":"udp://172.30.47.230:2555",'
         '"pos_id":"","output_id":"1","layout_id":""},"to":{"type":"output","stream_id":"7faa9bc2","stream_name":"df",'
         '"stream_url":"udp://172.30.47.230:2555","pos_id":1,"output_id":"1","layout_id":3}}')
_mg_post("set_window_source_raw", {"body": {"from": {"type": "source", "stream_id": "a"},
                                            "to": {"type": "output", "stream_id": "a", "pos_id": 2}}},
         "/api/output/source/set",
         '{"from":{"type":"source","stream_id":"a"},"to":{"type":"output","stream_id":"a","pos_id":2}}')
_mg_post("clear_window", {"output": 1, "window": 1}, "/api/output/source/remove", '{"output_id":"1","pos_id":1}')
_mg_get("list_resolutions", {}, "/api/output/resolution/list")
_mg_post("set_resolution", {"output": 1, "resolution": 14}, "/api/output/resolution/set",
         '{"output_id":"1","res_id":"14"}')
_mg_get("get_audio_mix", {"output": 1}, "/api/output/audiomix/get?output_id=1")
_mg_post("set_audio_mix", {"output": 1, "stream": "895a2714", "enabled": True}, "/api/output/audiomix/set",
         '{"output_id":"1","stream_id":"895a2714","enable":true,"volume":0,"mute":false,"type":"output"}')
_mg_post("remove_audio_mix", {"output": 1, "stream": "e893d03a", "list": "preview"}, "/api/output/audiomix/remove",
         '{"output_id":"1","stream_id":"e893d03a","type":"preview"}')
_mg_post("add_audio_mix", {"output": 1, "stream": "895a2714"}, "/api/output/audiomix/add",
         '{"output_id":"1","stream_id":"895a2714"}')

# PTZ (7.13).
_mg_post("ptz_zoom", {"output": 1, "window": 1, "value": 0.5}, "/api/ptz/set",
         '{"output_id":"1","pos_id":1,"type":"ptz_zoom","data":{"zoom_value":0.500}}')
_mg_post("ptz_zoom_speed", {"output": 1, "window": 1, "speed": -1.0}, "/api/ptz/set",
         '{"output_id":"1","pos_id":1,"type":"ptz_zoom_speed","data":{"zoom_speed":-1.000}}')
_mg_post("ptz_pan_tilt", {"output": 2, "window": 3, "pan": -0.25, "tilt": 0.5}, "/api/ptz/set",
         '{"output_id":"2","pos_id":3,"type":"ptz_pan_tilt","data":{"pan_value":-0.250,"tilt_value":0.500}}')
_mg_post("ptz_pan_tilt_speed", {"output": 1, "window": 1, "pan_speed": 0.0, "tilt_speed": 0.0}, "/api/ptz/set",
         '{"output_id":"1","pos_id":1,"type":"ptz_pan_tilt_speed","data":{"pan_speed":0.000,"tilt_speed":0.000}}')
_mg_post("ptz_store_preset", {"output": 1, "window": 1, "preset": 4}, "/api/ptz/set",
         '{"output_id":"1","pos_id":1,"type":"ptz_store_preset","data":{"preset_no":4}}')
_mg_post("ptz_recall_preset", {"output": 1, "window": 1, "preset": 1}, "/api/ptz/set",
         '{"output_id":"1","pos_id":1,"type":"ptz_recall_preset","data":{"preset_no":1,"speed":0.500}}')
_mg_post("ptz_auto_focus", {"output": 1, "window": 1}, "/api/ptz/set",
         '{"output_id":"1","pos_id":1,"type":"ptz_auto_focus","data":{}}')
_mg_post("ptz_focus", {"output": 1, "window": 1, "value": 0.75}, "/api/ptz/set",
         '{"output_id":"1","pos_id":1,"type":"ptz_focus","data":{"focus_value":0.750}}')
_mg_post("ptz_focus_speed", {"output": 1, "window": 1, "speed": 0.5}, "/api/ptz/set",
         '{"output_id":"1","pos_id":1,"type":"ptz_focus_speed","data":{"focus_speed":0.500}}')
_mg_post("ptz_white_balance", {"output": 1, "window": 1, "mode": "outdoor"}, "/api/ptz/set",
         '{"output_id":"1","pos_id":1,"type":"ptz_white_balance_outdoor","data":{}}')
_mg_post("ptz_white_balance_manual", {"output": 1, "window": 1, "red": 0.4, "blue": 0.6}, "/api/ptz/set",
         '{"output_id":"1","pos_id":1,"type":"ptz_white_balance_manual","data":{"red":0.400,"blue":0.600}}')
_mg_post("ptz_exposure_auto", {"output": 1, "window": 1}, "/api/ptz/set",
         '{"output_id":"1","pos_id":1,"type":"ptz_exposure_auto","data":{}}')
_mg_post("ptz_exposure_manual", {"output": 1, "window": 1, "level": 0.3}, "/api/ptz/set",
         '{"output_id":"1","pos_id":1,"type":"ptz_exposure_manual","data":{"exposure_level":0.300}}')
_mg_post("ptz_is_supported", {"output": 1, "window": 1}, "/api/ptz/set",
         '{"output_id":"1","pos_id":1,"type":"ptz_is_supported","data":{}}',
         http_reply={"status": 200, "body": '{"result":"error","msg":"508"}'},
         expect_result={"error": {"error": "device_error"}})

# Output tools (7.14-7.25).
_mg_get("get_audio_levels", {"output": 2}, "/api/output/audio/sound?output_id=2")
_mg_post("set_border", {"output": 1, "enabled": True, "color": "#FF0000"}, "/api/output/border/set",
         '{"output_id":"1","enable":true,"color":"#FF0000"}')
_mg_get("get_border", {"output": 2}, "/api/output/border/get?output_id=2")
_mg_post("set_background", {"output": 1, "enabled": True, "color": "#000000"}, "/api/output/background/set",
         '{"output_id":"1","enable":true,"background":"#000000"}')
_mg_get("get_background", {"output": 1}, "/api/output/background/get?output_id=1")
_mg_post("check_audio", {"id": ""}, "/api/output/audio/check", '{"id":""}',
         http_reply={"status": 200, "body": '{"data":true,"result":"ok"}'},
         expect_result={"ok": {"kind": "value", "value": True}})
_mg_post("set_ts_program", {"stream": "7faa9bc2", "selection": 3}, "/api/output/ts/program/set",
         '{"stream_id":"7faa9bc2","selected_id":3}')
_mg_get("get_ts_programs", {"stream": "7faa9bc2"}, "/api/output/ts/program/get?stream_id=7faa9bc2")
_mg_get("get_encoder", {"output": 1}, "/api/output/encode/get?output_id=1")
_mg_post("set_encoder", {"output": 1, "codec": "H265"}, "/api/output/encode/set",
         '{"output_id":1,"main":{"codec":"H265","grey":false,"profile":0,"pic_width":1,"pic_height":1,"mode":"VBR",'
         '"gop":60,"fps":"auto","bitrate":6000,"max_bitrate":20000,"code_frame":0,"deinterlace_mode":"default"}}')
_mg_post("set_encoder", {"output": 2, "codec": "H264", "profile": "baseline", "width": 1280, "height": 720,
                         "mode": "CBR", "fps": "30", "bitrate": 4000, "deinterlace": "line_doubling"},
         "/api/output/encode/set",
         '{"output_id":2,"main":{"codec":"H264","grey":false,"profile":2,"pic_width":1280,"pic_height":720,'
         '"mode":"CBR","gop":60,"fps":"30","bitrate":4000,"max_bitrate":20000,"code_frame":0,'
         '"deinterlace_mode":"line_doubling"}}', file="set_encoder-fixed")
_mg_post("set_vu_meter", {"output": 1, "window": -1, "enabled": False}, "/api/output/vumeter/set",
         '{"enable":false,"output_id":"1","pos_id":-1}')
_mg_post("set_window_mute", {"output": 1, "window": 0, "muted": True}, "/api/output/mute/set",
         '{"output_id":1,"pos_id":0,"mute":true}')

# ── Gateway streams (8) ──────────────────────────────────────────────────
_mg_get("list_gateway_streams", {}, "/api/gate/stream/list")
_mg_post("add_gateway_stream", {"body": {"name": "hls1", "protocol": "hls", "is_enable": True, "mode": "server",
                                         "session": "a", "segment_time": 2, "max_segments": 5}},
         "/api/gate/stream/add",
         '{"name":"hls1","protocol":"hls","is_enable":true,"mode":"server","session":"a","segment_time":2,'
         '"max_segments":5}')
_mg_post("add_gateway_rtmp_stream", {"name": "Push 1", "enabled": True, "address": "rtmp://192.168.1.100/live/stream"},
         "/api/gate/stream/add",
         '{"name":"Push 1","protocol":"rtmp","is_enable":true,"address":"rtmp://192.168.1.100/live/stream","user":"",'
         '"password":"","conn_intv":3,"conn_timeout":10,"old_rtmp":false}')
_mg_post("add_gateway_rtsp_stream", {"name": "RS96", "session": "ch96", "port": 596},
         "/api/gate/stream/add",
         '{"name":"RS96","protocol":"rtsp","is_enable":false,"session":"ch96","port":596,"http_tunnel_port":8554,'
         '"auth":false,"user":"","pwd":"","multicast_enable":false,"multicast_addr":"224.0.1.0","multicast_ttl":127,'
         '"multicast_port_min":31000,"multicast_port_max":31004}')
_mg_post("add_gateway_rtp_stream", {"name": "RTP", "address": "239.1.1.1", "port": 5000, "load_type": 33,
                                    "service_name": "Stage", "service_provider": "Kiloview", "ttl": 64,
                                    "pcr_period": 40, "transport_stream_id": 1, "start_pid": 256, "pmt_start_pid": 4096, "tables_version": 0},
         "/api/gate/stream/add",
         '{"name":"RTP","protocol":"rtp","is_enable":false,"address":"239.1.1.1","port":5000,"load_type":33,"ttl":64,'
         '"ts_service_name":"Stage","ts_service_provider":"Kiloview","ts_pcr_period":40,"ts_transport_stream_id":1,'
         '"ts_start_pid":256,"ts_pmt_start_pid":4096,"ts_tables_version":0}')
_mg_post("add_gateway_ts_stream", {"name": "TS", "address": "239.1.1.2", "port": 5002, "bind_port": 6000, "ttl": 64,
                                   "pts_pcr_delay": 0, "pcr_period": 40, "transport_stream_id": 1, "start_pid": 256, "pmt_start_pid": 4096, "tables_version": 0},
         "/api/gate/stream/add",
         '{"name":"TS","protocol":"ts","is_enable":false,"address":"239.1.1.2","port":5002,"bind_port":6000,"ttl":64,'
         '"ts_pts_pcr_delay":0,"ts_service_name":"","ts_service_provider":"","ts_pcr_period":40,'
         '"ts_transport_stream_id":1,"ts_start_pid":256,"ts_pmt_start_pid":4096,"ts_tables_version":0}')
_mg_post("add_gateway_srt_stream", {"name": "SRT", "mode": "Listener", "port": 9000, "pcr_period": 40, "start_pid": 256, "pmt_start_pid": 4096, "tables_version": 0},
         "/api/gate/stream/add",
         '{"name":"SRT","protocol":"srt","is_enable":false,"connection_mode":"Listener","address":"",'
         '"listener_port":9000,"latency":125,"encryption":"0","passphrase":"","ts_service_name":"",'
         '"ts_service_provider":"","ts_pcr_period":40,"ts_start_pid":256,"ts_pmt_start_pid":4096,'
         '"ts_tables_version":0}')
_mg_post("add_gateway_hls_stream", {"name": "HLS", "session": "live1", "segment_time": 2, "max_segments": 5}, "/api/gate/stream/add",
         '{"name":"HLS","protocol":"hls","is_enable":false,"mode":"server","session":"live1","segment_time":2,'
         '"max_segments":5}')
_mg_post("add_gateway_ndi_hx_stream", {"name": "NDI", "channel_name": "Stage", "connection": "multicast", "ttl": 64,
                                       "netmask": "255.255.0.0", "netprefix": "239.255.0.0"},
         "/api/gate/stream/add",
         '{"name":"NDI","protocol":"ndi_hx","is_enable":false,"channel_name":"Stage","ttl":64,'
         '"connection":"multicast","netmask":"255.255.0.0","netprefix":"239.255.0.0"}')
_mg_post("update_gateway_stream", {"body": {"id": _MG_GW, "name": "rtmp0", "is_enable": False}},
         "/api/gate/stream/update", '{"id":"%s","name":"rtmp0","is_enable":false}' % _MG_GW)
_mg_post("update_gateway_rtmp_stream", {"id": _MG_GW, "address": "rtmp://172.30.2.27:1935/live/1", "user": "admin",
                                        "password": "Admin123", "connect_timeout": 15},
         "/api/gate/stream/update",
         '{"id":"%s","protocol":"rtmp","address":"rtmp://172.30.2.27:1935/live/1","user":"admin",'
         '"password":"Admin123","conn_intv":3,"conn_timeout":15,"old_rtmp":false}' % _MG_GW)
_mg_post("update_gateway_rtsp_stream", {"id": _MG_GW, "session": "ch1", "port": 554, "auth": True, "user": "a",
                                        "password": "b", "multicast": True},
         "/api/gate/stream/update",
         '{"id":"%s","protocol":"rtsp","session":"ch1","port":554,"http_tunnel_port":8554,"auth":true,"user":"a",'
         '"pwd":"b","multicast_enable":true,"multicast_addr":"224.0.1.0","multicast_ttl":127,'
         '"multicast_port_min":31000,"multicast_port_max":31004}' % _MG_GW)
_mg_post("update_gateway_rtp_stream", {"id": _MG_GW, "address": "239.1.1.1", "port": 5000, "load_type": 33,
                                       "ttl": 64, "pcr_period": 40, "transport_stream_id": 1, "start_pid": 256, "pmt_start_pid": 4096, "tables_version": 0},
         "/api/gate/stream/update",
         '{"id":"%s","protocol":"rtp","address":"239.1.1.1","port":5000,"load_type":33,"ttl":64,'
         '"ts_service_name":"","ts_service_provider":"","ts_pcr_period":40,"ts_transport_stream_id":1,'
         '"ts_start_pid":256,"ts_pmt_start_pid":4096,"ts_tables_version":0}' % _MG_GW)
_mg_post("update_gateway_ts_stream", {"id": _MG_GW, "address": "239.1.1.2", "port": 5002, "pts_pcr_delay": 100,
                                      "ttl": 64, "pcr_period": 40, "transport_stream_id": 1, "start_pid": 256, "pmt_start_pid": 4096, "tables_version": 0},
         "/api/gate/stream/update",
         '{"id":"%s","protocol":"ts","address":"239.1.1.2","port":5002,"bind_port":0,"ttl":64,"ts_pts_pcr_delay":100,'
         '"ts_service_name":"","ts_service_provider":"","ts_pcr_period":40,"ts_transport_stream_id":1,'
         '"ts_start_pid":256,"ts_pmt_start_pid":4096,"ts_tables_version":0}' % _MG_GW)
_mg_post("update_gateway_srt_stream", {"id": _MG_GW, "mode": "Caller", "address": "10.0.0.9", "port": 9000,
                                       "latency": 500, "pcr_period": 40, "start_pid": 256, "pmt_start_pid": 4096, "tables_version": 0},
         "/api/gate/stream/update",
         '{"id":"%s","protocol":"srt","connection_mode":"Caller","address":"10.0.0.9","listener_port":9000,'
         '"latency":500,"encryption":"0","passphrase":"","ts_service_name":"","ts_service_provider":"",'
         '"ts_pcr_period":40,"ts_start_pid":256,"ts_pmt_start_pid":4096,"ts_tables_version":0}' % _MG_GW)
_mg_post("update_gateway_hls_stream", {"id": _MG_GW, "session": "live2", "segment_time": 4, "max_segments": 5},
         "/api/gate/stream/update",
         '{"id":"%s","protocol":"hls","mode":"server","session":"live2","segment_time":4,"max_segments":5}' % _MG_GW)
_mg_post("update_gateway_ndi_hx_stream", {"id": _MG_GW, "channel_name": "Stage 2", "ttl": 64}, "/api/gate/stream/update",
         '{"id":"%s","protocol":"ndi_hx","channel_name":"Stage 2","ttl":64,"connection":"","netmask":"",'
         '"netprefix":""}' % _MG_GW)
_mg_post("rename_gateway_stream", {"id": _MG_GW, "name": "rtmp0"}, "/api/gate/stream/update",
         '{"id":"%s","name":"rtmp0"}' % _MG_GW)
_mg_post("delete_gateway_stream", {"id": _MG_GW}, "/api/gate/stream/delete", '{"ids":[{"id":"%s"}]}' % _MG_GW)
_mg_post("delete_gateway_streams", {"ids": [{"id": "a808357e"}, {"id": "3cce2584"}]}, "/api/gate/stream/delete",
         '{"ids":[{"id":"a808357e"},{"id":"3cce2584"}]}')
_mg_post("bind_gateway_source", {"id": "b316084c", "stream": "1feb0d04"}, "/api/gate/stream/bind_src",
         '{"id":"b316084c","stream_id":"1feb0d04"}')
_mg_post("set_gateway_stream_enabled", {"id": "afab531c", "enabled": True}, "/api/gate/stream/batch_enable",
         '{"ids":[{"id":"afab531c"}],"enable":true}')
_mg_post("set_gateway_streams_enabled", {"ids": [{"id": "afab531c"}, {"id": "b316084c"}], "enabled": False},
         "/api/gate/stream/batch_enable", '{"ids":[{"id":"afab531c"},{"id":"b316084c"}],"enable":false}')
_mg_get("get_encoded_preview_output", {}, "/api/gate/stream/multi_out_query")
_mg_post("set_encoded_preview_output", {"output": 2}, "/api/gate/stream/multi_out_enable",
         '{"enable":2,"skip_switch_confirm":false}')

# ── Network (9) ──────────────────────────────────────────────────────────
_mg_get("get_network", {}, "/api/network/get")
_mg_post("set_network", {"interface": "eth0", "dhcp": False, "ip": "192.168.36.6", "mask": "255.255.255.0",
                         "gateway": "192.168.36.254", "mac": "18:C3:F4:25:66:66"},
         "/api/network/set",
         '{"device":"eth0","dynamic":"n","ip":"192.168.36.6","mask":"255.255.255.0","gw":"192.168.36.254",'
         '"dns":"","mac":"18:C3:F4:25:66:66"}')
_mg_post("set_maintenance_address", {"interface": "eth0", "enabled": True, "ip": "192.168.1.168",
                                     "mask": "255.255.255.0"},
         "/api/network/modify",
         '{"device":"eth0","enable2":true,"ip2":"192.168.1.168","mask2":"255.255.255.0","gw2":""}')
_mg_get("get_http_ports", {}, "/api/network/http/get")
_mg_post("set_http_ports", {"http": True, "https": True}, "/api/network/http/set",
         '{"http":true,"httpPort":80,"https":true,"httpsPort":443}')
_mg_post("ping", {"address": "192.168.36.124"}, "/api/network/ping", '{"ip":"192.168.36.124"}',
         http_reply={"status": 200, "body": '{"data":{"lossRate":0,"reachable":true},"result":"ok"}'},
         expect_result={"ok": {"kind": "value", "value": {"lossRate": 0, "reachable": True}}})
_mg_get("list_routes", {}, "/api/network/routing/get")
_mg_post("add_route", {"interface": "eth1", "ip": "192.168.1.1", "netmask": "255.255.255.255",
                       "gateway": "192.168.36.6"},
         "/api/network/routing/add",
         '{"device":"eth1","ip":"192.168.1.1","netmask":"255.255.255.255","gw":"192.168.36.6","mtu":1500,"metric":0}')
_mg_post("modify_route", {"id": "e762817b", "interface": "eth1", "ip": "192.168.1.2", "netmask": "255.255.255.255",
                          "gateway": "192.168.36.6"},
         "/api/network/routing/modify",
         '{"id":"e762817b","device":"eth1","ip":"192.168.1.2","netmask":"255.255.255.255","gw":"192.168.36.6",'
         '"mtu":1500,"metric":0}')
_mg_post("set_route_enabled", {"id": "c26163a6", "enabled": False}, "/api/network/routing/set",
         '{"id":"c26163a6","status":0}')
_mg_post("delete_route", {"id": "e762817b"}, "/api/network/routing/del", '{"ids":"e762817b"}')

# ── Time (10) ────────────────────────────────────────────────────────────
_mg_get("get_time_settings", {}, "/api/timeregion/settings")
_mg_post("set_time", {"mode": "pc", "time": "2024-09-09 09:20:46", "offset": 8, "ntp_servers": "ntp1.example.org"},
         "/api/timeregion/time/set",
         '{"time":"2024-09-09 09:20:46","timetype":"pc","offset":8,"ntp_servers":"ntp1.example.org"}')
_mg_post("set_timezone", {"timezone": "Asia/Shanghai", "offset": 8}, "/api/timeregion/timezone/set",
         '{"timezone":"Asia/Shanghai","offset":8}')

# ── System, maintenance, firmware (11-13) ────────────────────────────────
_mg_get("reboot", {}, "/api/sys/reboot", http_reply=_MG_OK, expect_result={"ok": {"kind": "ack"}})
_mg_get("reset", {}, "/api/sys/reset")
_mg_get("restore_factory", {}, "/api/sys/restore")
_mg_get("get_device_name", {}, "/api/sys/device/get",
        http_reply={"status": 200, "body": '{"data":{"device_name":"MG300-211111"},"result":"ok"}'},
        expect_result={"ok": {"kind": "value", "value": "MG300-211111"}})
_mg_post("set_device_name", {"name": "MG300-Stage"}, "/api/sys/device/set", '{"device_name":"MG300-Stage"}',
         http_reply={"status": 200, "body": '{"result":"error","msg":"500"}'},
         expect_result={"error": {"error": "device_error"}})
_mg_get("parse_url", {"url": "ndi://10.20.30.21:5961"}, "/api/sys/ip/get?url=ndi%3A%2F%2F10.20.30.21%3A5961")
_mg_post("replace_url_address", {"url": "srt://192.168.1.1:13?dddddd", "address": "192.168.1.124", "port": "1234"},
         "/api/sys/ip/replace", '{"url":"srt://192.168.1.1:13?dddddd","target_ip":"192.168.1.124","target_port":"1234"}',
         http_reply={"status": 200, "body": '{"data":{"url":"srt://192.168.1.124:1234?dddddd"},"result":"ok"}'},
         expect_result={"ok": {"kind": "value", "value": "srt://192.168.1.124:1234?dddddd"}})
_mg_upload("import_config", {"file": "UEsDBA==", "filename": "source_config.zip"}, "/api/sys/config/import",
           [("upload", b"PK\x03\x04", "source_config.zip")])
_mg_post("import_config_from_path", {"path": "/tmp/source_config.zip"}, "/api/sys/config/import",
         '{"path":"/tmp/source_config.zip"}')
_mg_get("get_usage", {}, "/api/maintenance/usage_get")
_mg_get("get_no_signal_mode", {}, "/api/sys/preset/get")
_mg_post("set_no_signal_mode", {"mode": "black"}, "/api/sys/preset/set", '{"preset":1}')
_mg_upload("upgrade_firmware", {"file": "AAEC", "filename": "fw.bin"}, "/api/firmware/upgrade",
           [("upload", b"\x00\x01\x02", "fw.bin")], http_reply=_MG_OK, expect_result={"ok": {"kind": "ack"}})

# ── Preview (14) ─────────────────────────────────────────────────────────
_mg_get("get_preview", {}, "/api/preview/get")
_mg_post("add_preview_source", {"output": 1, "stream": "0f79952d", "stream_name": "N60_238 (HX)",
                                "stream_url": "192.168.36.238:5964"},
         "/api/preview/source/modify",
         '{"from":{"type":"source","stream_id":"0f79952d","stream_name":"N60_238 (HX)","stream_url":"192.168.36.238:5964",'
         '"pos_id":"","output_id":"1","layout_id":""},"to":{"type":"preview","stream_id":"0f79952d",'
         '"stream_name":"N60_238 (HX)","stream_url":"192.168.36.238:5964","output_id":"1","layout_id":""}}')
_mg_post("remove_preview_source", {"position": 1}, "/api/preview/source/remove", '{"pos_id":1}')

# ── NDI discovery (15) ───────────────────────────────────────────────────
_mg_get("get_ndi_discovery", {}, "/api/ndi/discovery/all")
_mg_get("list_ndi_sources", {"method": "server", "group": "public", "server": "192.168.43.141"},
        "/api/ndi/discovery/sources?dis_method=4&dis_g_name=public&dis_s_ip=192.168.43.141")
_mg_get("list_ndi_sources_in_group", {"method": "group", "group": "studio", "group_id": "c719a3c5"},
        "/api/ndi/discovery/sources?dis_method=3&dis_g_name=studio&dis_s_ip=&group_id=c719a3c5")
_mg_post("set_ndi_manual_discovery", {"entries": [{"dis_d_ip": "192.168.43.141", "dis_g_name": "public"}]},
         "/api/ndi/discovery/manual/add", '{"manuals":[{"dis_d_ip":"192.168.43.141","dis_g_name":"public"}]}')
_mg_post("add_ndi_group_discovery", {"group": "123"}, "/api/ndi/discovery/auto/add", '{"dis_g_name":"123"}')
_mg_post("remove_ndi_auto_discovery", {"id": "3"}, "/api/ndi/discovery/auto/del", '{"id":"3"}')
_mg_post("add_ndi_discovery_server", {"server": "192.168.43.141"}, "/api/ndi/discovery/server/add",
         '{"dis_g_name":"public","dis_s_ip":"192.168.43.141"}')
_mg_post("remove_ndi_discovery_server", {"id": "4"}, "/api/ndi/discovery/server/del", '{"id":"4"}')

# ── Telemetry: the reads ─────────────────────────────────────────────────
_mg_t("info", "/api/info/get",
      {"data": {"device_name": "MG300-Stage", "firmware_version": "2.20.0132", "hardware_version": "V2",
                "serial_number": "MG3001598", "software_version": "2.0.0"}, "result": "ok"},
      {"device": {"name": "MG300-Stage", "firmware_version": "2.20.0132", "hardware_version": "V2",
                  "serial_number": "MG3001598", "software_version": "2.0.0"}})
_mg_t("device-name", "/api/sys/device/get", {"data": {"device_name": "MG300-211111"}, "result": "ok"},
      {"device": {"name": "MG300-211111"}})
_mg_t("usage", "/api/maintenance/usage_get",
      {"result": "ok", "data": {"cpu_precent": 0.74, "mem_precent": 23.02, "temperature": -1}},
      {"system": {"cpu_percent": 0.74, "memory_percent": 23.02, "temperature": -1.0}})
_mg_t("no-signal-mode", "/api/sys/preset/get", {"result": "ok", "data": 0},
      {"system": {"no_signal_mode": "no_signal_image"}})
_mg_t("auth-settings", "/api/users/auth_info_get", {"result": "ok", "data": {"enable": True, "canModify": True}},
      {"auth": {"enabled": True, "can_modify": True}})
_mg_t("auth-set-reply", "/api/users/auth_info_set", {"result": "ok", "data": {"enable": False}},
      {"auth": {"enabled": False}}, expect_then_send=[{"method": "POST", "target": "/api/users/auth_info_get"}])
_mg_t("session-valid", "/api/users/session/check", {"result": "ok", "data": {"alias": "admin", "username": "admin"}},
      {"session": {"valid": True, "username": "admin", "alias": "admin"}})
_mg_t("session-invalid", "/api/users/session/check", {"result": "error", "msg": "300"}, {"session": {"valid": False}})
_mg_t("output-list", "/api/output/list",
      {"data": [{"background": "#000000", "bg_enable": False,
                 "border": {"bottom_width": 2, "color": "#FF0000", "enable": False, "hex_color": 16711680,
                            "left_width": 2, "right_width": 2, "top_width": 2},
                 "id": "1", "layout_id": 2, "modified": False, "name": "output1", "res_id": "5",
                 "res_name": "1920x1080P60", "vumeter": True}], "result": "ok"},
      {"outputs": {"1": {"name": "output1", "layout": 2, "layout_modified": False, "resolution_id": "5",
                         "resolution": "1920x1080P60", "vu_meter": True, "background": "#000000",
                         "background_enabled": False,
                         "border": {"enabled": False, "color": "#FF0000", "top_width": 2, "bottom_width": 2,
                                    "left_width": 2, "right_width": 2}}}})
_mg_t("output-windows", "/api/output/get?output_id=1",
      {"data": {"layout_id": 2, "name": "output1", "vumeter": True, "position": [
          {"enable": True, "height": 30, "id": 1, "mute": False, "number": 1, "resolution": "1920x1080p60",
           "status": 2, "stream_id": "e893d03a", "stream_ip": "192.168.36.238:5965", "stream_name": "N60_238 (HB)",
           "type": "ndi", "video_bitrate": "55479.4", "video_format": "NDI | HB", "volume": 0, "vu_meter": True,
           "width": 30, "x": 0, "y": 0, "z_index": 0}]}, "result": "ok"},
      {"outputs": {"1": {"name": "output1", "layout": 2, "vu_meter": True, "windows": {"1": {
          "stream_id": "e893d03a", "stream_name": "N60_238 (HB)", "stream_address": "192.168.36.238:5965",
          "type": "ndi", "status": "connected", "enabled": True, "muted": False, "volume": 0, "vu_meter": True,
          "resolution": "1920x1080p60", "video_format": "NDI | HB", "video_bitrate": 55479.4, "number": 1,
          "x": 0.0, "y": 0.0, "width": 30.0, "height": 30.0, "z_index": 0}}}}},
      state_before={"outputs": {"1": {"windows": {"2": {"stream_id": "old"}}}}})
_mg_t("video-interfaces", "/api/output/interfaces/get?output_id=1&type=video",
      {"data": [{"colorspace": "RGB444", "enable": False, "id": 2, "label": "HDMI2", "mode": "HDMI",
                 "support": True, "visible": True}], "result": "ok"},
      {"outputs": {"1": {"video_interfaces": {"2": {"label": "HDMI2", "mode": "HDMI", "colorspace": "RGB444",
                                                    "enabled": False, "supported": True, "visible": True}}}}})
_mg_t("audio-interfaces", "/api/output/interfaces/get?output_id=2&type=audio",
      {"data": [{"id": 4, "label": "LineOut", "enable": True, "samplerate": 44.1, "channels": 2, "volume": -3,
                 "mute": False, "visible": True}], "result": "ok"},
      {"outputs": {"2": {"audio_interfaces": {"4": {"label": "LineOut", "enabled": True, "sample_rate": 44.1,
                                                    "channels": 2, "volume": -3, "muted": False, "visible": True}}}}})
_mg_t("audio-mix", "/api/output/audiomix/get?output_id=1",
      {"data": {"output": [{"stream_id": "a1", "name": "n60", "enable": True, "volume": -6, "mute": False,
                            "pos_id": 1, "type": "output"}],
                "preview": [{"stream_id": "b2", "name": "n50", "enable": False, "volume": 0, "mute": True,
                             "type": "preview"}]}, "result": "ok"},
      {"outputs": {"1": {"audio_mix": {"output": {"a1": {"name": "n60", "enabled": True, "volume": -6, "muted": False,
                                                         "window": 1}},
                                       "preview": {"b2": {"name": "n50", "enabled": False, "volume": 0,
                                                          "muted": True}}}}}})
_mg_t("audio-levels", "/api/output/audio/sound?output_id=1",
      {"data": [{"dB": [-12, -14], "stream_id": "e893d03a"}], "result": "ok"},
      {"outputs": {"1": {"audio_levels": {"0": {"stream_id": "e893d03a", "channels": {"1": -12.0, "2": -14.0}}}}}})
_mg_t("border", "/api/output/border/get?output_id=2",
      {"data": {"bottom_width": 2, "color": "#00FF00", "enable": True, "hex_color": 65280, "left_width": 3,
                "right_width": 3, "top_width": 2}, "result": "ok"},
      {"outputs": {"2": {"border": {"enabled": True, "color": "#00FF00", "top_width": 2, "bottom_width": 2,
                                    "left_width": 3, "right_width": 3}}}})
_mg_t("background", "/api/output/background/get?output_id=1",
      {"data": {"background": "#101010", "enable": True}, "result": "ok"},
      {"outputs": {"1": {"background": "#101010", "background_enabled": True}}})
_mg_t("encoder", "/api/output/encode/get?output_id=1",
      {"result": "ok", "data": {"enable": False, "main": {
          "codec": "H264", "profile": 0, "pic_width": 1, "pic_height": 1, "set_resolution_default": True,
          "grey": False, "fps": "auto", "mode": "VBR", "bitrate": 6000, "max_bitrate": 20000, "gop": 60,
          "deinterlace_mode": "weave", "code_frame": 0, "out_mixer_fps": 60},
          "sub": {"pic_width": 640, "pic_height": 360, "fps": 30, "bitrate": 3000}}},
      {"outputs": {"1": {"encoder": {"enabled": False, "codec": "H264", "profile": "high", "width": 1, "height": 1,
                                     "follows_resolution": True, "grayscale": False, "fps": "auto", "mode": "VBR",
                                     "bitrate": 6000, "max_bitrate": 20000, "gop": 60, "deinterlace": "weave",
                                     "reference_frames": 0, "mixer_fps": 60,
                                     "sub": {"width": 640, "height": 360, "fps": 30, "bitrate": 3000}}}}})
_mg_t("ts-programs", "/api/output/ts/program/get?stream_id=7faa9bc2",
      {"data": {"selected_id": 3, "selection": ["program_2", 3], "supported": True, "tree": [
          {"children": [{"id": 1, "name": "Audio (stream 1)", "pid": 257}], "id": "program_1", "name": "Main"},
          {"children": [{"id": 3, "name": "Audio (stream 3)", "pid": 258}], "id": "program_2", "name": "Basic"}]},
       "result": "ok"},
      {"ts_programs": {"7faa9bc2": {"selected": 3, "selected_name": "program_2", "supported": True, "programs": {
          "program_1": {"name": "Main", "audio": {"1": {"name": "Audio (stream 1)", "pid": 257}}},
          "program_2": {"name": "Basic", "audio": {"3": {"name": "Audio (stream 3)", "pid": 258}}}}}}},
      state_before={"ts_programs": {"7faa9bc2": {"programs": {"program_9": {"name": "gone"}}}}})
_mg_t("overlays", "/api/overlay/list",
      {"data": [{"background": "transparent", "editing": True, "id": 1,
                 "img_path": "./data/decoder_config/overlay/preview/overlay_1.png", "modified": False,
                 "name": "template_1", "selected": False, "status": 0},
                {"background": "output2", "editing": False, "id": 2, "img_path": "", "modified": True,
                 "name": "Logo", "selected": True, "status": 0}], "result": "ok"},
      {"overlays": {"1": {"name": "template_1", "selected": False, "status": 0, "background": "transparent",
                          "editing": True, "modified": False,
                          "image": "./data/decoder_config/overlay/preview/overlay_1.png"},
                    "2": {"name": "Logo", "selected": True, "status": 0, "background": "output2", "editing": False,
                          "modified": True, "image": ""}}},
      state_before={"overlays": {"7": {"name": "deleted"}}},
      expect_then_send=[{"method": "GET", "target": "/api/overlay/items/get?overlay_id=1"},
                        {"method": "GET", "target": "/api/overlay/items/get?overlay_id=2"}])
_mg_t("overlay-items", "/api/overlay/items/get?overlay_id=1",
      {"result": "ok", "data": {"id": 1, "name": "overlay1", "background": "transparent", "list": [
          {"id": 1, "width": 100, "height": 100, "x": 10, "y": 0, "z_index": 0, "type": "image", "opacity": 0,
           "img_id": 2},
          {"id": 3, "x": 0, "y": 0, "z_index": 1, "width": 100, "height": 100, "text_type": "time",
           "format": "hh:mm:ss", "font_color": "0xffffff", "opacity": 0.5, "stroke_enable": False,
           "stroke_color": "0xffffff", "stroke_size": 18, "content": "", "type": "text"}]}},
      {"overlays": {"1": {"name": "overlay1", "background": "transparent"}},
       "overlay_items": {"1": {
           "1": {"type": "image", "x": 10, "y": 0, "width": 100, "height": 100, "z_index": 0, "opacity": 0.0,
                 "image_id": 2},
           "3": {"type": "text", "x": 0, "y": 0, "width": 100, "height": 100, "z_index": 1, "opacity": 0.5,
                 "text_type": "time", "format": "hh:mm:ss", "font_color": "0xffffff", "stroke_enabled": False,
                 "stroke_color": "0xffffff", "stroke_size": 18, "content": ""}}}},
      state_before={"overlay_items": {"1": {"2": {"type": "text"}}}})
_mg_t("overlay-editing", "/api/overlay/editing/get", {"data": {"overlay_id": 1, "editing_path": ""}, "result": "ok"},
      {"overlay_editing": {"overlay": 1, "path": ""}})
_mg_t("overlay-preview", "/api/overlay/preview/get",
      {"data": {"overlay_id": 2, "preview_path": "/tmp/p.png"}, "result": "ok"},
      {"outputs": {"2": {"overlay": 2, "overlay_preview": "/tmp/p.png"}}}, request={"output_id": 2})
_mg_t("overlay-images", "/api/overlay/images/list",
      {"data": [{"id": 1, "img_path": "./test1.png", "width": 1920, "height": 1080}], "result": "ok"},
      {"overlay_images": {"1": {"path": "./test1.png", "width": 1920, "height": 1080}}})
_mg_t("overlay-formats", "/api/overlay/datetime/format/list",
      {"data": {"date": [{"id": 3, "label": "03-14-2012", "format": "MM-dd-yyyy"}],
                "time": [{"id": 2, "label": "1:30 PM", "format": "h:mm A"}]}, "result": "ok"},
      {"overlay_formats": {"date": {"3": {"label": "03-14-2012", "format": "MM-dd-yyyy"}},
                           "time": {"2": {"label": "1:30 PM", "format": "h:mm A"}}}})
_mg_t("overlay-fonts", "/api/overlay/font/list",
      {"data": [{"id": 1, "label": "Sans", "value": "sans.ttf"}], "result": "ok"},
      {"overlay_fonts": {"1": {"label": "Sans", "value": "sans.ttf"}}})
_mg_t("source-groups", "/api/source/groups/list",
      {"data": [{"id": "2ed1f715", "name": "NDI", "streams": [
          {"address": "192.168.36.238", "audio_sync_compst": 0, "buffer": "live:0:0:0:0:0", "connect_speed": 0,
           "id": "0f79952d", "name": "N60_238 (HX)", "ndi_name": "N60_238 (HX)", "preview_mode": "Auto", "status": 2,
           "trans_mode": "tcp", "type": "ndi", "url": "192.168.36.238:5964", "user": "", "password": "",
           "dis_g_name": "public", "audio_track_mode": 0, "audio_track_indices": [0, 2],
           "audio_track_pids": [257, 259], "ts_program_num": -1}]},
          {"id": "0093969758ac42c99245e89f9108ad08", "name": "Other", "streams": [
              {"address": "192.168.36.236", "bandwidth": 25, "connection_mode": "Caller", "encryption": "none",
               "id": "834fd006", "latency": 125, "listener_port": 1025, "name": "SRT_Caller", "payload_size": 1316,
               "show_advanced": "0", "srt_id": "", "status": 3, "type": "srt", "url": "srt://192.168.36.236:1025",
               "passphrase": "x"}]}], "result": "ok"},
      {"sources": {"groups": {"2ed1f715": {"name": "NDI", "order": 0},
                              "0093969758ac42c99245e89f9108ad08": {"name": "Other", "order": 1}},
                   "streams": {"0f79952d": {"group": "2ed1f715", "name": "N60_238 (HX)", "type": "ndi",
                                            "url": "192.168.36.238:5964", "status": "connected",
                                            "ndi_name": "N60_238 (HX)", "ndi_group": "public",
                                            "address": "192.168.36.238", "buffer": "live:0:0:0:0:0",
                                            "transport": "tcp", "preview_mode": "Auto", "connect_speed": 0,
                                            "audio_sync": 0, "user": "", "audio_track_mode": 0, "ts_program": -1,
                                            "audio_tracks": {"0": 0, "1": 2},
                                            "audio_track_pids": {"0": 257, "1": 259}},
                               "834fd006": {"group": "0093969758ac42c99245e89f9108ad08", "name": "SRT_Caller",
                                            "type": "srt", "url": "srt://192.168.36.236:1025", "status": "failed",
                                            "address": "192.168.36.236", "srt_mode": "Caller", "srt_port": 1025,
                                            "srt_latency": 125, "srt_stream_id": "", "srt_encryption": "none",
                                            "srt_bandwidth": 25, "srt_payload_size": 1316,
                                            "srt_show_advanced": "0"}}}},
      state_before={"sources": {"streams": {"gone": {"name": "deleted"}}}})
_mg_t("layouts", "/api/layout/list",
      {"data": [{"layout_count": 2, "layout_id": 2, "name": "PIP", "used": True, "position": [
          {"height": 100, "id": 1, "number": 1, "stream_id": "", "width": 100, "x": 0, "y": 0, "z_index": 0},
          {"height": 30, "id": 2, "number": 2, "stream_id": "e893d03a", "width": 30, "x": 69, "y": 69,
           "z_index": 1}]}], "result": "ok"},
      {"layouts": {"2": {"name": "PIP", "pane_count": 2, "used": True, "panes": {
          "1": {"stream_id": "", "number": 1, "x": 0.0, "y": 0.0, "width": 100.0, "height": 100.0, "z_index": 0},
          "2": {"stream_id": "e893d03a", "number": 2, "x": 69.0, "y": 69.0, "width": 30.0, "height": 30.0,
                "z_index": 1}}}}})
_mg_t("layout-templates", "/api/layout/template/list",
      {"count": 1000, "data": [{"name": "3x3", "template_count": 9, "template_id": 4, "position": [
          {"height": 33.33, "id": 2, "number": 2, "stream_id": "", "width": 33.33, "x": 33.33, "y": 0,
           "z_index": 1}]}], "result": "ok"},
      {"layout_templates": {"4": {"name": "3x3", "pane_count": 9, "panes": {
          "2": {"number": 2, "x": 33.33, "y": 0.0, "width": 33.33, "height": 33.33, "z_index": 1}}}}})
_mg_t("resolutions", "/api/output/resolution/list",
      {"data": [{"id": "5", "name": "1920x1080P60", "value": "1920x1080P60"},
                {"id": "19", "name": "576i/625i 50Hz(SDI only)", "value": "576i/625i 50Hz(SDI only)"}],
       "result": "ok"},
      {"resolutions": {"5": {"name": "1920x1080P60", "value": "1920x1080P60"},
                       "19": {"name": "576i/625i 50Hz(SDI only)", "value": "576i/625i 50Hz(SDI only)"}}})
_mg_t("preview", "/api/preview/get",
      {"data": {"position": [{"height": 144, "id": 1, "resolution": "1920x1080p60", "status": 1,
                              "stream_id": "e893d03a", "stream_name": "N60_238 (HB)",
                              "stream_url": "192.168.36.238:5965", "type": "ndi", "width": 320, "x": 0, "y": 0}]},
       "result": "ok"},
      {"preview": {"windows": {"1": {"stream_id": "e893d03a", "stream_name": "N60_238 (HB)",
                                     "stream_url": "192.168.36.238:5965", "type": "ndi", "status": "connecting",
                                     "resolution": "1920x1080p60", "x": 0, "y": 0, "width": 320, "height": 144}}}})
_mg_t("gateway-streams", "/api/gate/stream/list",
      {"data": [{
          "audio_stream_info": {"bits": 0, "channels": 2, "codec_id": "AAC", "sample_rate": 48000},
          "auth": False, "dest_url": "", "http_tunnel_port": 8554, "id": "f29fe5e3", "index": 34,
          "is_enable": False, "multicast_addr": "224.0.1.0", "multicast_enable": False, "multicast_port_max": 31004,
          "multicast_port_min": 31000, "multicast_ttl": 127, "name": "RS96",
          "net_statics": {"down_kbps": 0, "up_kbps": 0}, "output_state": "inactive", "port": 596,
          "protocol": "rtsp", "pwd": "", "session": "ch96", "src_url": "rtsp://172.30.48.227:596/ch96",
          "stream_id": "c9c7339a", "stream_name": "", "user": "", "error_code": 0, "error_msg": "",
          "ts_program_num": -1,
          "video_stream_info": {"codec_id": "H.264", "format": "p", "fps": 60, "height": 1080, "width": 1920}},
          {"id": "9a1b2c3d", "index": 35, "name": "HLS", "protocol": "hls", "is_enable": True, "src_url": "",
           "stream_id": "", "stream_name": "", "error_code": 610, "error_msg": "no source",
           "output_state": "error", "mode": "server", "segment_time": 2, "max_segments": 5,
           "media_playlist_url": "http://10.0.0.5/hls/live1.m3u8"}], "result": "ok"},
      {"gateway": {"streams": {
          "f29fe5e3": {"index": 34, "name": "RS96", "protocol": "rtsp", "enabled": False,
                       "source_url": "rtsp://172.30.48.227:596/ch96", "stream_id": "c9c7339a", "stream_name": "",
                       "error_code": 0, "error_message": "", "output_state": "inactive",
                       "video": {"codec": "H.264", "scan": "p", "fps": 60, "width": 1920, "height": 1080},
                       "audio": {"bits": 0, "channels": 2, "codec": "AAC", "sample_rate": 48000},
                       "down_kbps": 0, "up_kbps": 0, "ts_program": -1, "dest_url": "", "user": "", "auth": False,
                       "session": "ch96", "port": 596, "http_tunnel_port": 8554,
                       "multicast": {"enabled": False, "address": "224.0.1.0", "port_min": 31000, "port_max": 31004,
                                     "ttl": 127}},
          "9a1b2c3d": {"index": 35, "name": "HLS", "protocol": "hls", "enabled": True, "source_url": "",
                       "stream_id": "", "stream_name": "", "error_code": 610, "error_message": "no source",
                       "output_state": "error", "hls_mode": "server", "hls_segment_time": 2, "hls_max_segments": 5,
                       "hls_playlist_url": "http://10.0.0.5/hls/live1.m3u8"}}}},
      state_before={"gateway": {"streams": {"old": {"name": "deleted"}}}})
_mg_t("encoded-preview", "/api/gate/stream/multi_out_query",
      {"data": {"enable": 1, "skip_switch_confirm": False}, "result": "ok"},
      {"gateway": {"encoded_preview_output": 1, "skip_switch_confirm": False}})
_mg_t("network", "/api/network/get",
      {"data": [{"device": "eth0", "dns": "", "dynamic": "n", "enable": 1, "enable2": True, "gw": "192.168.36.254",
                 "gw2": "", "ip": "192.168.36.6", "ip2": "192.168.1.163", "mac": "18:C3:F4:25:66:66",
                 "mask": "255.255.255.0", "mask2": "255.255.255.0", "state": "up"}], "result": "ok"},
      {"network": {"interfaces": {"eth0": {"enabled": True, "link": "up", "dhcp": False, "ip": "192.168.36.6",
                                           "mask": "255.255.255.0", "gateway": "192.168.36.254", "dns": "",
                                           "mac": "18:C3:F4:25:66:66",
                                           "maintenance": {"enabled": True, "ip": "192.168.1.163",
                                                           "mask": "255.255.255.0", "gateway": ""}}}}})
_mg_t("http-ports", "/api/network/http/get",
      {"data": {"http": True, "httpPort": 80, "https": True, "httpsPort": 443, "authEnable": False}, "result": "ok"},
      {"network": {"http": {"enabled": True, "port": 80}, "https": {"enabled": True, "port": 443},
                   "http_auth": False}})
_mg_t("routes", "/api/network/routing/get",
      {"result": "ok", "data": [{"id": 1, "device": "eth0", "ip": "192.168.38.12", "netmask": "255.255.255.0",
                                 "gw": "192.168.38.254", "mtu": 1500, "metric": 0, "status": 0}]},
      {"network": {"routes": {"1": {"interface": "eth0", "ip": "192.168.38.12", "netmask": "255.255.255.0",
                                    "gateway": "192.168.38.254", "mtu": 1500, "metric": 0, "enabled": False}}}})
_mg_t("time", "/api/timeregion/settings",
      {"data": {"ntp_servers": "ntp1.example.org", "offset": 8, "time": "2024-09-09 10:40:46", "timetype": "pc",
                "timezone": "Asia/Shanghai"}, "result": "ok"},
      {"time": {"ntp_servers": "ntp1.example.org", "offset": 8, "current": "2024-09-09 10:40:46", "source": "pc",
                "timezone": "Asia/Shanghai"}})
_mg_t("users", "/api/users/list",
      {"data": {"count": 1, "list": [
          {"alias": "test1", "changed": True, "create_time": "169733754", "created_by": 1, "descript": "",
           "id": 2, "is_self": False, "is_system": True, "phone": "", "role_id": 2, "role_name": "admin_regular",
           "status": 1, "token": "9b8b92fb007a86dee14f5d89bf0c4d2a", "updated_at": "1970-01-03T07:08:53",
           "username": "admin1"}], "roles": []}, "result": "ok"},
      {"users_count": 1,
       "users": {"2": {"username": "admin1", "alias": "test1", "role": 2, "role_name": "admin_regular", "phone": "",
                       "description": "", "created": "169733754", "created_by": 1,
                       "updated": "1970-01-03T07:08:53", "password_changed": True, "is_self": False,
                       "is_system": True, "status": 1}}},
      state_before={"users": {"9": {"username": "gone"}}})
_mg_t("roles", "/api/roles/permissions",
      {"result": "ok", "data": [{"created_by": 2, "description": "fixed role", "id": 3, "is_self": False,
                                 "is_system": False, "label": "gen_role_1", "name": "gen_role_1", "status": 1,
                                 "permissions": [{"id": 1, "is_group": True, "label": "Home", "children": [
                                     {"checked": True, "id": 2, "is_group": False, "label": "Sources"},
                                     {"checked": False, "id": 5, "is_group": False, "label": "Outputs",
                                      "children": [{"checked": True, "id": 8, "is_group": True, "label": "Audio",
                                                    "multiple_selected": True, "children": [
                                                        {"checked": False, "id": 9, "is_group": False,
                                                         "label": "Audio output", "translate_name": "ID_X"}]}]}]}]}]},
      {"roles": {"3": {"name": "gen_role_1", "label": "gen_role_1", "description": "fixed role", "is_system": False,
                       "is_self": False, "status": 1, "created_by": 2, "permissions": {
                           "1": {"label": "Home", "group": True, "parent": 0},
                           "2": {"label": "Sources", "checked": True, "group": False, "parent": 1},
                           "5": {"label": "Outputs", "checked": False, "group": False, "parent": 1},
                           "8": {"label": "Audio", "checked": True, "group": True, "partial": True, "parent": 5},
                           "9": {"label": "Audio output", "checked": False, "group": False, "name": "ID_X",
                                 "parent": 8}}}}},
      state_before={"roles": {"6": {"name": "gone"}}})
_mg_t("ndi-discovery", "/api/ndi/discovery/all",
      {"data": {"manual": [{"dis_d_ip": "192.168.43.141", "dis_g_name": "public", "dis_method": 2, "dis_s_ip": "",
                            "id": "1"}],
                "auto_server": [{"dis_d_ip": "", "dis_g_name": "public", "dis_method": 4,
                                 "dis_s_ip": "192.168.43.141", "id": "4"}]}, "result": "ok"},
      {"ndi_discovery": {"1": {"group": "public", "address": "192.168.43.141", "server": "", "method": 2},
                         "4": {"group": "public", "address": "", "server": "192.168.43.141", "method": 4}}},
      state_before={"ndi_discovery": {"9": {"group": "old"}}})
_mg_t("ndi-sources", "/api/ndi/discovery/sources?dis_method=1&dis_g_name=&dis_s_ip=",
      {"data": [{"channel": "FULL", "group_name": "public", "ip": "192.168.35.178", "name": "N60-N6020240531 ",
                 "ndi_name": "N60.lab (FULL)", "port": 5961, "url": "192.168.35.178:5961"}], "result": "ok"},
      {"ndi": {"sources": {"N60.lab (FULL)": {"name": "N60-N6020240531 ", "url": "192.168.35.178:5961",
                                              "ip": "192.168.35.178", "port": 5961, "channel": "FULL",
                                              "group": "public"}}}})
_mg_t("error-reply-ignored", "/api/info/get", {"result": "error", "msg": "2"}, {})

# Removals of state the lists cannot see.
_mg_t("overlay-removed", "/api/overlay/remove", {"result": "ok"}, {"overlay_items": {}},
      request={"overlay_id": 2}, state_before={"overlay_items": {"2": {"1": {"type": "image"}}}},
      expect_then_send=[{"method": "GET", "target": "/api/overlay/list"}])
_mg_t("stream-removed", "/api/source/groups/streams/remove", {"result": "ok"}, {"ts_programs": {}},
      request={"group_id": _MG_G, "stream_id": "7faa9bc2"},
      state_before={"ts_programs": {"7faa9bc2": {"selected": 3}}},
      expect_then_send=[{"method": "GET", "target": "/api/source/groups/list", "body": '{"is_need_stream":true}'}])

# ── Telemetry: read-back after writes ────────────────────────────────────
_MG_OKB = {"result": "ok"}
_MG_O1 = {"method": "GET", "target": "/api/output/get?output_id=1"}
_MG_O2 = {"method": "GET", "target": "/api/output/get?output_id=2"}
_MG_GROUPS = {"method": "GET", "target": "/api/source/groups/list", "body": '{"is_need_stream":true}'}
_MG_OVL = {"method": "GET", "target": "/api/overlay/list"}
for _mg_name, _mg_path, _mg_sends, _mg_request in [
    ("readback-users", "/api/users/modify", [{"method": "GET", "target": "/api/users/list"}], None),
    ("readback-roles", "/api/roles/permupdate",
     [{"method": "GET", "target": "/api/roles/permissions"}, {"method": "GET", "target": "/api/users/list"}], None),
    ("readback-overlay", "/api/overlay/rename", [_MG_OVL], None),
    ("readback-overlay-select", "/api/overlay/select",
     [_MG_OVL, {"method": "GET", "target": "/api/overlay/preview/get", "body": '{"output_id":1}'},
      {"method": "GET", "target": "/api/overlay/preview/get", "body": '{"output_id":2}'}], None),
    ("readback-overlay-items", "/api/overlay/items/add",
     [{"method": "GET", "target": "/api/overlay/items/get?overlay_id=4"}, _MG_OVL], {"overlay_id": 4, "type": "text"}),
    ("readback-overlay-images", "/api/overlay/images/remove",
     [{"method": "GET", "target": "/api/overlay/images/list"}], None),
    ("readback-sources", "/api/source/groups/streams/modify", [_MG_GROUPS], None),
    ("readback-config-import", "/api/sys/config/import", [_MG_GROUPS], None),
    ("readback-ts-program", "/api/output/ts/program/set",
     [{"method": "GET", "target": "/api/output/ts/program/get?stream_id=7faa9bc2"}, _MG_GROUPS],
     {"stream_id": "7faa9bc2", "selected_id": 3}),
    ("readback-layout", "/api/layout/select",
     [{"method": "GET", "target": "/api/layout/list"}, {"method": "GET", "target": "/api/output/list"},
      _MG_O1, _MG_O2], None),
    ("readback-window", "/api/output/source/set",
     [_MG_O1, _MG_O2, {"method": "GET", "target": "/api/output/list"},
      {"method": "GET", "target": "/api/layout/list"}], None),
    ("readback-resolution", "/api/output/resolution/set",
     [{"method": "GET", "target": "/api/output/list"},
      {"method": "GET", "target": "/api/output/encode/get?output_id=1"},
      {"method": "GET", "target": "/api/output/encode/get?output_id=2"}], None),
    ("readback-interface", "/api/output/interfaces/set",
     [{"method": "GET", "target": "/api/output/interfaces/get?output_id=2&type=audio"}],
     {"output_id": "2", "type": "audio", "intf_id": 1}),
    ("readback-audio-mix", "/api/output/audiomix/set",
     [{"method": "GET", "target": "/api/output/audiomix/get?output_id=1"}], {"output_id": "1", "stream_id": "a1"}),
    ("readback-border", "/api/output/border/set",
     [{"method": "GET", "target": "/api/output/border/get?output_id=2"}], {"output_id": "2"}),
    ("readback-background", "/api/output/background/set",
     [{"method": "GET", "target": "/api/output/background/get?output_id=1"}], {"output_id": "1"}),
    ("readback-encoder", "/api/output/encode/set",
     [{"method": "GET", "target": "/api/output/encode/get?output_id=2"}], {"output_id": 2, "main": {}}),
    ("readback-mute", "/api/output/mute/set",
     [_MG_O1, {"method": "GET", "target": "/api/output/list"}], {"output_id": 1, "pos_id": 0, "mute": True}),
    ("readback-vu-meter", "/api/output/vumeter/set",
     [_MG_O2, {"method": "GET", "target": "/api/output/list"}], {"enable": False, "output_id": "2", "pos_id": -1}),
    ("readback-gateway", "/api/gate/stream/batch_enable", [{"method": "GET", "target": "/api/gate/stream/list"}], None),
    ("readback-encoded-preview", "/api/gate/stream/multi_out_enable",
     [{"method": "GET", "target": "/api/gate/stream/multi_out_query"}], None),
    ("readback-preview", "/api/preview/source/remove",
     [{"method": "GET", "target": "/api/preview/get"},
      {"method": "GET", "target": "/api/output/audiomix/get?output_id=1"},
      {"method": "GET", "target": "/api/output/audiomix/get?output_id=2"}], None),
    ("readback-network", "/api/network/set", [{"method": "GET", "target": "/api/network/get"}], None),
    ("readback-routes", "/api/network/routing/del", [{"method": "GET", "target": "/api/network/routing/get"}], None),
    ("readback-http", "/api/network/http/set", [{"method": "GET", "target": "/api/network/http/get"}], None),
    ("readback-time", "/api/timeregion/timezone/set", [{"method": "GET", "target": "/api/timeregion/settings"}], None),
    ("readback-device-name", "/api/sys/device/set", [{"method": "GET", "target": "/api/sys/device/get"}], None),
    ("readback-no-signal", "/api/sys/preset/set", [{"method": "GET", "target": "/api/sys/preset/get"}], None),
    ("readback-ndi-discovery", "/api/ndi/discovery/server/del",
     [{"method": "GET", "target": "/api/ndi/discovery/all"},
      {"method": "GET", "target": "/api/ndi/discovery/sources?dis_method=1&dis_g_name=&dis_s_ip="}], None),
]:
    _mg_t(_mg_name, _mg_path, _MG_OKB, {}, request=_mg_request, expect_then_send=_mg_sends)
_mg_t("readback-not-on-error", "/api/layout/select", {"result": "error", "msg": "105"}, {}, expect_then_send=[])

# The session: with the password setting set (admin by default), a command's
# first request is the login (section 3.1), and its token goes in the app
# header from then on (the engine's session tests cover the header and the
# login again on code 2). The other vectors leave the password empty, as for
# a gateway with authentication off, so each command's own request is the
# one compared.
V.append({"spec": _MG_, "command": "list_users", "input": {}, "file": "list_users-login",
          "settings": {"username": "admin", "password": "admin"},
          "expect_request": {"method": "POST", "target": "/api/users/login",
                             "body": '{"username":"admin","password":"admin"}', "content_type": "application/json"}})
for _mg_v in V:
    if _mg_v["spec"] == _MG_ and _mg_v.get("file") != "list_users-login":
        _mg_v.setdefault("settings", {}).setdefault("password", "")
