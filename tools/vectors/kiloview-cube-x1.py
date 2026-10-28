# Kiloview CUBE X1: JSON over HTTP under /api/cube/v2 (CUBE X1 HTTP API v2,
# firmware 0.01.0085). Requests and replies follow the document's own curl
# examples and response examples: JSON bodies for POSTs, the form-encoded
# login (1.1), query strings for GETs, and the {"result", "msg"} envelope.
KX = "kiloview-cube-x1"
_P = "/api/cube/v2"
_OK = {"status": 200, "body": '{"result":"ok","msg":""}'}
_ACK = {"ok": {"kind": "ack"}}


def _post(command, input, path, body, **extra):
    V.append({"spec": KX, "command": command, "input": input,
              "expect_request": {"method": "POST", "target": _P + path, "body": body,
                                 "content_type": "application/json"}, **extra})


def _get(command, input, target, **extra):
    http(KX, command, input, "GET", _P + target, **extra)


def _msg(msg):
    return {"status": 200, "body": json.dumps({"result": "ok", "msg": msg}, separators=(",", ":"))}


# ── 1 User management ───────────────────────────────────────────────────
# 1.1: the document's form body, with an empty grant_type, scope, client_id
# and client_secret; the token comes back without the envelope.
V.append({"spec": KX, "command": "login", "input": {"username": "chenq", "password": "Admin123"},
          "expect_request": {"method": "POST", "target": _P + "/user/login",
                             "body": "grant_type=&username=chenq&password=Admin123&scope=&client_id=&client_secret=",
                             "content_type": "application/x-www-form-urlencoded"},
          "http_reply": {"status": 200, "body": '{"access_token":"eyJ.abc.def","token_type":"bearer","first_login":false}'},
          "expect_result": {"ok": {"kind": "value", "value": "eyJ.abc.def"}}})
# A password holding & and = is percent-encoded in the form.
V.append({"spec": KX, "command": "login", "file": "login.encoded",
          "input": {"username": "admin", "password": "a&b=c d"},
          "expect_request": {"method": "POST", "target": _P + "/user/login",
                             "body": "grant_type=&username=admin&password=a%26b%3Dc%20d&scope=&client_id=&client_secret="},
          # The document's error example: a wrong user name.
          "http_reply": {"status": 200, "body": '{"result":"error","error":{"code":"10002","info":"User name does not exist"}}'},
          "expect_result": {"error": {"error": "device_error"}}})
V.append({"spec": KX, "command": "logout", "input": {},
          "expect_request": {"method": "POST", "target": _P + "/user/logout"},
          "http_reply": _OK, "expect_result": _ACK})
_get("renew_token", {}, "/user/token/get",
     http_reply=_msg({"access_token": "eyJ.new.tok", "token_type": "bearer"}),
     expect_result={"ok": {"kind": "value", "value": "eyJ.new.tok"}})
# 1.9: the document's example sends only username and password.
_post("get_long_term_token", {"username": "username", "password": "password"}, "/user/token/permanent",
      '{"username":"username","password":"password"}',
      http_reply=_msg({"access_token": "eyJ.long.tok", "token_type": "bearer"}),
      expect_result={"ok": {"kind": "value", "value": "eyJ.long.tok"}})
_post("get_timed_token", {"username": "admin", "password": "admin", "minutes": 1440}, "/user/token/permanent",
      '{"username":"admin","password":"admin","minutes":1440}')
# 1.3-1.6, the document's examples.
_post("add_user", {"name": "user1", "alias": "user1", "password": "Admin123", "administrator": True,
                   "panel_management": True, "resource_management": True, "firmware_management": True},
      "/user/add",
      '{"name":"user1","alias":"user1","passwd":"Admin123","system_m":true,"panel_m_per":true,'
      '"resource_m_per":true,"firmware_m_per":true}',
      http_reply=_OK, expect_result=_ACK)
_post("delete_user", {"user": 4}, "/user/del", '{"user_id":4}')
_post("modify_user", {"user": 5, "alias": "user1", "administrator": False, "panel_management": True,
                      "resource_management": False, "firmware_management": False}, "/user/modify",
      '{"user_id":5,"alias":"user1","system_m":false,"panel_m_per":true,"resource_m_per":false,"firmware_m_per":false}')
_post("set_user_password", {"user": 5, "password": "Admin123"}, "/user/passwd/modify",
      '{"user_id":5,"passwd":"Admin123"}')
_USERS = [{"id": 5, "name": "user1", "alias": "user1", "role": 1, "panel_m_per": True, "resource_m_per": True,
           "firmware_m_per": True, "panel_per_list": [{"panel_id": 1, "view_per": True, "switch_per": False,
                                                        "modify_per": False}],
           "create_time": "2023-10-09T11:13:06.847033"}]
_get("get_users", {}, "/user/query", http_reply=_msg(_USERS),
     expect_result={"ok": {"kind": "value", "value": _USERS}})
_get("get_user", {"name": "user1"}, "/user/query?user_name=user1")
_get("find_users", {"filter": "user one"}, "/user/query?filter=user%20one")

# ── 2 System management ─────────────────────────────────────────────────
_get("get_system_info", {}, "/system/info")
_get("get_http_ports", {}, "/system/http/get", http_reply=_msg({"http_port": 80, "https_port": 443}),
     expect_result={"ok": {"kind": "value", "value": {"http_port": 80, "https_port": 443}}})
_post("set_http_ports", {"http": True, "https": True, "http_port": 80, "https_port": 443}, "/system/http/set",
      '{"enable_http":true,"enable_https":true,"http_port":80,"https_port":443}')

# ── 3 Log management ────────────────────────────────────────────────────
_get("get_operation_log", {"page": 1, "page_size": 10}, "/log/operate/record?page_index=1&page_size=10")
_get("find_operation_log", {"page": 1, "page_size": 10, "filter": "admin"},
     "/log/operate/record?page_index=1&page_size=10&filter=admin")
_get("download_operation_log", {}, "/log/operate/download",
     http_reply={"status": 200, "body": "2023-10-09 admin User login"},
     expect_result={"ok": {"kind": "value", "value": "2023-10-09 admin User login"}})

# ── 4 Service management ────────────────────────────────────────────────
V.append({"spec": KX, "command": "restart_service", "input": {},
          "expect_request": {"method": "POST", "target": _P + "/server/reboot"},
          "http_reply": {"status": 200, "body": '{"result":"ok","msg":null}'}, "expect_result": _ACK})
_get("get_version", {}, "/server/version", http_reply=_msg({"sn": "0000000000000", "version": "1.10.0084"}),
     expect_result={"ok": {"kind": "value", "value": {"sn": "0000000000000", "version": "1.10.0084"}}})

# ── 5 Resource management ───────────────────────────────────────────────
_get("get_inputs", {}, "/resource/input/all")
_post("add_manual_discovery", {"group": "public", "device_ip": "192.168.35.19"}, "/resource/input/manual/add",
      '{"manuals":[{"dis_g_name":"public","dis_d_ip":"192.168.35.19"}]}')
_post("add_manual_discoveries", {"manuals": [{"dis_d_ip": "192.168.35.19", "dis_g_name": "public"},
                                             {"dis_d_ip": "192.168.35.20", "dis_g_name": "studio"}]},
      "/resource/input/manual/add",
      '{"manuals":[{"dis_d_ip":"192.168.35.19","dis_g_name":"public"},{"dis_d_ip":"192.168.35.20","dis_g_name":"studio"}]}')
_post("add_auto_discovery", {"group": "test"}, "/resource/input/auto/add", '{"dis_g_name":"test"}')
_post("add_discovery_server", {"server_ip": "192.168.36.80"}, "/resource/input/server/add",
      '{"dis_g_name":"public","dis_s_ip":"192.168.36.80"}')
_post("delete_discovery", {"discovery": 1}, "/resource/input/discovery/del", '{"dis_id":1}')
_get("get_discovery_methods", {}, "/resource/input/discovery/all")
_get("discover_manual", {}, "/resource/input/manual/discovery")
_get("discover_auto", {"group": "public"}, "/resource/input/auto/discovery?dis_g_name=public")
_get("discover_server", {"group": "public", "server_ip": "192.168.40.11"},
     "/resource/input/server/discovery?dis_g_name=public&dis_s_ip=192.168.40.11")
_post("add_rotation_list", {"name": "aaa", "sources": [{"input_id": 1, "play_duration": 10},
                                                      {"input_id": 2, "play_duration": 10}]},
      "/resource/rotation-list/add",
      '{"name":"aaa","rotation_srcs":[{"input_id":1,"play_duration":10},{"input_id":2,"play_duration":10}]}')
_post("delete_rotation_list", {"list": 1}, "/resource/rotation-list/del", '{"id":1}')
_post("modify_rotation_list", {"list": 1, "name": "aaa", "sources": [{"input_id": 1, "play_duration": 10},
                                                                    {"id": 1, "input_id": 1, "play_duration": 10}]},
      "/resource/rotation-list/modify",
      '{"id":1,"name":"aaa","rotation_srcs":[{"input_id":1,"play_duration":10},{"id":1,"input_id":1,"play_duration":10}]}')
_get("get_rotation_lists", {}, "/resource/rotation-list/all")
_get("get_rotation_list", {"list": 1}, "/resource/rotation-list?id=1")
_get("get_rotation_list_status", {"list": 1}, "/resource/rotation-list/status?id=1",
     http_reply=_msg({"status": True}), expect_result={"ok": {"kind": "value", "value": True}})

# ── 6 Panel management ──────────────────────────────────────────────────
_get("get_panels", {}, "/panel/all")
_get("get_panel", {"panel": 1}, "/panel/detail?id=1")
_post("manage_panel_inputs", {"panel": 1, "inputs": [{"alias": "alias", "id": 1}]}, "/panel/input/manage",
      '{"id":1,"inputs":[{"alias":"alias","id":1}]}')
_OUT = {"alias": "Output1", "c_name": "Output1", "codec_k": 0, "collect": False, "dis_s_ip": "192.168.20.3",
        "g_name": "public", "id": 1, "label_id": 1, "lock": False, "name": "Output1", "router": False, "smooth": 100}
_post("manage_panel_outputs", {"panel": 1, "outputs": [_OUT]}, "/panel/output/manage",
      '{"id":1,"outputs":[' + json.dumps(_OUT, separators=(",", ":")) + "]}")
_post("set_panel_permissions", {"panel": 1, "users": [{"id": 1, "modify_per": True, "switch_per": True,
                                                       "view_per": True}]},
      "/panel/permission/modify",
      '{"id":1,"users":[{"id":1,"modify_per":true,"switch_per":true,"view_per":true}]}')
_get("get_panel_permissions", {"panel": 1}, "/panel/permission?id=1")
_post("route", {"panel": 1, "output": 1, "input": 1}, "/panel/output/input",
      '{"id":1,"output_id":1,"input_id":1}', http_reply=_OK, expect_result=_ACK)
# An error answer fails the command.
_post("route", {"panel": 1, "output": 3, "input": 9}, "/panel/output/input",
      '{"id":1,"output_id":3,"input_id":9}', file="route.error",
      http_reply={"status": 200, "body": '{"result":"error","error":{"code":"10010","info":"Output is locked"}}'},
      expect_result={"error": {"error": "device_error"}})
_post("disconnect_output", {"panel": 1, "output": 1}, "/panel/output/input",
      '{"id":1,"output_id":1,"input_id":null}')
_post("start_rotation", {"panel": 1, "output": 1, "list": 1, "loop": True}, "/panel/output/rotation-list",
      '{"id":1,"output_id":1,"r_id":1,"loop":true}')
_post("stop_rotation", {"panel": 1, "output": 1}, "/panel/output/rotation-list",
      '{"id":1,"output_id":1,"r_id":null,"loop":null}')
_post("select_input_all", {"panel": 1, "input": 1, "select": True}, "/panel/input/select",
      '{"id":1,"input_id":1,"select":true}')
_post("lock_output", {"panel": 1, "output": 1, "lock": True}, "/panel/output/lock",
      '{"id":1,"output_id":1,"lock":true}')
_post("favorite_output", {"panel": 1, "output": 1, "favorite": False}, "/panel/output/collect",
      '{"id":1,"output_id":1,"collect":false}')
_post("set_rotation_loop", {"panel": 1, "output": 1, "list": 1, "loop": True}, "/panel/output/rotation-list/loop",
      '{"id":1,"output_id":1,"r_id":1,"loop":true}')

# ── 7 Template management ───────────────────────────────────────────────
_post("add_template", {"panel": 1, "name": "temlate name"}, "/template/add", '{"id":1,"name":"temlate name"}')
_post("delete_template", {"template": 1}, "/template/del", '{"id":1}')
_post("rename_template", {"template": 1, "name": "template name"}, "/template/name", '{"id":1,"name":"template name"}')
_get("get_templates", {"panel": 1}, "/template/query?id=1")
_post("save_template", {"template": 1}, "/template/save", '{"id":1}')
_post("save_template_as", {"template": 1, "name": "new template name"}, "/template/save-as",
      '{"id":1,"name":"new template name"}')
_post("switch_template", {"template": 2, "save": True}, "/template/switch", '{"switch_id":2,"save":true}')
_post("undo", {"panel": 1}, "/template/cancel", '{"id":1}')

# ── 8 Message management (its own path, outside /api/cube/v2) ───────────
V.append({"spec": KX, "command": "get_webrtc_sdp", "input": {"source": "N60-ABC (Chan1)", "server_ip": "192.168.40.238"},
          "expect_request": {"method": "POST", "target": "/api/cube/CubeServer/ws/queryWebrtcSdp",
                             "body": '{"input_src_name_list":["N60-ABC (Chan1)"],"server_ip":"192.168.40.238"}'},
          "http_reply": _msg([{"input_src_name": "N60-ABC (Chan1)", "sdp": "v=0\r\n"}]),
          "expect_result": {"ok": {"kind": "value", "value": [{"input_src_name": "N60-ABC (Chan1)", "sdp": "v=0\r\n"}]}}})
V.append({"spec": KX, "command": "get_webrtc_sdps",
          "input": {"sources": ["N60-ABC (Chan1)", "N60-ABC (FULL)"], "server_ip": "192.168.40.238"},
          "expect_request": {"method": "POST", "target": "/api/cube/CubeServer/ws/queryWebrtcSdp",
                             "body": '{"input_src_name_list":["N60-ABC (Chan1)","N60-ABC (FULL)"],"server_ip":"192.168.40.238"}'}})


# ── Telemetry ───────────────────────────────────────────────────────────
def _tel(name, path, msg, expect_state, **extra):
    body = json.dumps({"result": "ok", "msg": msg}, separators=(",", ":"))
    telemetry(KX, name, inbound_http={"path": _P + path, "body": body, **extra.pop("http", {})},
              expect_state=expect_state, **extra)


# 2.1, shortened to two seconds of record and one interface each kind.
_tel("system-info", "/system/info", {
    "system_start_time": "2023-10-09T09:35:32.720201", "system_running_time": 7681.362058,
    "cpu_temp": {"c": 38, "f": 100}, "cpu": 21, "cpu_record": [23, 22], "mem_record": [21, 21], "mem": 21,
    "net_card": {
        "eth0": {"ip": "192.168.40.111", "bytes_sent": 171498, "bytes_recv": 32611131,
                 "bytes_sent_total": 526743523, "bytes_recv_total": 87709316169, "is_up": True, "speed": 1000,
                 "type": "Ethernet", "bytes_send_record": [185694, 175218], "bytes_recv_record": [33962457, 33031270]},
        "enp1s0f0": {"ip": "", "bytes_sent": 0, "bytes_recv": 0, "bytes_sent_total": 0, "bytes_recv_total": 0,
                     "is_up": False, "speed": 0, "type": "Fiber", "bytes_send_record": [0, 0],
                     "bytes_recv_record": [0, 0]}},
    "sys_recv_total_record": [33962457, 33031270], "sys_sent_total_record": [185694, 175218],
    "input_src_num": 16, "output_num": 32, "abnormal_input_src_num": 4, "abnormal_output_num": 8},
    {"system": {"start_time": "2023-10-09T09:35:32.720201", "running_time": 7681.362058,
                "cpu_temperature": 38, "cpu_temperature_f": 100, "cpu": 21, "memory": 21,
                "cpu_record": "[23,22]", "memory_record": "[21,21]",
                "received_record": "[33962457,33031270]", "sent_record": "[185694,175218]",
                "input_count": 16, "output_count": 32, "abnormal_input_count": 4, "abnormal_output_count": 8},
     "network": {"interfaces": {
         "eth0": {"ip": "192.168.40.111", "bytes_sent": 171498, "bytes_received": 32611131,
                  "bytes_sent_total": 526743523, "bytes_received_total": 87709316169, "up": True, "speed": 1000,
                  "type": "Ethernet", "sent_record": "[185694,175218]", "received_record": "[33962457,33031270]"},
         "enp1s0f0": {"ip": "", "bytes_sent": 0, "bytes_received": 0, "bytes_sent_total": 0,
                      "bytes_received_total": 0, "up": False, "speed": 0, "type": "Fiber",
                      "sent_record": "[0,0]", "received_record": "[0,0]"}}}})
_tel("http-ports", "/system/http/get", {"http_port": 80, "https_port": 443},
     {"system": {"http_port": 80, "https_port": 443}})
_tel("version", "/server/version", {"sn": "0000000000000", "version": "1.10.0084"},
     {"device": {"serial": "0000000000000", "version": "1.10.0084"}})
# 1.7: the whole list replaces the users (user 9 was deleted).
_tel("users", "/user/query", _USERS,
     {"users": {"5": {"name": "user1", "alias": "user1", "role": "administrator", "panel_management": True,
                      "resource_management": True, "firmware_management": True,
                      "created": "2023-10-09T11:13:06.847033",
                      "panels": {"1": {"view": True, "switch": False, "modify": False}}}}},
     state_before={"users": {"9": {"name": "gone", "role": "user"}}})
# A filtered query adds to the users without removing the others.
_tel("users-filtered", "/user/query?user_name=user1", _USERS,
     {"users": {"9": {"name": "other", "role": "user"},
                "5": {"name": "user1", "alias": "user1", "role": "administrator", "panel_management": True,
                      "resource_management": True, "firmware_management": True,
                      "created": "2023-10-09T11:13:06.847033",
                      "panels": {"1": {"view": True, "switch": False, "modify": False}}}}},
     state_before={"users": {"9": {"name": "other", "role": "user"}}})
# 6.6: every user's permissions on panel 1.
_tel("panel-permissions", "/panel/permission?id=1", [
    {"id": 1, "name": "admin", "alias": "admin", "role": 0, "view_per": True, "switch_per": True, "modify_per": True},
    {"id": 2, "name": "a", "alias": "a", "role": 2, "view_per": False, "switch_per": False, "modify_per": False}],
    {"users": {"1": {"panels": {"1": {"view": True, "switch": True, "modify": True}}},
               "2": {"panels": {"1": {"view": False, "switch": False, "modify": False}}}}})
# 5.1, the document's example; a null label_id leaves label_id out.
_tel("inputs", "/resource/input/all", [
    {"id": 1, "name": "CUBEX1 (dst-1211Output1)", "alias": "CUBEX1 (dst-1211Output1)", "c_name": "dst-1211Output1",
     "g_name": "public", "d_ip": "192.168.40.205", "d_port": 5961, "available": False, "label_id": None,
     "conn_type": 0, "dis_method": 1}],
    {"inputs": {"1": {"name": "CUBEX1 (dst-1211Output1)", "alias": "CUBEX1 (dst-1211Output1)",
                      "channel_name": "dst-1211Output1", "group": "public", "device_ip": "192.168.40.205",
                      "device_port": 5961, "available": False, "connection": "unicast", "discovery": "auto"}}},
    state_before={"inputs": {"7": {"name": "OLD (CAM)"}}})
# 5.6, the document's example: the methods replaced, and each one's sources
# read (the manual list once, each group, each server).
_tel("discovery-methods", "/resource/input/discovery/all", {
    "manual": [{"id": 2, "dis_method": 0, "dis_g_name": "public", "dis_d_ip": "192.168.35.19", "dis_s_ip": ""}],
    "auto_server": [
        {"id": 1, "dis_method": 1, "dis_g_name": "public", "dis_d_ip": "", "dis_s_ip": ""},
        {"id": 3, "dis_method": 1, "dis_g_name": "test", "dis_d_ip": None, "dis_s_ip": ""},
        {"id": 4, "dis_method": 2, "dis_g_name": "public", "dis_d_ip": None, "dis_s_ip": "192.168.36.80"}]},
    {"discovery": {"2": {"method": "manual", "group": "public", "device_ip": "192.168.35.19", "server_ip": ""},
                   "1": {"method": "auto", "group": "public", "device_ip": "", "server_ip": ""},
                   "3": {"method": "auto", "group": "test", "server_ip": ""},
                   "4": {"method": "server", "group": "public", "server_ip": "192.168.36.80"}}},
    state_before={"discovery": {"8": {"method": "manual", "group": "gone"}}},
    expect_then_send=[{"method": "GET", "target": _P + "/resource/input/manual/discovery"},
                      {"method": "GET", "target": _P + "/resource/input/auto/discovery?dis_g_name=public"},
                      {"method": "GET", "target": _P + "/resource/input/auto/discovery?dis_g_name=test"},
                      {"method": "GET", "target": _P + "/resource/input/server/discovery?dis_g_name=public&dis_s_ip=192.168.36.80"}])
# 5.7-5.9, the document's examples (one source each).
_tel("discovered-manual", "/resource/input/manual/discovery", [
    {"name": "CUBEX1 (dst-ttttOutput3)", "alias": "CUBEX1 (dst-ttttOutput3)", "c_name": "dst-ttttOutput3",
     "g_name": "*manual*", "d_ip": "192.168.40.7", "d_port": 5970, "url": "192.168.40.7:5970", "available": True,
     "dis_method": 0, "dis_g_name": "public", "dis_d_ip": "192.168.40.7", "dis_s_ip": None}],
    {"discovered": {"manual": {"192.168.40.7:5970": {"name": "CUBEX1 (dst-ttttOutput3)",
                                                     "channel_name": "dst-ttttOutput3", "group": "public",
                                                     "available": True}}}})
_tel("discovered-auto", "/resource/input/auto/discovery?dis_g_name=my%20group", [
    {"name": "N60-123456751 (Chan1)", "alias": "N60-123456751 (Chan1)", "c_name": "Chan1", "g_name": "my group",
     "d_ip": "192.168.40.38", "d_port": 5962, "url": "192.168.40.38:5962", "available": True, "dis_method": 1,
     "dis_g_name": "my group", "dis_d_ip": None, "dis_s_ip": None}],
    {"discovered": {"auto": {"my group": {"192.168.40.38:5962": {
        "name": "N60-123456751 (Chan1)", "channel_name": "Chan1", "available": True}}}}},
    state_before={"discovered": {"auto": {"my group": {"192.168.40.9:5961": {"name": "OLD (CAM)"}}}}})
_tel("discovered-server", "/resource/input/server/discovery?dis_g_name=public&dis_s_ip=192.168.40.7", [
    {"name": "CUBEX1 (dst-Output114444)", "alias": "CUBEX1 (dst-Output114444)", "c_name": "dst-Output114444",
     "g_name": "public", "d_ip": "192.168.40.7", "d_port": 5961, "url": "192.168.40.7:5961", "available": True,
     "dis_method": 2, "dis_g_name": "public", "dis_d_ip": None, "dis_s_ip": "192.168.40.7"}],
    {"discovered": {"server": {"192.168.40.7": {"public": {"192.168.40.7:5961": {
        "name": "CUBEX1 (dst-Output114444)", "channel_name": "dst-Output114444", "available": True}}}}}})
# 5.13-5.15.
_tel("rotation-lists", "/resource/rotation-list/all", [{"id": 1, "name": "aaa", "src_num": 2, "play_duration": 20}],
     {"rotation_lists": {"1": {"name": "aaa", "source_count": 2, "play_duration": 20}}},
     state_before={"rotation_lists": {"4": {"name": "gone"}}},
     expect_then_send=[{"method": "GET", "target": _P + "/resource/rotation-list?id=1"},
                       {"method": "GET", "target": _P + "/resource/rotation-list/status?id=1"}])
_tel("rotation-list", "/resource/rotation-list?id=1", {"id": 1, "name": "aaa", "rotation_srcs": [
    {"id": 1, "input_id": 1, "input_name": "CUBEX1 (dst-1211Output1)", "input_alias": "CUBEX1 (dst-1211Output1)",
     "input_c_name": "dst-1211Output1", "play_duration": 10},
    {"id": 2, "input_id": 2, "input_name": "CUBEX1 (dst-Output1)", "input_alias": "CUBEX1 (dst-Output1)",
     "input_c_name": "dst-Output1", "play_duration": 10}]},
    {"rotation_sources": {"1": {
        "0": {"source_id": 1, "input": 1, "input_name": "CUBEX1 (dst-1211Output1)",
              "input_alias": "CUBEX1 (dst-1211Output1)", "input_channel_name": "dst-1211Output1", "play_duration": 10},
        "1": {"source_id": 2, "input": 2, "input_name": "CUBEX1 (dst-Output1)",
              "input_alias": "CUBEX1 (dst-Output1)", "input_channel_name": "dst-Output1", "play_duration": 10}}}},
    state_before={"rotation_sources": {"1": {"2": {"source_id": 9, "input": 3}}}})
_tel("rotation-status", "/resource/rotation-list/status?id=1", {"status": True}, {"rotation_in_use": {"1": True}})
# 6.1: the panels replaced, and each one's details read.
_tel("panels", "/panel/all", {"num": 2, "panels": [
    {"id": 1, "name": "aaaa", "input_n": 2, "output_n": 5, "icon": 0, "enable": True},
    {"id": 2, "name": "bbbb", "input_n": 2, "output_n": 0, "icon": 0, "enable": True}]},
    {"panels": {"1": {"name": "aaaa", "input_count": 2, "output_count": 5, "icon": 0, "enabled": True},
                "2": {"name": "bbbb", "input_count": 2, "output_count": 0, "icon": 0, "enabled": True}}},
    expect_then_send=[{"method": "GET", "target": _P + "/panel/detail?id=1"},
                      {"method": "GET", "target": _P + "/panel/detail?id=2"}])
# 6.2, the document's example (one output shortened away): output 1 routed
# from input 2, output 2 from input 1; output 3, disconnected before, is gone.
_IN = {"name": "CUBEX1 (dst-1211Output1)", "alias": "CUBEX1 (dst-1211Output1)", "c_name": "dst-1211Output1",
       "g_name": "public", "d_ip": "192.168.40.205", "d_port": 5961, "url": "192.168.40.205:5961",
       "available": False, "dis_method": 1, "dis_g_name": "public", "dis_d_ip": None, "dis_s_ip": None,
       "conn_type": 0, "label_id": None, "resolution": "0x0@0HZ", "v_b": 0, "a_b": 0}


def _o(id, input_id, label, links, res):
    return {"id": id, "name": f"Output{id}", "alias": f"Output{id}", "c_name": f"Output{id}", "g_name": "public",
            "smooth": 0, "router": False, "dis_s_ip": "", "conn_type": 0, "mult_add": None, "mult_mask": None,
            "ttl_time": None, "codec_k": 0, "lock": False, "collect": False, "input_id": input_id, "r_id": None,
            "r_loop": None, "cri_id": None, "cri_index": None, "cr_progress": None, "label_id": label,
            "link_num": links, "resolution": res, "v_b": 0, "a_b": 0}


def _os(input_id, label, links, res, id):
    return {"name": f"Output{id}", "alias": f"Output{id}", "channel_name": f"Output{id}", "group": "public",
            "smooth": 0, "routed": False, "discovery_server_ip": "", "connection": "unicast",
            "on_switch": "keep_connection", "locked": False, "favorite": False, "input": input_id,
            "label_id": label, "connections": links, "resolution": res, "video_bitrate": 0, "audio_bitrate": 0}


_tel("panel-detail", "/panel/detail?id=1", {
    "id": 1, "name": "aaaa", "audio_status": False,
    "other_inputs": [dict(_IN, id=2, name="CUBEX1 (dst-Output1)", c_name="dst-Output1", resolution="1920x1080@50HZ")],
    "panel_inputs": [dict(_IN, id=1, label_id=3)],
    "panel_outputs": [_o(1, 2, 1, 2, "1920x1080@50HZ"), _o(2, 1, 2, 0, "0x0@0HZ")],
    "panel_templates": [{"id": 1, "name": "default", "start": False}, {"id": 3, "name": "1111", "start": True}],
    "rotation_lists": [], "cancel_num": 5},
    {"panel_info": {"1": {"undo_count": 5, "audio_status": False}},
     "panel_inputs": {"1": {"1": {"channel_name": "dst-1211Output1", "label_id": 3}}},
     "input_status": {"1": {"available": False, "resolution": "0x0@0HZ", "video_bitrate": 0, "audio_bitrate": 0},
                      "2": {"available": False, "resolution": "1920x1080@50HZ", "video_bitrate": 0,
                            "audio_bitrate": 0}},
     "panel_outputs": {"1": {"1": _os(2, 1, 2, "1920x1080@50HZ", 1), "2": _os(1, 2, 0, "0x0@0HZ", 2)}},
     "panel_templates": {"1": {"1": {"name": "default", "started": False}, "3": {"name": "1111", "started": True}}}},
    state_before={"panel_outputs": {"1": {"3": {"name": "Output3", "input": 4}}}})
# An output playing a rotation list, in a multicast and reset-on-switch setup.
_tel("panel-detail-rotation", "/panel/detail?id=2", {
    "id": 2, "name": "bbbb", "audio_status": False, "other_inputs": [], "panel_inputs": [],
    "panel_outputs": [dict(_o(4, 1, 4, 1, "1920x1080@50HZ"), router=True, conn_type=1, mult_add="239.255.0.1",
                           mult_mask="255.255.255.0", ttl_time=4, codec_k=1, lock=True, collect=True, r_id=1,
                           r_loop=True, cri_id=1, cri_index=0, cr_progress=6)],
    "panel_templates": [], "rotation_lists": [], "cancel_num": 0},
    {"panel_info": {"2": {"undo_count": 0, "audio_status": False}},
     "panel_outputs": {"2": {"4": dict(_os(1, 4, 1, "1920x1080@50HZ", 4), routed=True, connection="unicast_multicast",
                                       multicast_address="239.255.0.1", multicast_mask="255.255.255.0", ttl=4,
                                       on_switch="reset_connection", locked=True, favorite=True, rotation_list=1,
                                       rotation_loop=True, rotation_input=1, rotation_index=0,
                                       rotation_progress=6)}}})
_tel("templates", "/template/query?id=1", [{"id": 1, "name": "default", "start": False},
                                           {"id": 3, "name": "1111", "start": True}],
     {"panel_templates": {"1": {"1": {"name": "default", "started": False}, "3": {"name": "1111", "started": True}}}},
     state_before={"panel_templates": {"1": {"7": {"name": "deleted", "started": False}}}})

# Read-backs after a change the server accepted.
_OKB = '{"result":"ok","msg":""}'


def _reread(name, path, request, then, **extra):
    inbound = {"path": _P + path, "body": extra.pop("body", _OKB)}
    if request is not None:
        inbound["request"] = request
    telemetry(KX, name, inbound_http=inbound,
              expect_then_send=[{"method": "GET", "target": _P + t} for t in then],
              expect_state=extra.pop("expect_state", {}), **extra)


_reread("reread-user", "/user/add", {"name": "user1"}, ["/user/query"])
_reread("reread-http-ports", "/system/http/set", {"http_port": 80}, ["/system/http/get"])
_reread("reread-discovery", "/resource/input/server/add", {"dis_g_name": "public"},
        ["/resource/input/discovery/all", "/resource/input/all"])
_reread("reread-rotation", "/resource/rotation-list/modify", {"id": 1}, ["/resource/rotation-list/all"])
_reread("reread-rotation-delete", "/resource/rotation-list/del", {"id": 1}, ["/resource/rotation-list/all"],
        state_before={"rotation_sources": {"1": {"0": {"input": 1}}, "2": {"0": {"input": 2}}},
                      "rotation_in_use": {"1": True, "2": False}},
        expect_state={"rotation_sources": {"2": {"0": {"input": 2}}}, "rotation_in_use": {"2": False}})
_reread("reread-route", "/panel/output/input", {"id": 3, "output_id": 1, "input_id": 2}, ["/panel/detail?id=3"])
_reread("reread-lock", "/panel/output/lock", {"id": 1, "output_id": 1, "lock": True}, ["/panel/detail?id=1"])
_reread("reread-panel-inputs", "/panel/input/manage", {"id": 2, "inputs": []},
        ["/panel/detail?id=2", "/resource/input/all"])
_reread("reread-permissions", "/panel/permission/modify", {"id": 1, "users": []}, ["/panel/permission?id=1"])
_reread("reread-template-add", "/template/add", {"id": 2, "name": "t"}, ["/panel/detail?id=2"])
_reread("reread-template-switch", "/template/switch", {"switch_id": 2, "save": True}, ["/panel/all"])
# A refused change reads nothing again.
_reread("reread-route-error", "/panel/output/input", {"id": 3, "output_id": 1, "input_id": 2}, [],
        body='{"result":"error","error":{"code":"10010","info":"Output is locked"}}')

# The session: with a password, a command's first request is the login (a
# form, as the login command sends it). Every other vector leaves the
# password empty, so each command's own request is the one compared.
V.append({"spec": KX, "command": "logout", "input": {}, "file": "logout-login",
          "settings": {"username": "admin", "password": "admin"},
          "expect_request": {"method": "POST", "target": "/api/cube/v2/user/login",
                             "body": "grant_type=&username=admin&password=admin&scope=&client_id=&client_secret=",
                             "content_type": "application/x-www-form-urlencoded"}})
for _kx_v in V:
    if _kx_v["spec"] == KX and _kx_v.get("file") != "logout-login":
        _kx_v.setdefault("settings", {}).setdefault("password", "")
