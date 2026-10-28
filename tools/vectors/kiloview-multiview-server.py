# Kiloview Multiview Server: HTTP API 1.0 on port 65532 (Multiview Server API
# Version 1.0). Bodies and replies follow the document's examples.
KMS = "kiloview-multiview-server"
_KMS_OK = {"status": 200, "body": '{"result":"ok"}'}


def _kms(command, input, method, target, body=None, **extra):
    req = {"method": method, "target": target}
    if body is not None:
        req["body"] = body
        req["content_type"] = "application/json"
    V.append({"spec": KMS, "command": command, "input": input, "expect_request": req, **extra})


_kms("get_general", {}, "GET", "/api/config/general/get.json")
_kms("get_nav", {}, "GET", "/api/config/nav/nav.json")
_kms("get_layouts", {}, "GET", "/api/output/icon.json")
_kms("get_layout_templates", {}, "GET", "/api/pattern/template.json")
_KMS_G = [{"y": 0, "x": 0, "height": 100, "width": 100}, {"y": 69, "x": 69, "height": 30, "width": 30}]
_KMS_GJ = json.dumps(_KMS_G, separators=(",", ":"))
_kms("add_layout", {"layout": "11", "name": "123", "grids": _KMS_G}, "POST", "/api/pattern/add.json",
     '{"id":"11","name":"123","data":' + _KMS_GJ + "}", http_reply=_KMS_OK, expect_result={"ok": {"kind": "ack"}})
_kms("remove_layout", {"layout": 1}, "GET", "/api/pattern/remove.json?id=1")
_kms("modify_layout", {"layout": "1", "name": "123", "grids": _KMS_G}, "POST", "/api/pattern/modify.json",
     '{"id":"1","name":"123","data":' + _KMS_GJ + "}")
_kms("get_windows", {}, "GET", "/api/output/tabs_get.json")
_kms("add_window", {}, "GET", "/api/window/new.json")
_kms("rename_window", {"window": 4, "name": "output 4"}, "POST", "/api/output/tabs_modify.json",
     '{"id":4,"name":"output 4"}')
_kms("select_window", {"window": 2}, "POST", "/api/output/tabs_modify.json", '{"id":2}')
_kms("close_window", {"window": 1}, "POST", "/api/window/close_window.json", '{"output":"1"}')
_kms("get_screens", {}, "GET", "/api/window/get_screen_list.json")
_kms("get_window_screen", {"window": 1}, "GET", "/api/window/get_window_position.json?output=1")
_kms("set_window_screen", {"window": 1, "screen": 3, "full_screen": True, "shown": False}, "POST",
     "/api/window/set_window_position.json", '{"output":"1","screen":3,"isFull":true,"isShow":false}')
_kms("get_window", {"window": 1}, "GET", "/api/output/get.json?output=1")
_kms("set_window_layout", {"window": 1, "action": "change", "layout": 4, "layout_number": 4}, "POST",
     "/api/output/layout.json", '{"action":"change","output":1,"position":0,"layout_id":4,"layout_number":4}')
_kms("clear_window_sources", {"window": 1}, "POST", "/api/output/delete_all.json", '{"output":1}')
_kms("get_toolbar", {"window": 1}, "GET", "/api/output/get_state_contol.json?output=1")
_kms("set_toolbar", {"window": 1, "item": "centerline", "enabled": True}, "POST", "/api/output/set_state_contol.json",
     '{"output":"1","type":"centerline","enable":1}')
_kms("get_border", {}, "GET", "/api/output/GetEnableBorder.json")
_kms("set_border", {"color": 16777215, "enabled": True}, "POST", "/api/output/EnableBorder.json",
     '{"bordercolor":16777215,"bordercolor_enabled":1}')
_kms("get_audio_gain", {}, "GET", "/api/audio/getGlobalGain.json")
_kms("set_audio_gain", {"window": 1, "gain": "0"}, "POST", "/api/audio/setGlobalGain.json", '{"output":1,"audioGain":0}')
_kms("set_grid_audio", {"window": 1, "grid": 1, "audio": "off"}, "POST", "/api/output/window_audio_set.json",
     '{"output":"1","id":1,"volume":"off"}')
_kms("get_stream_mode", {"window": 1, "grid": 1}, "GET", "/api/output/get_stream_mode.json?output=1&id=1")
_kms("set_stream_mode", {"grid": 1, "mode": "auto"}, "POST", "/api/output/set_stream_mode.json", '{"id":1,"mode":"auto"}')
_kms("get_grid", {"window": 1, "grid": 1}, "GET", "/api/output/get_single_layout.json?output_id=1&id=1")
_kms("set_grid", {"window": 1, "grid": 1, "name": "HDMI", "size": 16, "color": "#265ddd", "safe_area": True,
                  "audio_meter": True, "center_cross": False}, "POST", "/api/output/set_single_layout.json",
     '{"output":"1","id":1,"name":"HDMI","size":16,"color":"#265ddd","safe":true,"soundColumn":true,"centerline":false}')
_kms("set_grid_tally", {"window": 1, "grid": 2, "tally": "PVW", "enabled": True}, "POST", "/api/output/set_pvw_pgm.json",
     '{"output":"1","id":2,"name":"PVW","enable":1}')
_kms("get_ndi_status", {"window": 2}, "GET", "/api/ndi/get_status.json?output=2")
_kms("set_ndi_status", {"window": 1, "enabled": True}, "POST", "/api/ndi/set_status.json", '{"output":"1","enable":true}',
     http_reply={"status": 200, "body": '{"output":"1","enable":true}'}, expect_result={"ok": {"kind": "ack"}})
_kms("get_take", {"window": 1}, "GET", "/api/ndi/get_take.json?output=1")
_kms("set_take", {"window": 1, "target": 3, "safe_area": True}, "POST", "/api/ndi/set_take.json",
     '{"output":"1","takeTarget":"3","autoTake":false,"centerLine":false,"title":false,"safeFrame":true,'
     '"volumeMeter":false,"border":false,"syncVolume":false}')
_kms("apply_take", {"window": 1}, "POST", "/api/ndi/apply_take.json", '{"output":"1","apply":true}')
_kms("get_ndi_output", {}, "GET", "/api/ndi/get.json")
_kms("set_ndi_output", {"window": 1, "host": "DESKTOP-7TOCUHF", "name": "output1", "resolution": 2, "width": 1920,
                        "height": 1080, "aspect_ratio": 0, "aspect_width": 18, "aspect_height": 9, "frame_rate": 0},
     "POST", "/api/ndi/set.json",
     '{"output":"1","host":"DESKTOP-7TOCUHF","name":"output1","resolution":2,"customResolution":{"width":1920,'
     '"height":1080},"aspectRatio":0,"customAspectRatio":{"width":18,"height":9},"frameRate":0,"customFrameRate":24}')
_kms("get_resolutions", {}, "GET", "/api/ndi/resolution.json")
_kms("get_aspect_ratios", {}, "GET", "/api/ndi/aspectRatio.json")
_kms("get_frame_rates", {}, "GET", "/api/ndi/frameRate.json")
_kms("get_ndi_groups", {}, "GET", "/api/discovery/scan/groupChecked.json")
_kms("get_manual_ips", {}, "GET", "/api/discovery/scan/getManualIps.json")
_kms("set_ndi_discovery", {"groups": ["kiloview", "newtek"], "ips": ["192.168.2.52", "192.168.2.33"]}, "POST",
     "/api/discovery/scan/addManualIp.json",
     '{"group_name":["kiloview","newtek"],"ip":["192.168.2.52","192.168.2.33"]}')
_kms("get_sources", {}, "GET", "/api/source/get.json")
_kms("set_source_preload", {"source": "N4-19162010064 (#00 Channel-1)", "enabled": True}, "POST", "/api/source/modify.json",
     '{"id":"N4-19162010064 (#00 Channel-1)","switch":true}')
_kms("open_source_webpage", {"url": "192.168.2.33"}, "POST", "/api/webSite/set.json", '{"url":"192.168.2.33"}')
_kms("refresh_sources", {}, "GET", "/api/webSite/refresh.json")
_kms("get_window_sdp", {"stream": "1", "ip": "192.168.42.156"}, "GET", "/api/webrtc/get_sdp.json?stream_id=1&ip=192.168.42.156",
     http_reply={"status": 200, "body": '{"result":"ok","data":{"sdp":"v=0"}}'},
     expect_result={"ok": {"kind": "value", "value": {"sdp": "v=0"}}})


def _kms_tel(name, path, data, expect_state, **extra):
    body = json.dumps({"result": "ok", "data": data}, separators=(",", ":"))
    telemetry(KMS, name, inbound_http={"path": path, "body": body}, expect_state=expect_state, **extra)


_kms_tel("general", "/api/config/general/get.json", {
    "deviceName": "NDI Multiview", "languageShow": True, "languageOptions": [{"value": "en", "name": "English"}],
    "logoShow": False, "logoUrl": "", "qrcode": "http://192.168.2.150:8080/index.html"},
    {"general": {"device_name": "NDI Multiview", "language_switch": True, "languages": '[{"value":"en","name":"English"}]',
                 "logo_shown": False, "qrcode": "http://192.168.2.150:8080/index.html"}})
_kms_tel("nav", "/api/config/nav/nav.json", {"main_nav": {"home": True}}, {"general": {"nav_shown": True}})
_KMS_P = [{"x": 0, "y": 0, "width": 100, "height": 100}]
_kms_tel("layouts", "/api/output/icon.json", [{"id": 0, "number": 1, "name": "PIP", "use_output": 1, "position": _KMS_P}],
         {"layouts": {"0": {"name": "PIP", "grid_count": 1, "in_use": True,
                            "grids": json.dumps(_KMS_P, separators=(",", ":"))}}},
         state_before={"layouts": {"5": {"name": "gone"}}})
_kms_tel("templates", "/api/pattern/template.json", [{"id": 1, "name": "picInPic", "conflict": 0, "position": _KMS_P}],
         {"layout_templates": {"1": {"name": "picInPic", "grids": json.dumps(_KMS_P, separators=(",", ":"))}}})
_kms_tel("screens", "/api/window/get_screen_list.json", [{"id": 1, "label": "Screen 1"}], {"screens": {"1": "Screen 1"}})
_kms_tel("border", "/api/output/GetEnableBorder.json", {"bordercolor": 16777215, "bordercolor_enabled": 1},
         {"border": {"color": 16777215, "shown": True}})
_kms_tel("audio-gain", "/api/audio/getGlobalGain.json", {"audioGain": 10}, {"audio_gain": 10})
_kms_tel("ndi-output", "/api/ndi/get.json", {
    "output": "1", "host": "DESKTOP-7TOCUHF", "name": "output1", "resolution": 2,
    "customResolution": {"width": 1920, "height": 1080}, "aspectRatio": 0,
    "customAspectRatio": {"width": 18, "height": 9}, "frameRate": 0, "customFrameRate": 24},
    {"ndi_output": {"window": "1", "host": "DESKTOP-7TOCUHF", "name": "output1", "resolution": 2, "width": 1920,
                    "height": 1080, "aspect_ratio": 0, "aspect_width": 18, "aspect_height": 9, "frame_rate": 0,
                    "custom_frame_rate": 24}})
_kms_tel("sources", "/api/source/get.json", [{"ip": "192.168.254.239", "url": "192.168.254.239:45611",
                                              "stream_id": "Kiloview (Chan 2)", "name": "Kiloview (Chan 2)",
                                              "id": "Kiloview (Chan 2)", "official": False}],
         {"sources": {"Kiloview (Chan 2)": {"ip": "192.168.254.239", "url": "192.168.254.239:45611", "official": False}}},
         state_before={"sources": {"Old (1)": {"ip": "10.0.0.1"}}})
_kms_tel("ndi-groups", "/api/discovery/scan/groupChecked.json", ["kiloview", "NDI"],
         {"ndi_discovery": {"groups": '["kiloview","NDI"]'}})
_kms_tel("manual-ips", "/api/discovery/scan/getManualIps.json", ["192.168.2.52"],
         {"ndi_discovery": {"manual_ips": '["192.168.2.52"]'}})
_kms_tel("windows", "/api/output/tabs_get.json", [{"NDIStatus": False, "enable": True, "id": "1", "name": "Main",
                                                   "takeTarget": 0}],
         {"windows": {"1": {"name": "Main", "in_use": True, "ndi": False, "take_target": 0}}},
         state_before={"windows": {"2": {"name": "Output 2"}}},
         expect_then_send=[{"method": "GET", "target": "/api/output/get.json?output=1"},
                           {"method": "GET", "target": "/api/window/get_window_position.json?output=1"},
                           {"method": "GET", "target": "/api/output/get_state_contol.json?output=1"},
                           {"method": "GET", "target": "/api/ndi/get_status.json?output=1"},
                           {"method": "GET", "target": "/api/ndi/get_take.json?output=1"}])
_kms_tel("window", "/api/output/get.json?output=1", {"layout_id": "4", "layout_number": 4, "layout": [
    {"id": 1, "stream_id": "a", "stream_name": "KILOVIEW (Chan 1)", "volume": "on", "imgUrl": "x", "pattern_name": "HDMI",
     "pattern_color": "#dd0606", "pattern_size": 14, "PTZEnable": True, "PVW": 1, "PGM": 0, "Active": 1,
     "frameRate": 30, "aspectRatio": "1080x720", "url": "http://192.168.2.33:8000/admin"}]},
    {"windows": {"1": {"layout": "4", "grid_count": 4}},
     "grids": {"1": {"1": {"source": "a", "source_name": "KILOVIEW (Chan 1)", "audio": "on", "name": "HDMI",
                           "name_color": "#dd0606", "name_size": 14, "pvw": True, "pgm": False, "active": 1,
                           "frame_rate": 30, "resolution": "1080x720", "url": "http://192.168.2.33:8000/admin"}}}},
    state_before={"grids": {"1": {"9": {"source": "gone"}}}})
_kms_tel("window-screen", "/api/window/get_window_position.json?output=1", {"screen": 2, "isFull": False, "isShow": False},
         {"windows": {"1": {"screen": 2, "full_screen": False, "shown": False}}})
_kms_tel("toolbar", "/api/output/get_state_contol.json?output=1", {
    "soundColumn": {"enable": 0}, "securityBox": {"enable": 0}, "centerline": {"enable": 0}, "nameEnable": {"enable": 1}},
    {"windows": {"1": {"toolbar": {"audio_meters": False, "safe_areas": False, "center_crosses": False, "names": True}}}})
_kms_tel("ndi-status", "/api/ndi/get_status.json?output=2", {"enable": True}, {"windows": {"2": {"ndi_streaming": True}}})
_kms_tel("take", "/api/ndi/get_take.json?output=1", {
    "autoTake": True, "output": "1", "takeTarget": "3", "centerLine": False, "title": False, "safeFrame": True,
    "volumeMeter": False, "border": False},
    {"windows": {"1": {"take": {"auto": True, "target": "3", "center_cross": False, "title": False, "safe_area": True,
                                "audio_meter": False, "border": False}}}})
_kms_tel("grid", "/api/output/get_single_layout.json?output_id=1&id=1", {
    "name": "HDMI", "size": 16, "color": "#265ddd", "safe": True, "soundColumn": True, "centerline": False},
    {"grids": {"1": {"1": {"name": "HDMI", "name_size": 16, "name_color": "#265ddd", "safe_area": True,
                           "audio_meter": True, "center_cross": False}}}})
_kms_tel("stream-mode", "/api/output/get_stream_mode.json?output=1&id=1", {"mode": "main"},
         {"grids": {"1": {"1": {"stream_mode": "main"}}}})


def _kms_reread(name, path, then, request=None, body='{"result":"ok"}'):
    inbound = {"path": path, "body": body}
    if request is not None:
        inbound["request"] = request
    telemetry(KMS, name, inbound_http=inbound, expect_state={},
              expect_then_send=[{"method": "GET", "target": t} for t in then])


_kms_reread("reread-layout", "/api/pattern/remove.json?id=1", ["/api/output/icon.json"])
_kms_reread("reread-window", "/api/ndi/apply_take.json", ["/api/output/tabs_get.json"], {"output": "1"})
_kms_reread("reread-ndi-status", "/api/ndi/set_status.json", ["/api/output/tabs_get.json"], {"output": "1"},
            body='{"output":"1","enable":true}')
_kms_reread("reread-grid", "/api/output/set_single_layout.json",
            ["/api/output/get_single_layout.json?output_id=1&id=2", "/api/output/get.json?output=1"],
            {"output": "1", "id": 2})
_kms_reread("reread-border", "/api/output/EnableBorder.json", ["/api/output/GetEnableBorder.json"], {"bordercolor": 1})
_kms_reread("reread-sources", "/api/webSite/refresh.json", ["/api/source/get.json"])
_kms_reread("reread-refused", "/api/output/delete_all.json", [], {"output": 1}, body='{"result":"error","msg":"x"}')
