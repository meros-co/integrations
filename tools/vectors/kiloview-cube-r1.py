R1 = "kiloview-cube-r1"
# Kiloview CUBE R1 (CUBE R1 API 2023.11): JSON over HTTP under /api/r1/, every
# answer {"result":"ok"[,"data":...]} or {"result":"error","msg":...}. Request
# bodies are the document's examples, written compactly; the project is the
# project_id setting (1 by default). Reply bodies are the document's examples.

_OK = '{"result":"ok"}'
_CT = "application/json"


def _post(command, input, path, body, **extra):
    V.append({"spec": R1, "command": command, "input": input,
              "expect_request": {"method": "POST", "target": path, "body": body, "content_type": _CT}, **extra})


# Users (2.1-2.3).
_post("login", {"username": "admin", "password": "admin"}, "/api/r1/users/login.json",
      '{"username":"admin","password":"admin"}',
      http_reply={"status": 200, "body": '{"data":{"alias":"admin","changed":true,'
                  '"token":"3ee69a2f13b71cc57061845bc3bf04c5","username":"admin"},"result":"ok"}'},
      expect_result={"ok": {"kind": "value", "value": "3ee69a2f13b71cc57061845bc3bf04c5"}})
_post("login", {"username": "admin", "password": "wrong"}, "/api/r1/users/login.json",
      '{"username":"admin","password":"wrong"}', file="login.refused",
      http_reply={"status": 200, "body": '{"msg":"Username or password is incorrect","result":"error"}'},
      expect_result={"error": {"error": "device_error"}})
http(R1, "logout", {}, "GET", "/api/r1/users/logout.json",
     http_reply={"status": 200, "body": _OK}, expect_result={"ok": {"kind": "ack"}})
# Sent with a token the recorder no longer accepts.
http(R1, "logout", {}, "GET", "/api/r1/users/logout.json", file="logout.token-error",
     settings={"token": "3ee69a2f13b71cc57061845bc3bf04c5"},
     http_reply={"status": 200, "body": '{"msg":"Token.Error","result":"error"}'},
     expect_result={"error": {"error": "device_error"}})
_post("change_password", {"username": "admin", "old_password": "admin", "new_password": "Admin1"},
      "/api/r1/users/modify.json", '{"oldpassword":"admin","username":"admin","password":"Admin1"}',
      http_reply={"status": 200, "body": _OK}, expect_result={"ok": {"kind": "ack"}})

# General configuration (1.1).
http(R1, "get_general_config", {}, "GET", "/api/r1/config/general/get.json",
     http_reply={"status": 200, "body": '{"result":"ok","data":{"deviceName":{"zh":"Cube R1","en":"Cube R1"},'
                 '"languageShow":false,"websocketPort":12346}}'},
     expect_result={"ok": {"kind": "value", "value": {"deviceName": {"zh": "Cube R1", "en": "Cube R1"},
                                                       "languageShow": False, "websocketPort": 12346}}})

# Source discovery (3.1-3.6).
http(R1, "list_sources", {}, "GET", "/api/r1/source/list.json")
_post("add_auto_discovery_group",
      {"name": "Studio", "server_enabled": True, "server": "192.168.1.1", "ndi_groups": ["kiloview", "NDI", "test"]},
      "/api/r1/source/add.json",
      '{"disc_name":"Studio","disc_type":"Auto","discovery_enable":true,"discovery_ip":"192.168.1.1",'
      '"group":["kiloview","NDI","test"]}',
      http_reply={"status": 200, "body": _OK}, expect_result={"ok": {"kind": "ack"}})
_post("add_manual_discovery_group",
      {"name": "Stage", "ndi_groups": ["kiloview", "NDI", "test"], "addresses": ["192.168.40.11", "192.168.40.12"]},
      "/api/r1/source/add.json",
      '{"disc_name":"Stage","disc_type":"Manual","group":["kiloview","NDI","test"],'
      '"ip":["192.168.40.11","192.168.40.12"]}')
http(R1, "get_discovery_group", {"group_id": "5"}, "GET", "/api/r1/source/get_param.json?disc_id=5")
_post("modify_auto_discovery_group",
      {"group_id": "1", "name": "Studio", "server_enabled": False, "server": "", "ndi_groups": ["public"]},
      "/api/r1/source/modify.json",
      '{"disc_id":"1","disc_name":"Studio","disc_type":"Auto","discovery_enable":false,"discovery_ip":"",'
      '"group":["public"]}')
_post("modify_manual_discovery_group",
      {"group_id": "1", "name": "Stage", "ndi_groups": ["kiloview"], "addresses": ["192.168.40.11"]},
      "/api/r1/source/modify.json",
      '{"disc_id":"1","disc_name":"Stage","disc_type":"Manual","group":["kiloview"],"ip":["192.168.40.11"]}')
_post("remove_discovery_group", {"group_id": "1"}, "/api/r1/source/remove.json", '{"disc_id":"1"}',
      http_reply={"status": 200, "body": _OK}, expect_result={"ok": {"kind": "ack"}})
_post("refresh_sources", {}, "/api/r1/source/refresh.json", '{"refresh":true}')

# Layouts (4.1-4.2).
http(R1, "list_layouts", {}, "GET", "/api/r1/layout/icon.json",
     http_reply={"status": 200, "body": '{"result":"ok","data":[{"id":1,"number":"1",'
                 '"position":[{"x":0,"y":0,"width":100,"height":100}]}]}'},
     expect_result={"ok": {"kind": "value", "value": [
         {"id": 1, "number": "1", "position": [{"x": 0, "y": 0, "width": 100, "height": 100}]}]}})
_post("set_layout", {"layout_id": 3, "windows": 9}, "/api/r1/layout/setLayout.json",
      '{"project_id":1,"layout_id":3,"layout_number":9}',
      http_reply={"status": 200, "body": _OK}, expect_result={"ok": {"kind": "ack"}})

# Monitoring windows (5.1-5.8).
http(R1, "get_output", {}, "GET", "/api/r1/output/get.json?project_id=1")
_post("set_window_source", {"position": 3, "source": "KILOVIEW wersdfdsdf"}, "/api/r1/output/setSource.json",
      '{"project_id":1,"position":3,"stream_id":"KILOVIEW wersdfdsdf","type":"source"}',
      http_reply={"status": 200, "body": _OK}, expect_result={"ok": {"kind": "ack"}})
# The document's answer when the user lacks the permission.
_post("set_window_source", {"position": 3, "source": "N6-00000000 (Channel-HX)", "operation": "display"},
      "/api/r1/output/setSource.json",
      '{"project_id":1,"position":3,"stream_id":"N6-00000000 (Channel-HX)","type":"display"}',
      file="set_window_source.out-of-auth",
      http_reply={"status": 200, "body": '{"result":"error","reason":"out-of-auth"}'},
      expect_result={"error": {"error": "device_error"}})
_post("set_window_name", {"position": 3, "name": ""}, "/api/r1/output/setName.json",
      '{"project_id":1,"position":3,"name":""}')
_post("set_window_display", {"position": 3, "show_all": True, "show_volume": True, "show_video": True},
      "/api/r1/output/setEnable.json",
      '{"project_id":1,"position":3,"showAll":true,"showVolume":true,"showVideo":true}')
_post("remove_window_source", {"position": 1}, "/api/r1/output/remove.json", '{"project_id":1,"position":1}')
_post("clear_window_sources", {}, "/api/r1/output/deleteAll.json", '{"project_id":1}')
# Another project, through the setting.
_post("clear_window_sources", {}, "/api/r1/output/deleteAll.json", '{"project_id":2}',
      file="clear_window_sources.project-2", settings={"project_id": 2})
_post("set_all_windows_visible", {"visible": False}, "/api/r1/output/setGlobalVisible.json",
      '{"project_id":1,"outputGlobalVisible":false}')
_post("set_window_audio", {"position": 6, "audio": "on"}, "/api/r1/output/window_audio_set.json",
      '{"project_id":1,"position":6,"volume":"on"}')

# Storage (6.1-6.2).
http(R1, "get_storage", {}, "GET", "/api/r1/storage/getStoInfo.json")
_post("set_storage", {"start_disk": "1", "split": "size", "limit_size": 10, "limit_time": 60},
      "/api/r1/storage/setStoLimit.json", '{"choose":"1","limitType":"size","limitSize":10,"limitTime":60}',
      http_reply={"status": 200, "body": _OK}, expect_result={"ok": {"kind": "ack"}})

# Recording (7.1-7.4).
http(R1, "get_recording_status", {}, "GET", "/api/r1/record/getRecStatus.json?project_id=1",
     http_reply={"status": 200, "body": '{"result":"ok","data":{"isRecording":true,"startTime":1622100648254,"msg":""}}'},
     expect_result={"ok": {"kind": "value", "value": {"isRecording": True, "startTime": 1622100648254, "msg": ""}}})
_post("start_recording", {}, "/api/r1/record/setRecStatus.json", '{"isRecording":true}',
      http_reply={"status": 200, "body": _OK}, expect_result={"ok": {"kind": "ack"}})
_post("stop_recording", {}, "/api/r1/record/setRecStatus.json", '{"isRecording":false}')
http(R1, "get_recording_settings", {}, "GET", "/api/r1/record/getinfo.json?project_id=1")
_post("set_recording_settings",
      {"encoding": "native", "sync": True, "scheduled_start": True, "start_time": "2023-04-07 14:34:33",
       "scheduled_stop": False, "stop_time": "2023-04-07 16:34:33"},
      "/api/r1/record/setinfo.json",
      '{"solution":0,"sync":true,"isStart":true,"startTime":"2023-04-07 14:34:33","isStop":false,'
      '"stopTime":"2023-04-07 16:34:33"}')
_post("set_recording_settings",
      {"encoding": "h265", "sync": False, "scheduled_start": False, "start_time": "2023-04-07 14:34:33",
       "scheduled_stop": True, "stop_time": "2023-04-07 16:34:33"},
      "/api/r1/record/setinfo.json",
      '{"solution":2,"sync":false,"isStart":false,"startTime":"2023-04-07 14:34:33","isStop":true,'
      '"stopTime":"2023-04-07 16:34:33"}', file="set_recording_settings.h265")

# Performance (8.1).
http(R1, "get_performance", {}, "GET", "/api/r1/performance/getSys.json",
     http_reply={"status": 200, "body": '{"result":"ok","data":{"cpu":45,"gpu":45,"temp":90,"mem_use":2560,'
                 '"mem_total":8192,"net_up":1024,"net_down":128}}'},
     expect_result={"ok": {"kind": "value", "value": {"cpu": 45, "gpu": 45, "temp": 90, "mem_use": 2560,
                                                       "mem_total": 8192, "net_up": 1024, "net_down": 128}}})

# WebRTC (9.1-9.2).
http(R1, "get_webrtc_offer", {"ip": "192.168.39.191"}, "GET",
     "/api/r1/webrtc/get_sdp.json?pro_id=1&win_id=1&ip=192.168.39.191",
     http_reply={"status": 200, "body": '{"data":{"sdp":"v=0\\r\\ns=-\\r\\n","session_id":"1"},"result":"ok"}'},
     expect_result={"ok": {"kind": "value", "value": {"sdp": "v=0\r\ns=-\r\n", "session_id": "1"}}})
http(R1, "remove_webrtc_session", {"session_id": "1"}, "GET", "/api/r1/webrtc/remove_session.json?session_id=1",
     http_reply={"status": 200, "body": _OK}, expect_result={"ok": {"kind": "ack"}})

# Playback files (10.1-10.3).
_post("list_recordings", {"disk_id": "1", "channel": "source-1", "from": "2023-4-10 13:09:39"},
      "/api/r1/playback/getPlayList.json",
      '{"disk_id":"1","channelName":"source-1","startTime":"2023-4-10 13:09:39","stopTime":""}',
      http_reply={"status": 200, "body": '{"result":"ok","data":[{"name":"14022023-09.48.21",'
                  '"videoSrc":"/data/Camera1/big_buck_bunny.mp4"}]}'},
      expect_result={"ok": {"kind": "value", "value": [
          {"name": "14022023-09.48.21", "videoSrc": "/data/Camera1/big_buck_bunny.mp4"}]}})
_post("get_recording_info", {"path": "/media/ndicore/309E-8297/Camera-2/16052023-06.31.53-1.mov"},
      "/api/r1/playback/getVideoInfo.json",
      '{"videoSrc":"/media/ndicore/309E-8297/Camera-2/16052023-06.31.53-1.mov"}')
_post("delete_recording", {"path": "/media/ndicore/309E-8297/source-1/1.mov"}, "/api/r1/playback/remove.json",
      '{"videoSrc":"/media/ndicore/309E-8297/source-1/1.mov"}',
      http_reply={"status": 200, "body": _OK}, expect_result={"ok": {"kind": "ack"}})

# System (11.1-11.7).
http(R1, "get_hostname", {}, "GET", "/api/r1/system/getHostname.json",
     http_reply={"status": 200, "body": '{"result":"ok","data":{"hostname":"CubeR1"}}'},
     expect_result={"ok": {"kind": "value", "value": "CubeR1"}})
_post("set_hostname", {"hostname": "cube"}, "/api/r1/system/setHostname.json", '{"hostname":"cube"}')
http(R1, "get_timezone", {}, "GET", "/api/r1/system/getTimeZone.json")
_post("set_timezone", {"timezone": "Asia/Aqtobe", "offset": "5"}, "/api/r1/system/setTimeZone.json",
      '{"timezone":"Asia/Aqtobe","offset":"5"}')
http(R1, "get_time_sync", {}, "GET", "/api/r1/system/getTimer.json")
_post("set_time_sync", {"mode": "ntp", "server": "192.168.1.168"}, "/api/r1/system/setTimer.json",
      '{"timeType":"ntp","ip":"192.168.1.168"}')
_post("sync_time_now", {}, "/api/r1/system/timer/apply.json", '{"project_id":1}')

# Network (12.1-12.2).
http(R1, "get_network", {}, "GET", "/api/r1/network/getNet.json")
_post("set_network", {"interface": "ofp1", "dhcp": False, "ip": "192.168.39.198", "netmask": "255.255.255.0",
                      "gateway": "192.168.39.254", "dns": "223.5.5.5"},
      "/api/r1/network/modifyNet.json",
      '{"device":"ofp1","dynamic":"n","dns":"223.5.5.5","ip":"192.168.39.198","state":"up",'
      '"gw":"192.168.39.254","sub":"255.255.255.0"}')

# Firmware (13.1).
http(R1, "get_firmware", {}, "GET", "/api/r1/firmware/get.json",
     http_reply={"status": 200, "body": '{"result":"ok","data":{"softwareVersion":"1.0.0","firmwareVersion":"1.0.0"}}'},
     expect_result={"ok": {"kind": "value", "value": {"softwareVersion": "1.0.0", "firmwareVersion": "1.0.0"}}})

# ── Telemetry ──────────────────────────────────────────────────────────────
telemetry(R1, "general-config", inbound_http={
    "path": "/api/r1/config/general/get.json",
    "body": '{"result":"ok","data":{"deviceName":{"zh":"Cube R1","en":"Cube R1"},"languageShow":false,'
            '"websocketPort":12346,"languageOptions":[{"value":"en","name":"English"}]}}'},
    expect_state={"device": {"name": "Cube R1", "language_switch": False, "websocket_port": 12346}})

# 3.1's example (its broken second group written whole); a group no longer
# listed is removed, and each group's settings are read.
telemetry(R1, "source-list", inbound_http={
    "path": "/api/r1/source/list.json",
    "body": '{"data":[{"disc_id":"1","disc_name":"auto-discovery","sources":['
            '{"id":"DECODER-00000000 (1_channel)","ip":"192.168.39.191:5961","name":"DECODER-00000000 (1_channel)"},'
            '{"id":"HOLLY (VLC 1)","ip":"192.168.39.168:5962","name":"HOLLY (VLC 1)"}]},'
            '{"disc_id":"5","disc_name":"43","sources":[{"id":"N20-24 (Channel-1)","ip":"192.168.43.24:5961",'
            '"name":"N20-24 (Channel-1)"}]}],"result":"ok"}'},
    state_before={"sources": {"groups": {"9": {"name": "Old"}}}},
    expect_then_send=[{"method": "GET", "target": "/api/r1/source/get_param.json?disc_id=1"},
                      {"method": "GET", "target": "/api/r1/source/get_param.json?disc_id=5"}],
    expect_state={"sources": {"groups": {
        "1": {"name": "auto-discovery", "sources": {
            "DECODER-00000000 (1_channel)": {"name": "DECODER-00000000 (1_channel)", "address": "192.168.39.191:5961"},
            "HOLLY (VLC 1)": {"name": "HOLLY (VLC 1)", "address": "192.168.39.168:5962"}}},
        "5": {"name": "43", "sources": {
            "N20-24 (Channel-1)": {"name": "N20-24 (Channel-1)", "address": "192.168.43.24:5961"}}}}}})
telemetry(R1, "discovery-group-auto", inbound_http={
    "path": "/api/r1/source/get_param.json?disc_id=1",
    "body": '{"data":{"disc_id":"1","disc_name":"Studio","disc_type":"Auto","discovery_enable":true,'
            '"discovery_ip":"192.168.1.1","group":["kiloview","NDI","test"]},"result":"ok"}'},
    expect_state={"sources": {"groups": {"1": {
        "name": "Studio", "type": "Auto", "server_enabled": True, "server": "192.168.1.1",
        "ndi_groups": {"0": "kiloview", "1": "NDI", "2": "test"}}}}})
telemetry(R1, "discovery-group-manual", inbound_http={
    "path": "/api/r1/source/get_param.json?disc_id=1",
    "body": '{"data":{"disc_id":"1","disc_name":"Stage","disc_type":"Manual","group":["kiloview"],'
            '"ip":["192.168.40.11","192.168.40.12"]},"result":"ok"}'},
    state_before={"sources": {"groups": {"1": {"name": "Studio", "ndi_groups": {"0": "a", "1": "b"}}}}},
    expect_state={"sources": {"groups": {"1": {
        "name": "Stage", "type": "Manual", "ndi_groups": {"0": "kiloview"},
        "addresses": {"0": "192.168.40.11", "1": "192.168.40.12"}}}}})
telemetry(R1, "source-change-rereads", inbound_http={"path": "/api/r1/source/remove.json", "body": _OK},
          expect_then_send=[{"method": "GET", "target": "/api/r1/source/list.json"}],
          expect_state={})

telemetry(R1, "layouts", inbound_http={
    "path": "/api/r1/layout/icon.json",
    "body": '{"result":"ok","data":[{"id":1,"number":"1","position":[{"x":0,"y":0,"width":100,"height":100}]},'
            '{"id":2,"number":"2","position":[{"x":0,"y":0,"width":50,"height":100},'
            '{"x":50,"y":0,"width":50,"height":100}]}]}'},
    expect_state={"layouts": {
        "1": {"windows": 1, "positions": {"1": {"x": 0.0, "y": 0.0, "width": 100.0, "height": 100.0}}},
        "2": {"windows": 2, "positions": {"1": {"x": 0.0, "y": 0.0, "width": 50.0, "height": 100.0},
                                          "2": {"x": 50.0, "y": 0.0, "width": 50.0, "height": 100.0}}}}})

telemetry(R1, "output", inbound_http={
    "path": "/api/r1/output/get.json?project_id=1",
    "body": '{"result":"ok","data":{"layout_id":"1","layout_number":1,"layout":[{"id":1,"name":"test",'
            '"stream_id":"KILOVIEW (Chan 1 )","stream_name":"KILOVIEW (Chan 1 )","bit_rate":40960,'
            '"ntp_state":"sync","showAll":true,"showVolume":true,"showVideo":true,"isKiloview":true,'
            '"volume":"off"}]}}'},
    state_before={"output": {"windows": {"4": {"name": "gone"}}}},
    expect_state={"output": {"layout_id": 1, "layout_number": 1, "windows": {"1": {
        "name": "test", "source": "KILOVIEW (Chan 1 )", "source_name": "KILOVIEW (Chan 1 )", "bit_rate": 40960,
        "ntp_state": "sync", "show_all": True, "show_volume": True, "show_video": True, "official": True,
        "audio": "off"}}}})
telemetry(R1, "window-change-rereads", inbound_http={"path": "/api/r1/output/setSource.json", "body": _OK},
          expect_then_send=[{"method": "GET", "target": "/api/r1/output/get.json?project_id=1"}],
          expect_state={})
telemetry(R1, "layout-change-rereads", inbound_http={"path": "/api/r1/layout/setLayout.json", "body": _OK},
          expect_then_send=[{"method": "GET", "target": "/api/r1/output/get.json?project_id=1"}],
          expect_state={})
# An error answer reads nothing again.
telemetry(R1, "window-change-error", inbound_http={
    "path": "/api/r1/output/setSource.json", "body": '{"result":"error","reason":"out-of-auth"}'},
    expect_then_send=[], expect_state={})

telemetry(R1, "storage", inbound_http={
    "path": "/api/r1/storage/getStoInfo.json",
    "body": '{"result":"ok","data":{"disk":[{"name":"disk1","id":"0","unlock":true,"state":"online","total":"900G",'
            '"used":"30M","rate":1,"speed":0,"recoding":true,"msg":"","channel":[{"id":"111333","name":"Camera1"},'
            '{"id":"1113323","name":"Camera2"}]},{"name":"disk2","id":"1","unlock":false,"state":"offline",'
            '"total":"","used":"","rate":1,"speed":0,"recoding":false,"msg":"","channel":[]}],'
            '"choose":"1","limitType":"size","limitSize":10,"limitTime":60}}'},
    expect_state={"storage": {
        "start_disk": "1", "split": "size", "limit_size": 10, "limit_time": 60,
        "disks": {
            "0": {"name": "disk1", "unlocked": True, "state": "online", "total": "900G", "used": "30M", "rate": 1,
                  "speed": 0, "recording": True, "message": "",
                  "channels": {"111333": {"name": "Camera1"}, "1113323": {"name": "Camera2"}}},
            "1": {"name": "disk2", "unlocked": False, "state": "offline", "total": "", "used": "", "rate": 1,
                  "speed": 0, "recording": False, "message": ""}}}})
telemetry(R1, "storage-change-rereads", inbound_http={"path": "/api/r1/storage/setStoLimit.json", "body": _OK},
          expect_then_send=[{"method": "GET", "target": "/api/r1/storage/getStoInfo.json"}],
          expect_state={})
telemetry(R1, "ws-disk-lock", inbound_ws='{"socketType":"storePathUpdate","result":"ok",'
                                         '"data":{"disk1":"unlock","disk2":"lock"}}',
          expect_then_send=[{"method": "GET", "target": "/api/r1/storage/getStoInfo.json"}],
          expect_state={})

telemetry(R1, "recording-status", inbound_http={
    "path": "/api/r1/record/getRecStatus.json?project_id=1",
    "body": '{"result":"ok","data":{"isRecording":true,"startTime":1622100648254,"msg":""}}'},
    expect_state={"record": {"recording": True, "start_time": 1622100648254, "message": ""}})
telemetry(R1, "recording-change-rereads", inbound_http={"path": "/api/r1/record/setRecStatus.json", "body": _OK},
          expect_then_send=[{"method": "GET", "target": "/api/r1/record/getRecStatus.json?project_id=1"},
                            {"method": "GET", "target": "/api/r1/storage/getStoInfo.json"}],
          expect_state={})
telemetry(R1, "ws-record-update", inbound_ws='{"update":"record","result":"ok"}',
          expect_then_send=[{"method": "GET", "target": "/api/r1/record/getRecStatus.json?project_id=1"},
                            {"method": "GET", "target": "/api/r1/storage/getStoInfo.json"}],
          expect_state={})
telemetry(R1, "recording-settings", inbound_http={
    "path": "/api/r1/record/getinfo.json?project_id=1",
    "body": '{"result":"ok","data":{"solution":1,"sync":true,"isStart":true,"startTime":"2023-04-07 14:34:33",'
            '"isStop":false,"stopTime":"2023-04-07 16:34:33"}}'},
    expect_state={"record": {"encoding": "h264", "sync": True, "scheduled_start": True,
                             "start_at": "2023-04-07 14:34:33", "scheduled_stop": False,
                             "stop_at": "2023-04-07 16:34:33"}})
telemetry(R1, "recording-settings-change-rereads", inbound_http={"path": "/api/r1/record/setinfo.json", "body": _OK},
          expect_then_send=[{"method": "GET", "target": "/api/r1/record/getinfo.json?project_id=1"}],
          expect_state={})

telemetry(R1, "performance", inbound_http={
    "path": "/api/r1/performance/getSys.json",
    "body": '{"result":"ok","data":{"cpu":45.5,"gpu":45,"temp":90,"mem_use":2560,"mem_total":8192,'
            '"net_up":1024,"net_down":128}}'},
    expect_state={"performance": {"cpu": 45.5, "gpu": 45.0, "temperature": 90, "memory_used": 2560,
                                  "memory_total": 8192, "network_up": 1024, "network_down": 128}})

telemetry(R1, "hostname", inbound_http={"path": "/api/r1/system/getHostname.json",
                                        "body": '{"result":"ok","data":{"hostname":"CubeR1"}}'},
          expect_state={"system": {"hostname": "CubeR1"}})
telemetry(R1, "hostname-change-rereads", inbound_http={"path": "/api/r1/system/setHostname.json", "body": _OK},
          expect_then_send=[{"method": "GET", "target": "/api/r1/system/getHostname.json"}], expect_state={})
telemetry(R1, "timezone", inbound_http={"path": "/api/r1/system/getTimeZone.json",
                                        "body": '{"result":"ok","data":{"timezone":"Asia/Aqtobe","offset":"5"}}'},
          expect_state={"system": {"timezone": "Asia/Aqtobe", "timezone_offset": "5"}})
telemetry(R1, "timezone-change-rereads", inbound_http={"path": "/api/r1/system/setTimeZone.json", "body": _OK},
          expect_then_send=[{"method": "GET", "target": "/api/r1/system/getTimeZone.json"},
                            {"method": "GET", "target": "/api/r1/system/getTimer.json"}], expect_state={})
telemetry(R1, "time-sync", inbound_http={"path": "/api/r1/system/getTimer.json",
                                         "body": '{"result":"ok","data":{"time":"1698989621283 ","timeType":"dev",'
                                                 '"ip":"192.168.1.168"}}'},
          expect_state={"system": {"time": "1698989621283 ", "time_sync": "dev", "ntp_server": "192.168.1.168"}})
telemetry(R1, "time-sync-change-rereads", inbound_http={"path": "/api/r1/system/timer/apply.json", "body": _OK},
          expect_then_send=[{"method": "GET", "target": "/api/r1/system/getTimer.json"}], expect_state={})

telemetry(R1, "network", inbound_http={
    "path": "/api/r1/network/getNet.json",
    "body": '{"result":"ok","data":[{"dynamic":"y","dns":"223.5.5.5","device":"eth0","ip":"192.168.39.96",'
            '"state":"up","mac":"32:30:11:11:11:ad","gw":"192.168.39.254","sub":"255.255.255.0"},'
            '{"dynamic":"n","dns":"223.5.5.5","device":"ofp1","ip":"192.168.39.98","state":"up",'
            '"mac":"32:30:11:11:11:af","gw":"192.168.39.254","sub":"255.255.255.0"}]}'},
    expect_state={"network": {"interfaces": {
        "eth0": {"dhcp": True, "ip": "192.168.39.96", "netmask": "255.255.255.0", "gateway": "192.168.39.254",
                 "dns": "223.5.5.5", "link": "up", "mac": "32:30:11:11:11:ad"},
        "ofp1": {"dhcp": False, "ip": "192.168.39.98", "netmask": "255.255.255.0", "gateway": "192.168.39.254",
                 "dns": "223.5.5.5", "link": "up", "mac": "32:30:11:11:11:af"}}}})
telemetry(R1, "network-change-rereads", inbound_http={"path": "/api/r1/network/modifyNet.json", "body": _OK},
          expect_then_send=[{"method": "GET", "target": "/api/r1/network/getNet.json"}], expect_state={})

telemetry(R1, "firmware", inbound_http={
    "path": "/api/r1/firmware/get.json",
    "body": '{"result":"ok","data":{"softwareVersion":"1.0.0","firmwareVersion":"1.0.0"}}'},
    expect_state={"firmware": {"software_version": "1.0.0", "firmware_version": "1.0.0"}})
