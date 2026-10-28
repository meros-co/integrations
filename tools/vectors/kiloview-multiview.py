# Kiloview Multiview Pro: /api/v2 (Multiview Pro API V_1.10, Chinese).
# Request bodies and replies follow the document's examples.
KM = "kiloview-multiview"
_KM_OK = {"status": 200, "body": '{"result":"ok"}'}
_KM_ACK = {"ok": {"kind": "ack"}}


def _km(command, input, method, target, body=None, **extra):
    req = {"method": method, "target": target}
    if body is not None:
        req["body"] = body
        req["content_type"] = "application/json"
    V.append({"spec": KM, "command": command, "input": input, "expect_request": req, **extra})


def _km_post(command, input, path, body, **extra):
    _km(command, input, "POST", "/api/v2" + path, body, **extra)


_KM_RES = {"width": 1920, "height": 1080, "resolution": 2, "aspect": 1.777, "frame_rate": 60}
_KM_RESJ = '"resolution":{"width":1920,"height":1080,"index":2,"aspect":1.777},"frameRate":60'
_KM_PW = {"program": "1", "window": "1"}

# 3 Login.
_km_post("login", {"username": "admin", "password": "Admin123"}, "/users/login.json",
         '{"username":"admin","password":"Admin123"}',
         http_reply={"status": 200, "body": '{"data":{"alias":"admin","changed":true,"role":"admin",'
                                            '"token":"8731f577dbc347671b708d41169413dd","username":"admin"},"result":"ok"}'},
         expect_result={"ok": {"kind": "value", "value": "8731f577dbc347671b708d41169413dd"}})
_km_post("login", {"username": "admin", "password": "x"}, "/users/login.json", '{"username":"admin","password":"x"}',
         file="login.error", http_reply={"status": 200, "body": '{"result":"error","msg":"password error"}'},
         expect_result={"error": {"error": "device_error"}})
# 4 Global configuration.
_km("get_general", {}, "GET", "/api/v2/config/general.json")
_km("get_nav", {}, "GET", "/api/v2/config/nav/nav")
_km("get_resolutions", {}, "GET", "/api/v2/resolution/list.json")
_km("get_frame_rates", {}, "GET", "/api/v2/frameRate/list.json",
    http_reply={"status": 200, "body": '{"result":"ok","data":[60,50,40]}'},
    expect_result={"ok": {"kind": "value", "value": [60, 50, 40]}})
_km_post("set_language", {"language": "en"}, "/lang/modify.json", '{"lang":"en"}', http_reply=_KM_OK,
         expect_result=_KM_ACK)
# 5 Programs.
_km("get_programs", {}, "GET", "/api/v2/program/list.json")
_km_post("add_program", dict(_KM_RES, name="programma A"), "/program/add.json", '{"name":"programma A",' + _KM_RESJ + "}")
_km_post("rename_program", {"program": "1", "name": "programma A"}, "/program/rername.json",
         '{"pro_id":"1","name":"programma A"}')
_km_post("remove_program", {"program": "1"}, "/program/remove.json", '{"pro_id":"1"}')
_km_post("set_program_on", {"program": "1", "enabled": True}, "/program/set_status.json", '{"pro_id":"1","status":true}')
_km_post("switch_program", {"program": "1"}, "/program/switch.json", '{"pro_id":"1"}')
_km("get_program", {"program": "1"}, "GET", "/api/v2/program/get_param.json?pro_id=1")
_km_post("set_program", dict(_KM_RES, program="1", name="programma A"), "/program/set_param.json",
         '{"pro_id":"1","name":"programma A",' + _KM_RESJ + "}")
# 6 Source discovery.
_km("get_discovery_groups", {}, "GET", "/api/v2/source/list.json")
_km_post("add_auto_discovery", {"name": "Auto 1", "server_enabled": True, "server_ip": "192.168.1.1",
                                "groups": ["kiloview", "NDI", "test"]}, "/source/add.json",
         '{"disc_name":"Auto 1","disc_type":"Auto","discovery_enable":true,"discovery_ip":"192.168.1.1",'
         '"group":["kiloview","NDI","test"]}')
_km_post("add_manual_discovery", {"name": "Manual 1", "groups": ["kiloview"], "ips": ["192.168.40.11", "192.168.40.12"]},
         "/source/add.json",
         '{"disc_name":"Manual 1","disc_type":"Manual","group":["kiloview"],"ip":["192.168.40.11","192.168.40.12"]}')
_km_post("modify_auto_discovery", {"discovery": "1", "name": "Auto 1", "groups": ["public"]}, "/source/modify.json",
         '{"disc_id":"1","disc_name":"Auto 1","disc_type":"Auto","discovery_enable":false,"discovery_ip":"",'
         '"group":["public"]}')
_km_post("modify_manual_discovery", {"discovery": "2", "name": "Manual 1", "groups": [], "ips": ["10.0.0.5"]},
         "/source/modify.json", '{"disc_id":"2","disc_name":"Manual 1","disc_type":"Manual","group":[],"ip":["10.0.0.5"]}')
_km("get_discovery", {"discovery": "1"}, "GET", "/api/v2/source/get_param.json?disc_id=1")
_km_post("remove_discovery", {"discovery": "1"}, "/source/remove.json", '{"disc_id":"1"}')
_km_post("open_source_webpage", {"url": "http://192.168.40.9/"}, "/webSite/set.json", '{"url":"http://192.168.40.9/"}')
_km_post("refresh_sources", {}, "/source/refresh.json", '{"refresh":true}')
# 6 Windows.
_km("get_windows", {"program": "1"}, "GET", "/api/v2/window/list.json?pro_id=1")
_km_post("switch_window", {"program": "1", "window": "2", "session": "1"}, "/window/switch.json",
         '{"pro_id":"1","win_id":"2","session_id":"1"}')
_km_post("add_window", dict(_KM_RES, program="1", window="1", name="window_1", numbered=True), "/window/add.json",
         '{"pro_id":"1","win_id":"1","name":"window_1",' + _KM_RESJ +
         ',"number_used":true,"number_range":{"min":1,"max":16}}')
_km_post("set_window", dict(_KM_RES, program="1", window="1", name="window_1"), "/window/set_param.json",
         '{"pro_id":"1","win_id":"1","name":"window_1",' + _KM_RESJ +
         ',"number_used":false,"number_range":{"min":1,"max":16}}')
_km_post("remove_window", _KM_PW, "/window/remove.json", '{"pro_id":"1","win_id":"1"}')
_km("get_output_types", {}, "GET", "/api/v2/window/get_output_list.json")
_km("get_screens", {}, "GET", "/api/v2/window/get_screen_list.json")
_km("get_window", _KM_PW, "GET", "/api/v2/window/get.json?pro_id=1&win_id=1")
_km("get_window_screen", {"program": "1", "window": "pvw"}, "GET", "/api/v2/window/get_position.json?pro_id=1&win_id=pvw")
_km_post("set_window_screen", dict(_KM_PW, screen=1), "/window/set_position.json", '{"pro_id":"1","win_id":"1","select":1}')
_km("get_window_output", _KM_PW, "GET", "/api/v2/window/get_output_type.json?pro_id=1&win_id=1")
_km_post("set_window_output", dict(_KM_PW, ndi=True), "/window/set_output_type.json",
         '{"pro_id":"1","win_id":"1","select":"NDI"}')
_km_post("set_window_output", dict(_KM_PW, ndi=False), "/window/set_output_type.json",
         '{"pro_id":"1","win_id":"1","select":""}', file="set_window_output.off")
_km("get_window_ndi", _KM_PW, "GET", "/api/v2/window/ndi/get_param.json?pro_id=1&win_id=1")
_km_post("set_window_ndi", dict(_KM_PW, groups=["public"], server_enabled=True, server_ip="192.168.1.1"),
         "/window/ndi/set_param.json",
         '{"pro_id":"1","win_id":"1","connection_type":"Default","netprefix":"","netmask":"","ttl":1,'
         '"group":["public"],"discovery_enable":true,"discovery_ip":"192.168.1.1"}')
_km_post("clear_window_sources", _KM_PW, "/window/delete_sources.json", '{"pro_id":"1","win_id":"1"}')
_km("get_toolbar", _KM_PW, "GET", "/api/v2/window/get_state_control.json?pro_id=1&win_id=1")
_km_post("set_toolbar", dict(_KM_PW, item="vu_meter", enabled=True), "/window/set_state_control.json",
         '{"pro_id":"1","win_id":"1","type":"vu_meter","config":{"enable":true}}')
_km_post("set_toolbar_font", dict(_KM_PW, color="#fff", size=12), "/window/set_state_control.json",
         '{"pro_id":"1","win_id":"1","type":"font","config":{"color":"#fff","size":12}}')
_km_post("set_toolbar_border", dict(_KM_PW, enabled=False, color="#fff"), "/window/set_state_control.json",
         '{"pro_id":"1","win_id":"1","type":"border","config":{"enable":false,"color":"#fff"}}')
_km_post("set_toolbar_audio_gain", dict(_KM_PW, gain=10), "/window/set_state_control.json",
         '{"pro_id":"1","win_id":"1","type":"audio_gain","config":{"value":10}}')
_km_post("set_window_layout", dict(_KM_PW, layout="1"), "/window/set_layout.json",
         '{"pro_id":"1","win_id":"1","layout_id":"1"}')
_km_post("reload_window_layout", dict(_KM_PW, layout="1"), "/window/reload_layout.json",
         '{"pro_id":"1","win_id":"1","layout_id":"1"}')
_km_post("set_pane_source", {"from_program": "1", "from_window": "1", "from_pane": 1, "program": "1", "window": "2",
                             "pane": 2, "source": ""}, "/window/set_source.json",
         '{"from":{"pro_id":"1","win_id":"1","pos_id":1},"to":{"pro_id":"1","win_id":"2","pos_id":2,"stream_id":""}}')
_km_post("remove_pane_source", {"program": "1", "window": "2", "pane": 2}, "/window/remove_source.json",
         '{"pro_id":"1","win_id":"2","pos_id":2}')
_km("get_pane", dict(_KM_PW, pane=1), "POST", "/api/v2/window/get_single_layout.json?pro_id=1&win_id=1&pos_id=1")
_km_post("set_pane", dict(_KM_PW, pane=1, item="name_enable", enabled=True), "/window/set_single_layout.json",
         '{"pro_id":"1","win_id":"1","pos_id":1,"type":"name_enable","config":{"enable":true}}')
_km_post("set_pane_name", dict(_KM_PW, pane=1, name="Camera 1"), "/window/set_single_layout.json",
         '{"pro_id":"1","win_id":"1","pos_id":1,"type":"name","config":{"pattern_name":"Camera 1"}}')
_km_post("set_pane_audio", dict(_KM_PW, pane=1, mix="always"), "/window/set_audio_type.json",
         '{"pro_id":"1","win_id":"1","pos_id":1,"mix_type":"always"}')
# 7 Layouts.
_km("get_layout_templates", {}, "GET", "/api/v2/layout/template.json")
_km("get_layouts", {"program": "1"}, "GET", "/api/v2/layout/list.json?pro_id=1")
_km_post("add_layout", {"program": "1", "name": "123", "template": "1"}, "/layout/add.json",
         '{"pro_id":"1","name":"123","template_id":"1"}')
_km_post("rename_layout", {"program": "1", "layout": "1", "name": "Wide"}, "/layout/rename.json",
         '{"pro_id":"1","layout_id":"1","name":"Wide"}')
_KM_POS = [{"y": 0, "x": 0, "height": 100, "width": 100, "type": "", "number": "1", "id": 1, "z_index": 0}]
_km_post("modify_layout", dict(_KM_PW, layout="1", name="123", panes=_KM_POS, count=1), "/layout/modify.json",
         '{"pro_id":"1","win_id":"1","layout_id":"1","layout_count":1,"name":"123","position":'
         + json.dumps(_KM_POS, separators=(",", ":")) + "}")
_km_post("remove_layout", {"program": "1", "layout": "2"}, "/layout/remove.json", '{"pro_id":"1","layout_id":"2"}')
_km_post("save_layout_as", {"program": "1", "layout": "1", "name": "Copy"}, "/layout/resave.json",
         '{"pro_id":"1","layout_id":"1","name":"Copy"}')
# 8 NDI discovery.
_km("get_ndi_groups", {}, "GET", "/api/discovery/scan/groupChecked.json")
_km("get_manual_ips", {}, "GET", "/api/discovery/scan/getManualIps.json")
_km("set_ndi_discovery", {"groups": ["kiloview", "newtek"], "ips": ["192.168.2.52", "192.168.2.33"]}, "POST",
    "/api/discovery/scan/addManualIp.json",
    '{"group_name":["kiloview","newtek"],"ip":["192.168.2.52","192.168.2.33"]}')
# 9 Others.
_km("get_window_sdp", dict(_KM_PW, ip="192.168.42.156"), "GET",
    "/api/v2/webrtc/get_sdp.json?pro_id=1&win_id=1&ip=192.168.42.156")
_KM_PP = {"program": "1", "window": "pvw", "pane": 1}
_KM_PPJ = '"pro_id":"1","win_id":"pvw","pos_id":1'
_km_post("ptz", dict(_KM_PP, type="ptz_zoom", data={"zoom_value": 0.5}), "/ptz/set.json",
         "{" + _KM_PPJ + ',"type":"ptz_zoom","data":{"zoom_value":0.5}}')
_km_post("ptz_zoom", dict(_KM_PP, zoom=0.5), "/ptz/set.json", "{" + _KM_PPJ + ',"type":"ptz_zoom","data":{"zoom_value":0.500}}')
_km_post("ptz_zoom_speed", dict(_KM_PP, speed=-1.0), "/ptz/set.json",
         "{" + _KM_PPJ + ',"type":"ptz_zoom_speed","data":{"zoom_speed":-1.000}}')
_km_post("ptz_pan_tilt", dict(_KM_PP, pan=-1.0, tilt=0.25), "/ptz/set.json",
         "{" + _KM_PPJ + ',"type":"ptz_pan_tilt","data":{"pan_value":-1.000,"tilt_value":0.250}}')
_km_post("ptz_pan_tilt_speed", dict(_KM_PP, pan_speed=0.0, tilt_speed=1.0), "/ptz/set.json",
         "{" + _KM_PPJ + ',"type":"ptz_pan_tilt_speed","data":{"pan_speed":0.000,"tilt_speed":1.000}}')
_km_post("ptz_store_preset", dict(_KM_PP, preset=3), "/ptz/set.json",
         "{" + _KM_PPJ + ',"type":"ptz_store_preset","data":{"preset_no":3}}')
_km_post("ptz_recall_preset", dict(_KM_PP, preset=3, speed=0.5), "/ptz/set.json",
         "{" + _KM_PPJ + ',"type":"ptz_recall_preset","data":{"preset_no":3,"speed":0.500}}')
_km_post("ptz_focus", dict(_KM_PP, focus=1.0), "/ptz/set.json", "{" + _KM_PPJ + ',"type":"ptz_focus","data":{"focus_value":1.000}}')
_km_post("ptz_focus_speed", dict(_KM_PP, speed=0.5), "/ptz/set.json",
         "{" + _KM_PPJ + ',"type":"ptz_focus_speed","data":{"focus_speed":0.500}}')
_km_post("ptz_auto_focus", _KM_PP, "/ptz/set.json", "{" + _KM_PPJ + ',"type":"ptz_auto_focus","data":{}}')
_km_post("ptz_white_balance", dict(_KM_PP, mode="oneshot"), "/ptz/set.json",
         "{" + _KM_PPJ + ',"type":"ptz_white_balance_oneshot","data":{}}')
_km_post("ptz_white_balance_manual", dict(_KM_PP, red=0.1, blue=0.9), "/ptz/set.json",
         "{" + _KM_PPJ + ',"type":"ptz_white_balance_manual","data":{"red":0.100,"blue":0.900}}')
_km_post("ptz_exposure_auto", _KM_PP, "/ptz/set.json", "{" + _KM_PPJ + ',"type":"ptz_exposure_auto","data":{}}')
_km_post("ptz_exposure_manual", dict(_KM_PP, level=0.75), "/ptz/set.json",
         "{" + _KM_PPJ + ',"type":"ptz_exposure_manual","data":{"exposure_level":0.750}}')
_km_post("ptz_is_supported", _KM_PP, "/ptz/set.json", "{" + _KM_PPJ + ',"type":"ptz_is_supported","data":{}}',
         http_reply={"status": 200, "body": '{"result":"error"}'}, expect_result={"error": {"error": "device_error"}})
_km("get_transitions", {}, "GET", "/api/v2/take/transition/list.json")
_km_post("take", {"program": "1", "time": 10}, "/take/set.json", '{"pro_id":"1","time":10,"progress":1.0,"select":1}')
_km_post("tbar", {"program": "1", "progress": 0.4, "transition": 2}, "/take/set.json",
         '{"pro_id":"1","time":0,"progress":0.400,"select":2}')
_km("get_take", {"program": "1"}, "POST", "/api/v2/take/get.json?pro_id=1")
_km_post("set_take", {"program": "1", "time": 3}, "/take/set_params.json", '{"pro_id":"1","time":3,"select":1}')


# ── Telemetry ───────────────────────────────────────────────────────────
def _km_tel(name, path, data, expect_state, request=None, **extra):
    body = json.dumps({"result": "ok", "data": data}, separators=(",", ":"), ensure_ascii=False)
    inbound = {"path": path, "body": body}
    if request is not None:
        inbound["request"] = request
    telemetry(KM, name, inbound_http=inbound, expect_state=expect_state, **extra)


_km_tel("general", "/api/v2/config/general.json", {
    "deviceName": "NDI Multiview", "languageShow": True, "websocketPort": 12345, "qrcode": "https://192.168.40.215:81/",
    "loginImgUrl": "./img/logo.png", "logoImgShow": True,
    "languageOptions": [{"value": "en", "name": "English"}], "version": "V_1.10.0001"},
    {"general": {"device_name": "NDI Multiview", "version": "V_1.10.0001", "websocket_port": 12345,
                 "qrcode": "https://192.168.40.215:81/", "language_switch": True, "logo_shown": True,
                 "languages": '[{"value":"en","name":"English"}]'}})
_km_tel("nav", "/api/v2/config/nav/nav", {"main_nav": {"home": True}}, {"general": {"nav_shown": True}})
_km_tel("resolutions", "/api/v2/resolution/list.json", [{"id": 1, "label": "1280x720", "width": 1280, "height": 720}],
        {"resolutions": {"1": {"label": "1280x720", "width": 1280, "height": 720}}},
        state_before={"resolutions": {"9": {"label": "old"}}})
_km_tel("resolutions-value", "/api/v2/resolution/list.json",
        [{"value": 2, "label": "1080p", "width": 1920, "height": 1080}],
        {"resolutions": {"2": {"label": "1080p", "width": 1920, "height": 1080}}})
_km_tel("frame-rates", "/api/v2/frameRate/list.json", [60, 50, 40], {"frame_rates": "[60,50,40]"})
_km_tel("transitions", "/api/v2/take/transition/list.json", [{"id": 1, "label": "fade"}, {"id": 4, "label": "wipeRight"}],
        {"transitions": {"1": "fade", "4": "wipeRight"}})
_km_tel("output-types", "/api/v2/window/get_output_list.json", [{"id": 1, "label": "NDI"}], {"output_types": {"1": "NDI"}})
_km_tel("screens", "/api/v2/window/get_screen_list.json", [{"id": 1, "label": "Screen 1"}], {"screens": {"1": "Screen 1"}})
_km_tel("programs", "/api/v2/program/list.json", [{"pro_id": "1", "name": "program A", "status": True}],
        {"programs": {"1": {"name": "program A", "on": True}}},
        state_before={"programs": {"2": {"name": "gone"}}},
        expect_then_send=[{"method": "GET", "target": "/api/v2/window/list.json?pro_id=1"},
                          {"method": "GET", "target": "/api/v2/layout/list.json?pro_id=1"},
                          {"method": "GET", "target": "/api/v2/program/get_param.json?pro_id=1"},
                          {"method": "POST", "target": "/api/v2/take/get.json?pro_id=1"}])
_km_tel("program-params", "/api/v2/program/get_param.json?pro_id=1",
        {"name": "programma A", "resolution": {"width": 1920, "height": 1080, "index": 2, "aspect": 1.777}, "frameRate": 60},
        {"programs": {"1": {"width": 1920, "height": 1080, "resolution": 2, "aspect": 1.777, "frame_rate": 60.0}}})
_km_tel("take", "/api/v2/take/get.json?pro_id=1", {"time": 10, "select": 1},
        {"programs": {"1": {"take_time": 10, "take_transition": 1}}})
_km_tel("windows", "/api/v2/window/list.json?pro_id=1", [
    {"id": "pvw", "name": "PVW", "layout_id": "1", "modified": False,
     "resolution": {"width": 1920, "height": 1080, "index": 2, "aspect": 1.777}, "frameRate": 60}],
    {"windows": {"1": {"pvw": {"name": "PVW", "layout": "1", "modified": False, "width": 1920, "height": 1080,
                               "resolution": 2, "aspect": 1.777, "frame_rate": 60.0}}}},
    state_before={"windows": {"1": {"7": {"name": "gone"}}}},
    expect_then_send=[{"method": "GET", "target": "/api/v2/window/get.json?pro_id=1&win_id=pvw"},
                      {"method": "GET", "target": "/api/v2/window/get_position.json?pro_id=1&win_id=pvw"},
                      {"method": "GET", "target": "/api/v2/window/get_output_type.json?pro_id=1&win_id=pvw"},
                      {"method": "GET", "target": "/api/v2/window/ndi/get_param.json?pro_id=1&win_id=pvw"},
                      {"method": "GET", "target": "/api/v2/window/get_state_control.json?pro_id=1&win_id=pvw"}])
_km_tel("window", "/api/v2/window/get.json?pro_id=1&win_id=1", {
    "layout_id": "3", "layout_count": 1, "layout_name": "4",
    "resolution": {"aspect": 0, "height": 1080, "index": 2, "width": 1920},
    "layout": [{"id": 1, "number": 1, "x": 0, "y": 0, "z_index": 0, "width": 100, "height": 100, "type": "",
                "aspectRatio": "1920x1080", "frameRate": 30, "stream_id": "KILOVIEW (Chan 1)",
                "stream_name": "KILOVIEW (Chan 1)", "pattern_name": "Camera_1",
                "audio_mix": {"mix_type": "follow", "pgm_enable": True, "pvw_enable": False}}]},
    {"windows": {"1": {"1": {"layout_name": "4", "pane_count": 1}}},
     "panes": {"1": {"1": {"1": {"number": 1, "x": 0.0, "y": 0.0, "z": 0.0, "width": 100.0, "height": 100.0, "type": "",
                                 "source_resolution": "1920x1080", "source_frame_rate": 30.0,
                                 "source": "KILOVIEW (Chan 1)", "source_name": "KILOVIEW (Chan 1)",
                                 "name": "Camera_1", "audio_mix": "follow", "on_pgm": True, "on_pvw": False}}}}},
    state_before={"panes": {"1": {"1": {"2": {"number": 2}}}}})
_km_tel("window-screen", "/api/v2/window/get_position.json?pro_id=1&win_id=1", {"select": 1},
        {"windows": {"1": {"1": {"screen": 1}}}})
_km_tel("window-output", "/api/v2/window/get_output_type.json?pro_id=1&win_id=1", {"select": "NDI"},
        {"windows": {"1": {"1": {"ndi_output": True}}}})
_km_tel("window-output-off", "/api/v2/window/get_output_type.json?pro_id=1&win_id=1", {"select": ""},
        {"windows": {"1": {"1": {"ndi_output": False}}}})
_km_tel("window-ndi", "/api/v2/window/ndi/get_param.json?pro_id=1&win_id=1", {
    "pro_id": "1", "win_id": "1", "connection_type": "Default", "netprefix": "", "netmask": "", "ttl": 1,
    "group": ["public"], "discovery_enable": True, "discovery_ip": "192.168.1.1"},
    {"windows": {"1": {"1": {"ndi": {"connection": "Default", "netprefix": "", "netmask": "", "ttl": 1,
                                     "groups": '["public"]', "server_enabled": True, "server_ip": "192.168.1.1"}}}}})
_km_tel("toolbar", "/api/v2/window/get_state_control.json?pro_id=1&win_id=1", {
    "name_enable": False, "safe_area": False, "center_cross": False, "vu_meter": True,
    "font": {"color": "#ddd", "size": 12}, "border": {"enable": False, "color": "#fff"}, "audio_gain": 10},
    {"windows": {"1": {"1": {"toolbar": {"names": False, "safe_area": False, "center_cross": False, "vu_meter": True,
                                         "font_color": "#ddd", "font_size": 12, "border": False,
                                         "border_color": "#fff", "audio_gain": 10}}}}})
_km_tel("pane", "/api/v2/window/get_single_layout.json?pro_id=1&win_id=1&pos_id=1", {
    "pattern_name": "", "safe_area": True, "vu_meter": True, "center_cross": False, "name_enable": False},
    {"panes": {"1": {"1": {"1": {"name": "", "safe_area": True, "vu_meter": True, "center_cross": False,
                                 "name_shown": False}}}}})
_KM_LPOS = [{"x": 0, "y": 0, "width": 100, "height": 100, "type": "", "number": "1", "id": 1, "z_index": 0}]
_km_tel("layouts", "/api/v2/layout/list.json?pro_id=1",
        [{"layout_id": "1", "layout_count": 1, "name": "PIP", "used": True, "position": _KM_LPOS}],
        {"layouts": {"1": {"1": {"name": "PIP", "pane_count": 1, "used": True,
                                 "panes": json.dumps(_KM_LPOS, separators=(",", ":"))}}}},
        state_before={"layouts": {"1": {"5": {"name": "gone"}}}})
_km_tel("templates", "/api/v2/layout/template.json",
        [{"template_id": "1", "template_count": 1, "name": "picInPic", "position": _KM_LPOS}],
        {"layout_templates": {"1": {"name": "picInPic", "pane_count": 1,
                                    "panes": json.dumps(_KM_LPOS, separators=(",", ":"))}}})
_km_tel("sources", "/api/v2/source/list.json", [{"disc_id": "1", "disc_name": "Auto 1", "sources": [
    {"id": "N4-19162010064 (Channel-1)", "name": "N4-19162010064 (Channel-1)", "ip": "192.168.2.33",
     "url": "192.168.2.33:8080", "web_url": ""}]}],
    {"discovery": {"1": {"name": "Auto 1", "sources": {"N4-19162010064 (Channel-1)": {
        "name": "N4-19162010064 (Channel-1)", "ip": "192.168.2.33", "url": "192.168.2.33:8080", "web_url": ""}}}}},
    state_before={"discovery": {"3": {"name": "gone"}}},
    expect_then_send=[{"method": "GET", "target": "/api/v2/source/get_param.json?disc_id=1"}])
_km_tel("discovery-params", "/api/v2/source/get_param.json?disc_id=1", {
    "disc_id": "1", "disc_name": "Auto 1", "disc_type": "Auto", "discovery_enable": True,
    "discovery_ip": "192.168.1.1", "group": ["kiloview", "NDI"]},
    {"discovery": {"1": {"type": "Auto", "server_enabled": True, "server_ip": "192.168.1.1",
                         "groups": '["kiloview","NDI"]'}}})
_km_tel("ndi-groups", "/api/discovery/scan/groupChecked.json", ["kiloview", "NDI", "test"],
        {"ndi_discovery": {"groups": '["kiloview","NDI","test"]'}})
_km_tel("manual-ips", "/api/discovery/scan/getManualIps.json", ["192.168.2.52"],
        {"ndi_discovery": {"manual_ips": '["192.168.2.52"]'}})


def _km_reread(name, path, request, then):
    telemetry(KM, name, inbound_http={"path": path, "body": '{"result":"ok"}', "request": request},
              expect_state={}, expect_then_send=then)


def _km_g(t):
    return {"method": "GET", "target": t}


_km_reread("reread-program", "/api/v2/program/set_status.json", {"pro_id": "1", "status": True},
           [_km_g("/api/v2/program/list.json")])
_km_reread("reread-source", "/api/v2/source/add.json", {"disc_name": "x"}, [_km_g("/api/v2/source/list.json")])
_km_reread("reread-window", "/api/v2/window/set_position.json", {"pro_id": "2", "win_id": "1", "select": 1},
           [_km_g("/api/v2/window/list.json?pro_id=2"), _km_g("/api/v2/layout/list.json?pro_id=2")])
_km_reread("reread-set-source", "/api/v2/window/set_source.json",
           {"from": {"pro_id": "1", "win_id": "1", "pos_id": 1}, "to": {"pro_id": "3", "win_id": "2", "pos_id": 2}},
           [_km_g("/api/v2/window/list.json?pro_id=3")])
_km_reread("reread-pane", "/api/v2/window/set_single_layout.json", {"pro_id": "1", "win_id": "pvw", "pos_id": 4},
           [{"method": "POST", "target": "/api/v2/window/get_single_layout.json?pro_id=1&win_id=pvw&pos_id=4"},
            _km_g("/api/v2/window/get.json?pro_id=1&win_id=pvw")])
_km_reread("reread-layout", "/api/v2/layout/add.json", {"pro_id": "1", "name": "x"},
           [_km_g("/api/v2/layout/list.json?pro_id=1"), _km_g("/api/v2/window/list.json?pro_id=1")])
_km_reread("reread-take", "/api/v2/take/set.json", {"pro_id": "1", "time": 1},
           [{"method": "POST", "target": "/api/v2/take/get.json?pro_id=1"}, _km_g("/api/v2/window/list.json?pro_id=1")])
_km_reread("reread-ndi-discovery", "/api/discovery/scan/addManualIp.json", {"ip": []},
           [_km_g("/api/discovery/scan/groupChecked.json"), _km_g("/api/discovery/scan/getManualIps.json")])
