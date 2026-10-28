# Kiloview NDI CORE / NDI CORE MAX: JSON over HTTP under /api/cube/v2 (NDI
# CORE & CORE MAX HTTP API Instructions, V1.10.0084). Requests and replies
# follow the document's own curl examples and response examples: JSON bodies
# for POSTs, the form-encoded login (1.1), query strings for GETs, and the
# {"result", "msg"} envelope.
KC = "kiloview-ndi-core"
_KCP = "/api/cube/v2"
_KC_OK = {"status": 200, "body": '{"result":"ok","msg":""}'}
_KC_ACK = {"ok": {"kind": "ack"}}


def _kc_post(command, input, path, body, **extra):
    V.append({"spec": KC, "command": command, "input": input,
              "expect_request": {"method": "POST", "target": _KCP + path, "body": body,
                                 "content_type": "application/json"}, **extra})


def _kc_get(command, input, target, **extra):
    http(KC, command, input, "GET", _KCP + target, **extra)


def _kc_msg(msg):
    return {"status": 200, "body": json.dumps({"result": "ok", "msg": msg}, separators=(",", ":"))}


# ── 1 User management ───────────────────────────────────────────────────
V.append({"spec": KC, "command": "login", "input": {"username": "chenq", "password": "Admin123"},
          "expect_request": {"method": "POST", "target": _KCP + "/user/login",
                             "body": "grant_type=&username=chenq&password=Admin123&scope=&client_id=&client_secret=",
                             "content_type": "application/x-www-form-urlencoded"},
          "http_reply": {"status": 200, "body": '{"access_token":"eyJ.abc.def","token_type":"bearer","first_login":false}'},
          "expect_result": {"ok": {"kind": "value", "value": "eyJ.abc.def"}}})
# The document's error example: a user name that does not exist.
V.append({"spec": KC, "command": "login", "file": "login.error", "input": {"username": "nobody", "password": "x&y"},
          "expect_request": {"method": "POST", "target": _KCP + "/user/login",
                             "body": "grant_type=&username=nobody&password=x%26y&scope=&client_id=&client_secret="},
          "http_reply": {"status": 200, "body": '{"result":"error","error":{"code":"10002","info":"Username does not exist"}}'},
          "expect_result": {"error": {"error": "device_error"}}})
V.append({"spec": KC, "command": "logout", "input": {},
          "expect_request": {"method": "POST", "target": _KCP + "/user/logout"},
          "http_reply": _KC_OK, "expect_result": _KC_ACK})
_kc_get("renew_token", {}, "/user/token/get",
        http_reply=_kc_msg({"access_token": "eyJ.new.tok", "token_type": "bearer"}),
        expect_result={"ok": {"kind": "value", "value": "eyJ.new.tok"}})
_kc_post("get_long_term_token", {"username": "username", "password": "password"}, "/user/token/permanent",
         '{"username":"username","password":"password"}',
         http_reply=_kc_msg({"access_token": "eyJ.long.tok", "token_type": "bearer"}),
         expect_result={"ok": {"kind": "value", "value": "eyJ.long.tok"}})
_kc_post("get_timed_token", {"username": "admin", "password": "admin", "minutes": 600}, "/user/token/permanent",
         '{"username":"admin","password":"admin","minutes":600}')
_kc_post("add_user", {"name": "user1", "alias": "user1", "password": "Admin123", "administrator": True,
                      "panel_management": True, "resource_management": True, "firmware_management": True},
         "/user/add",
         '{"name":"user1","alias":"user1","passwd":"Admin123","system_m":true,"panel_m_per":true,'
         '"resource_m_per":true,"firmware_m_per":true}',
         http_reply=_KC_OK, expect_result=_KC_ACK)
_kc_post("delete_user", {"user": 4}, "/user/del", '{"user_id":4}')
_kc_post("modify_user", {"user": 5, "alias": "user1", "administrator": True, "panel_management": True,
                         "resource_management": True, "firmware_management": True}, "/user/modify",
         '{"user_id":5,"alias":"user1","system_m":true,"panel_m_per":true,"resource_m_per":true,"firmware_m_per":true}')
_kc_post("set_user_password", {"user": 5, "password": "Admin123"}, "/user/passwd/modify",
         '{"user_id":5,"passwd":"Admin123"}')
_KC_USERS = [{"id": 5, "name": "user1", "alias": "user1", "role": 1, "panel_m_per": True, "resource_m_per": True,
              "firmware_m_per": True, "panel_per_list": [{"panel_id": 1, "view_per": True, "switch_per": False,
                                                           "modify_per": False}],
              "create_time": "2023-10-09T11:13:06.847033"}]
_kc_get("get_users", {}, "/user/query", http_reply=_kc_msg(_KC_USERS),
        expect_result={"ok": {"kind": "value", "value": _KC_USERS}})
_kc_get("get_user", {"name": "user1"}, "/user/query?user_name=user1")
_kc_get("find_users", {"filter": "ad min"}, "/user/query?filter=ad%20min")

# ── 2 System, 3 Log, 4 Service management ───────────────────────────────
_kc_get("get_system_info", {}, "/system/info")
_kc_get("get_http_ports", {}, "/system/http/get", http_reply=_kc_msg({"http_port": 80, "https_port": 443}),
        expect_result={"ok": {"kind": "value", "value": {"http_port": 80, "https_port": 443}}})
_kc_get("get_operation_log", {"page": 1, "page_size": 10}, "/log/operate/record?page_index=1&page_size=10")
_kc_get("find_operation_log", {"page": 1, "page_size": 10, "filter": "admin"},
        "/log/operate/record?page_index=1&page_size=10&filter=admin")
_kc_get("download_operation_log", {}, "/log/operate/download",
        http_reply={"status": 200, "body": "2023-10-09 admin User login"},
        expect_result={"ok": {"kind": "value", "value": "2023-10-09 admin User login"}})
_kc_get("download_operation_log_filtered", {"filter": "admin"}, "/log/operate/download?filter=admin")
V.append({"spec": KC, "command": "restart_service", "input": {},
          "expect_request": {"method": "POST", "target": _KCP + "/server/reboot"},
          "http_reply": {"status": 200, "body": '{"result":"ok","msg":null}'}, "expect_result": _KC_ACK})
_kc_get("get_version", {}, "/server/version", http_reply=_kc_msg({"sn": "0000000000000", "version": "1.10.0084"}),
        expect_result={"ok": {"kind": "value", "value": {"sn": "0000000000000", "version": "1.10.0084"}}})

# ── 5 Resource management ───────────────────────────────────────────────
# 5.1, the document's example.
_kc_post("add_input", {"name": "inputsrc1", "alias": "inputsrc1", "channel_name": "inputsrc1-channel",
                       "device_ip": "192.168.35.126", "device_port": 5534, "url": "192.168.35.126:5534",
                       "discovery_ip": "192.168.35.126"},
         "/resource/input/add",
         '{"inputs":[{"name":"inputsrc1","alias":"inputsrc1","c_name":"inputsrc1-channel","g_name":"public",'
         '"d_ip":"192.168.35.126","d_port":5534,"url":"192.168.35.126:5534","dis_method":0,"dis_g_name":"*manual*",'
         '"dis_d_ip":"192.168.35.126","dis_s_ip":null}]}')
_KC_INS = [{"name": "a", "alias": "a", "c_name": "a", "g_name": "public", "d_ip": "10.0.0.5", "d_port": 5961,
            "url": "10.0.0.5:5961", "dis_method": 2, "dis_g_name": "public", "dis_d_ip": None,
            "dis_s_ip": "10.0.0.2"}]
_kc_post("add_inputs", {"inputs": _KC_INS}, "/resource/input/add",
         '{"inputs":' + json.dumps(_KC_INS, separators=(",", ":")) + "}")
_kc_post("delete_input", {"input": 3}, "/resource/input/del", '{"ids":[3]}')
_kc_post("delete_inputs", {"ids": [1, 2, 3]}, "/resource/input/del", '{"ids":[1,2,3]}')
_kc_post("modify_input", {"input": 1, "alias": "alias"}, "/resource/input/modify",
         '{"id":1,"alias":"alias","conn_type":0}')
_kc_get("get_inputs", {}, "/resource/input/all")
_kc_post("add_manual_discovery", {"group": "public", "device_ip": "192.168.35.19"}, "/resource/input/manual/add",
         '{"manuals":[{"dis_g_name":"public","dis_d_ip":"192.168.35.19"}]}')
_kc_post("add_manual_discoveries", {"manuals": [{"dis_d_ip": "192.168.35.19", "dis_g_name": "public"}]},
         "/resource/input/manual/add", '{"manuals":[{"dis_d_ip":"192.168.35.19","dis_g_name":"public"}]}')
_kc_post("add_auto_discovery", {"group": "test"}, "/resource/input/auto/add", '{"dis_g_name":"test"}')
_kc_post("add_discovery_server", {"server_ip": "192.168.36.80"}, "/resource/input/server/add",
         '{"dis_g_name":"public","dis_s_ip":"192.168.36.80"}')
_kc_post("delete_discovery", {"discovery": 1}, "/resource/input/discovery/del", '{"dis_id":1}')
_kc_get("get_discovery_methods", {}, "/resource/input/discovery/all")
_kc_get("discover_manual", {}, "/resource/input/manual/discovery")
_kc_get("discover_auto", {"group": "public"}, "/resource/input/auto/discovery?dis_g_name=public")
_kc_get("discover_server", {"group": "public", "server_ip": "192.168.40.11"},
        "/resource/input/server/discovery?dis_g_name=public&dis_s_ip=192.168.40.11")
# 5.13, the document's example.
_kc_post("add_output", {"channel_name": "output2-channel", "smooth": 1000}, "/resource/output/add",
         '{"outputs":[{"c_name":"output2-channel","g_name":"public","router":false,"codec_k":0,"dis_s_ip":"",'
         '"smooth":1000}]}')
_kc_post("add_outputs", {"outputs": [{"c_name": "o", "g_name": "studio", "router": True, "codec_k": 1,
                                      "dis_s_ip": "10.0.0.2", "smooth": 0}]}, "/resource/output/add",
         '{"outputs":[{"c_name":"o","g_name":"studio","router":true,"codec_k":1,"dis_s_ip":"10.0.0.2","smooth":0}]}')
_kc_post("delete_output", {"output": 2}, "/resource/output/del", '{"ids":[2]}')
_kc_post("delete_outputs", {"ids": [1, 2, 3]}, "/resource/output/del", '{"ids":[1,2,3]}')
_kc_post("modify_output", {"output": 3, "alias": "alias", "on_switch": "reset_connection"}, "/resource/output/modify",
         '{"id":3,"alias":"alias","codec_k":1}')
_kc_get("get_outputs", {}, "/resource/output/all")
_kc_post("add_rotation_list", {"name": "Rotation List 1", "sources": [{"input_id": 1, "play_duration": 10},
                                                                     {"input_id": 2, "play_duration": 10}]},
         "/resource/rotation-list/add",
         '{"name":"Rotation List 1","rotation_srcs":[{"input_id":1,"play_duration":10},{"input_id":2,"play_duration":10}]}')
_kc_post("delete_rotation_list", {"list": 1}, "/resource/rotation-list/del", '{"id":1}')
_kc_post("modify_rotation_list", {"list": 1, "name": "Rotation List 1",
                                  "sources": [{"input_id": 1, "play_duration": 10},
                                              {"id": 1, "input_id": 1, "play_duration": 10}]},
         "/resource/rotation-list/modify",
         '{"id":1,"name":"Rotation List 1","rotation_srcs":[{"input_id":1,"play_duration":10},'
         '{"id":1,"input_id":1,"play_duration":10}]}')
_kc_get("get_rotation_lists", {}, "/resource/rotation-list/all")
_kc_get("get_rotation_list", {"list": 1}, "/resource/rotation-list?id=1")
_kc_get("get_rotation_list_status", {"list": 1}, "/resource/rotation-list/status?id=1",
        http_reply=_kc_msg({"status": True}), expect_result={"ok": {"kind": "value", "value": True}})

# ── 6 Panel management ──────────────────────────────────────────────────
_KC_PU = [{"id": 1, "view_per": True, "switch_per": True, "modify_per": True}]
_KC_PUJ = '[{"id":1,"view_per":true,"switch_per":true,"modify_per":true}]'
_kc_post("add_panel", {"name": "new panel name", "users": _KC_PU}, "/panel/add",
         '{"name":"new panel name","icon":0,"users":' + _KC_PUJ + "}")
_kc_post("delete_panel", {"panel": 3}, "/panel/del", '{"ids":[3]}')
_kc_post("delete_panels", {"ids": [1, 2, 3]}, "/panel/del", '{"ids":[1,2,3]}')
_kc_post("modify_panel", {"panel": 1, "name": "panel name", "users": _KC_PU}, "/panel/modify",
         '{"id":1,"name":"panel name","users":' + _KC_PUJ + "}")
_kc_post("rename_panel", {"panel": 1, "name": "panel name"}, "/panel/name", '{"id":1,"name":"panel name"}')
_kc_post("set_panel_icon", {"panel": 1, "icon": 0}, "/panel/icon", '{"id":1,"icon":0}')
_kc_post("enable_panel", {"panel": 1, "enabled": False}, "/panel/enable", '{"id":1,"enable":false}')
_kc_get("get_panels", {}, "/panel/all")
_kc_get("get_panel", {"panel": 1}, "/panel/detail?id=1")
_kc_post("set_panel_permissions", {"panel": 1, "users": _KC_PU}, "/panel/permission/modify",
         '{"id":1,"users":' + _KC_PUJ + "}")
_kc_get("get_panel_permissions", {"panel": 1}, "/panel/permission?id=1")
_kc_post("manage_panel_inputs", {"panel": 1, "inputs": [{"id": 1, "alias": "alias"}]}, "/panel/input/manage",
         '{"id":1,"inputs":[{"id":1,"alias":"alias"}]}')
_kc_post("manage_panel_outputs", {"panel": 1, "outputs": [{"id": 1, "name": "name", "alias": "alias", "codec_k": 0}]},
         "/panel/output/manage", '{"id":1,"outputs":[{"id":1,"name":"name","alias":"alias","codec_k":0}]}')
_kc_post("route", {"panel": 1, "output": 1, "input": 1}, "/panel/output/input",
         '{"id":1,"output_id":1,"input_id":1}', http_reply=_KC_OK, expect_result=_KC_ACK)
_kc_post("route", {"panel": 1, "output": 3, "input": 9}, "/panel/output/input",
         '{"id":1,"output_id":3,"input_id":9}', file="route.error",
         http_reply={"status": 200, "body": '{"result":"error","error":{"code":"10010","info":"Output is locked"}}'},
         expect_result={"error": {"error": "device_error"}})
_kc_post("disconnect_output", {"panel": 1, "output": 1}, "/panel/output/input",
         '{"id":1,"output_id":1,"input_id":null}')
_kc_post("start_rotation", {"panel": 1, "output": 1, "list": 1}, "/panel/output/rotation-list",
         '{"id":1,"output_id":1,"r_id":1,"loop":true}')
_kc_post("stop_rotation", {"panel": 1, "output": 1}, "/panel/output/rotation-list",
         '{"id":1,"output_id":1,"r_id":null,"loop":null}')
_kc_post("select_input_all", {"panel": 1, "input": 1}, "/panel/input/select", '{"id":1,"input_id":1,"select":true}')
_kc_post("lock_output", {"panel": 1, "output": 1, "lock": True}, "/panel/output/lock",
         '{"id":1,"output_id":1,"lock":true}')
_kc_post("set_rotation_loop", {"panel": 1, "output": 1, "list": 1, "loop": True}, "/panel/output/rotation-list/loop",
         '{"id":1,"output_id":1,"r_id":1,"loop":true}')

# ── 7 Template management ───────────────────────────────────────────────
_kc_post("add_template", {"panel": 1, "name": "temlate name"}, "/template/add", '{"id":1,"name":"temlate name"}')
_kc_post("delete_template", {"template": 1}, "/template/del", '{"id":1}')
_kc_post("rename_template", {"template": 1, "name": "template name"}, "/template/name",
         '{"id":1,"name":"template name"}')
_kc_get("get_templates", {"panel": 1}, "/template/query?id=1")
_kc_post("save_template", {"template": 1}, "/template/save", '{"id":1}')
_kc_post("save_template_as", {"template": 1, "name": "new template name"}, "/template/save-as",
         '{"id":1,"name":"new template name"}')
_kc_post("switch_template", {"template": 2, "save": True}, "/template/switch", '{"switch_id":2,"save":true}')
_kc_post("undo", {"panel": 1}, "/template/cancel", '{"id":1}')

# ── 8 Authorization management ──────────────────────────────────────────
_KC_AUTH = {"machine_code": "6A1B233A-03025207-00055105-52055553-05065157", "auth_status": False, "auth_code": "",
            "expiration_time": "2024-08-24T16:37:31.829707", "days": 27}
_kc_get("get_authorization", {}, "/authorize/info", http_reply=_kc_msg(_KC_AUTH),
        expect_result={"ok": {"kind": "value", "value": _KC_AUTH}})
_kc_post("update_authorization", {"code": "xxxx"}, "/authorize/update", '{"auth_code":"xxxx"}')


# ── Telemetry ───────────────────────────────────────────────────────────
def _kc_tel(name, path, msg, expect_state, **extra):
    body = json.dumps({"result": "ok", "msg": msg}, separators=(",", ":"))
    telemetry(KC, name, inbound_http={"path": _KCP + path, "body": body, **extra.pop("http", {})},
              expect_state=expect_state, **extra)


# 2.1, shortened to two seconds of record and one interface.
_kc_tel("system-info", "/system/info", {
    "system_start_time": "2023-10-09T09:35:32.720201", "system_running_time": 7681.362058,
    "cpu_temp": {"c": 38, "f": 100}, "cpu": 21, "cpu_record": [23, 22], "mem_record": [21, 21], "mem": 21,
    "net_card": {
        "eth0": {"ip": "192.168.40.111", "bytes_sent": 171498, "bytes_recv": 32611131,
                 "bytes_sent_total": 526743523, "bytes_recv_total": 87709316169, "is_up": True, "speed": 1000,
                 "type": "Ethernet", "bytes_send_record": [185694, 175218], "bytes_recv_record": [33962457, 33031270]}},
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
                  "type": "Ethernet", "sent_record": "[185694,175218]", "received_record": "[33962457,33031270]"}}}},
    state_before={"network": {"interfaces": {"enp1s0f1": {"ip": "", "up": False}}}})
_kc_tel("http-ports", "/system/http/get", {"http_port": 80, "https_port": 443},
        {"system": {"http_port": 80, "https_port": 443}})
_kc_tel("version", "/server/version", {"sn": "0000000000000", "version": "1.10.0084"},
        {"device": {"serial": "0000000000000", "version": "1.10.0084"}})
_kc_tel("authorization", "/authorize/info", _KC_AUTH,
        {"authorization": {"machine_code": "6A1B233A-03025207-00055105-52055553-05065157", "authorized": False,
                           "code": "", "expiration": "2024-08-24T16:37:31.829707", "days_left": 27}})
_kc_tel("users", "/user/query", _KC_USERS,
        {"users": {"5": {"name": "user1", "alias": "user1", "role": "administrator", "panel_management": True,
                         "resource_management": True, "firmware_management": True,
                         "created": "2023-10-09T11:13:06.847033",
                         "panels": {"1": {"view": True, "switch": False, "modify": False}}}}},
        state_before={"users": {"9": {"name": "gone", "role": "user"}}})
_kc_tel("users-filtered", "/user/query?user_name=user1", _KC_USERS,
        {"users": {"9": {"name": "other", "role": "user"},
                   "5": {"name": "user1", "alias": "user1", "role": "administrator", "panel_management": True,
                         "resource_management": True, "firmware_management": True,
                         "created": "2023-10-09T11:13:06.847033",
                         "panels": {"1": {"view": True, "switch": False, "modify": False}}}}},
        state_before={"users": {"9": {"name": "other", "role": "user"}}})
_kc_tel("panel-permissions", "/panel/permission?id=1", [
    {"id": 1, "name": "admin", "alias": "admin", "role": 0, "view_per": True, "switch_per": True, "modify_per": True},
    {"id": 2, "name": "a", "alias": "a", "role": 2, "view_per": False, "switch_per": False, "modify_per": False}],
    {"users": {"1": {"panels": {"1": {"view": True, "switch": True, "modify": True}}},
               "2": {"panels": {"1": {"view": False, "switch": False, "modify": False}}}}})
# 5.4, the document's example.
_kc_tel("inputs", "/resource/input/all", [
    {"id": 1, "name": "CUBEX1 (dst-1211Output1)", "alias": "CUBEX1 (dst-1211Output1)", "c_name": "dst-1211Output1",
     "g_name": "public", "d_ip": "192.168.40.205", "d_port": 5961, "available": False, "label_id": None,
     "conn_type": 0, "dis_method": 1}],
    {"inputs": {"1": {"name": "CUBEX1 (dst-1211Output1)", "alias": "CUBEX1 (dst-1211Output1)",
                      "channel_name": "dst-1211Output1", "group": "public", "device_ip": "192.168.40.205",
                      "device_port": 5961, "available": False, "connection": "unicast", "discovery": "auto"}}},
    state_before={"inputs": {"7": {"name": "OLD (CAM)"}}})
# 5.16, the document's example (one output), and a multicast one.
_kc_tel("outputs", "/resource/output/all", [
    {"id": 1, "name": "Output1", "alias": "Output1", "c_name": "Output1", "g_name": "public", "smooth": 0,
     "router": False, "dis_s_ip": "", "codec_k": 0, "label_id": 1, "link_num": 2, "enable": True, "conn_type": 0,
     "mult_add": None, "mult_mask": None, "ttl_time": None},
    {"id": 2, "name": "Output2", "alias": "Studio", "c_name": "Output2", "g_name": "public", "smooth": 0,
     "router": True, "dis_s_ip": "10.0.0.2", "codec_k": 1, "label_id": 2, "link_num": 0, "enable": False,
     "conn_type": 1, "mult_add": "239.255.0.1", "mult_mask": "255.255.255.0", "ttl_time": 4}],
    {"outputs": {
        "1": {"name": "Output1", "alias": "Output1", "channel_name": "Output1", "group": "public", "smooth": 0,
              "routed": False, "discovery_server_ip": "", "on_switch": "keep_connection", "label_id": 1,
              "connections": 2, "enabled": True, "connection": "unicast"},
        "2": {"name": "Output2", "alias": "Studio", "channel_name": "Output2", "group": "public", "smooth": 0,
              "routed": True, "discovery_server_ip": "10.0.0.2", "on_switch": "reset_connection", "label_id": 2,
              "connections": 0, "enabled": False, "connection": "unicast_multicast",
              "multicast_address": "239.255.0.1", "multicast_mask": "255.255.255.0", "ttl": 4}}},
    state_before={"outputs": {"9": {"name": "Output9"}}})
# 5.9, the document's example.
_kc_tel("discovery-methods", "/resource/input/discovery/all", {
    "manual": [{"id": 2, "dis_method": 0, "dis_g_name": "public", "dis_d_ip": "192.168.35.19", "dis_s_ip": ""}],
    "auto_server": [
        {"id": 1, "dis_method": 1, "dis_g_name": "public", "dis_d_ip": "", "dis_s_ip": ""},
        {"id": 4, "dis_method": 2, "dis_g_name": "public", "dis_d_ip": None, "dis_s_ip": "192.168.36.80"}]},
    {"discovery": {"2": {"method": "manual", "group": "public", "device_ip": "192.168.35.19", "server_ip": ""},
                   "1": {"method": "auto", "group": "public", "device_ip": "", "server_ip": ""},
                   "4": {"method": "server", "group": "public", "server_ip": "192.168.36.80"}}},
    state_before={"discovery": {"8": {"method": "manual", "group": "gone"}}},
    expect_then_send=[{"method": "GET", "target": _KCP + "/resource/input/manual/discovery"},
                      {"method": "GET", "target": _KCP + "/resource/input/auto/discovery?dis_g_name=public"},
                      {"method": "GET", "target": _KCP + "/resource/input/server/discovery?dis_g_name=public&dis_s_ip=192.168.36.80"}])
_kc_tel("discovered-manual", "/resource/input/manual/discovery", [
    {"name": "CUBEX1 (dst-ttttOutput3)", "alias": "CUBEX1 (dst-ttttOutput3)", "c_name": "dst-ttttOutput3",
     "g_name": "*manual*", "d_ip": "192.168.40.7", "d_port": 5970, "url": "192.168.40.7:5970", "available": True,
     "dis_method": 0, "dis_g_name": "public", "dis_d_ip": "192.168.40.7", "dis_s_ip": None}],
    {"discovered": {"manual": {"192.168.40.7:5970": {"name": "CUBEX1 (dst-ttttOutput3)",
                                                     "channel_name": "dst-ttttOutput3", "group": "public",
                                                     "available": True}}}})
_kc_tel("discovered-auto", "/resource/input/auto/discovery?dis_g_name=public", [
    {"name": "NDICORE-MAX (dst-OnlyTest-Output1)", "alias": "NDICORE-MAX (dst-OnlyTest-Output1)",
     "c_name": "dst-OnlyTest-Output1", "g_name": "public", "d_ip": "192.168.40.19", "d_port": 5961,
     "url": "192.168.40.19:5961", "available": True, "dis_method": 1, "dis_g_name": "public", "dis_d_ip": None,
     "dis_s_ip": None}],
    {"discovered": {"auto": {"public": {"192.168.40.19:5961": {
        "name": "NDICORE-MAX (dst-OnlyTest-Output1)", "channel_name": "dst-OnlyTest-Output1", "available": True}}}}})
_kc_tel("discovered-server", "/resource/input/server/discovery?dis_g_name=public&dis_s_ip=192.168.40.7", [
    {"name": "CUBEX1 (dst-Output114444)", "alias": "CUBEX1 (dst-Output114444)", "c_name": "dst-Output114444",
     "g_name": "public", "d_ip": "192.168.40.7", "d_port": 5961, "url": "192.168.40.7:5961", "available": True,
     "dis_method": 2, "dis_g_name": "public", "dis_d_ip": None, "dis_s_ip": "192.168.40.7"}],
    {"discovered": {"server": {"192.168.40.7": {"public": {"192.168.40.7:5961": {
        "name": "CUBEX1 (dst-Output114444)", "channel_name": "dst-Output114444", "available": True}}}}}})
_kc_tel("rotation-lists", "/resource/rotation-list/all", [{"id": 1, "name": "aaa", "src_num": 2, "play_duration": 20}],
        {"rotation_lists": {"1": {"name": "aaa", "source_count": 2, "play_duration": 20}}},
        state_before={"rotation_lists": {"4": {"name": "gone"}}},
        expect_then_send=[{"method": "GET", "target": _KCP + "/resource/rotation-list?id=1"},
                          {"method": "GET", "target": _KCP + "/resource/rotation-list/status?id=1"}])
_kc_tel("rotation-list", "/resource/rotation-list?id=1", {"id": 1, "name": "aaa", "rotation_srcs": [
    {"id": 1, "input_id": 1, "input_name": "CUBEX1 (dst-1211Output1)", "input_alias": "CUBEX1 (dst-1211Output1)",
     "input_c_name": "dst-1211Output1", "play_duration": 10}]},
    {"rotation_sources": {"1": {
        "0": {"source_id": 1, "input": 1, "input_name": "CUBEX1 (dst-1211Output1)",
              "input_alias": "CUBEX1 (dst-1211Output1)", "input_channel_name": "dst-1211Output1",
              "play_duration": 10}}}},
    state_before={"rotation_sources": {"1": {"1": {"source_id": 9, "input": 3}}}})
_kc_tel("rotation-status", "/resource/rotation-list/status?id=1", {"status": True}, {"rotation_in_use": {"1": True}})
_kc_tel("panels", "/panel/all", {"num": 2, "panels": [
    {"id": 1, "name": "aaaa", "input_n": 2, "output_n": 5, "icon": 0, "enable": True},
    {"id": 2, "name": "bbbb", "input_n": 2, "output_n": 0, "icon": 0, "enable": False}]},
    {"panels": {"1": {"name": "aaaa", "input_count": 2, "output_count": 5, "icon": 0, "enabled": True},
                "2": {"name": "bbbb", "input_count": 2, "output_count": 0, "icon": 0, "enabled": False}}},
    expect_then_send=[{"method": "GET", "target": _KCP + "/panel/detail?id=1"},
                      {"method": "GET", "target": _KCP + "/panel/detail?id=2"}])
# 6.10, the document's example (rotation lists left out).
_KC_IN = {"name": "CUBEX1 (dst-1211Output1)", "alias": "CUBEX1 (dst-1211Output1)", "c_name": "dst-1211Output1",
          "g_name": "public", "d_ip": "192.168.40.205", "d_port": 5961, "url": "192.168.40.205:5961",
          "available": False, "dis_method": 1, "dis_g_name": "public", "dis_d_ip": None, "dis_s_ip": None,
          "conn_type": 0, "label_id": None, "resolution": "0x0@0HZ", "v_b": 0, "a_b": 0}


def _kc_o(id, input_id, links, res):
    return {"id": id, "name": f"Output{id}", "alias": f"Output{id}", "c_name": f"Output{id}", "g_name": "public",
            "smooth": 0, "router": False, "dis_s_ip": "", "conn_type": 0, "mult_add": None, "mult_mask": None,
            "ttl_time": None, "codec_k": 0, "lock": False, "collect": False, "input_id": input_id, "r_id": None,
            "r_loop": None, "cri_id": None, "cri_index": None, "cr_progress": None, "label_id": id,
            "link_num": links, "resolution": res, "v_b": 0, "a_b": 0}


def _kc_os(id, input_id, links, res):
    return {"name": f"Output{id}", "alias": f"Output{id}", "channel_name": f"Output{id}", "group": "public",
            "smooth": 0, "routed": False, "discovery_server_ip": "", "connection": "unicast",
            "on_switch": "keep_connection", "locked": False, "favorite": False, "input": input_id,
            "connections": links, "resolution": res, "video_bitrate": 0, "audio_bitrate": 0}


_kc_tel("panel-detail", "/panel/detail?id=1", {
    "id": 1, "name": "aaaa", "audio_status": False,
    "other_inputs": [dict(_KC_IN, id=2, name="CUBEX1 (dst-Output1)", alias="CUBEX1 (dst-Output1)",
                          c_name="dst-Output1", resolution="1920x1080@50HZ")],
    "panel_inputs": [dict(_KC_IN, id=1)],
    "panel_outputs": [_kc_o(1, 2, 2, "1920x1080@50HZ"), _kc_o(2, 1, 0, "0x0@0HZ")],
    "panel_templates": [{"id": 1, "name": "default", "start": False}, {"id": 3, "name": "1111", "start": True}],
    "rotation_lists": [], "cancel_num": 5},
    {"panel_info": {"1": {"undo_count": 5, "audio_status": False}},
     "panel_inputs": {"1": {"1": {"alias": "CUBEX1 (dst-1211Output1)", "channel_name": "dst-1211Output1"}}},
     "input_status": {"1": {"available": False, "resolution": "0x0@0HZ", "video_bitrate": 0, "audio_bitrate": 0},
                      "2": {"available": False, "resolution": "1920x1080@50HZ", "video_bitrate": 0,
                            "audio_bitrate": 0}},
     "panel_outputs": {"1": {"1": _kc_os(1, 2, 2, "1920x1080@50HZ"), "2": _kc_os(2, 1, 0, "0x0@0HZ")}},
     "panel_templates": {"1": {"1": {"name": "default", "started": False}, "3": {"name": "1111", "started": True}}}},
    state_before={"panel_outputs": {"1": {"3": {"name": "Output3", "input": 4}}}})
_kc_tel("panel-detail-rotation", "/panel/detail?id=2", {
    "id": 2, "name": "bbbb", "audio_status": True, "other_inputs": [], "panel_inputs": [],
    "panel_outputs": [dict(_kc_o(4, 1, 1, "1920x1080@50HZ"), router=True, conn_type=1, mult_add="239.255.0.1",
                           mult_mask="255.255.255.0", ttl_time=4, codec_k=1, lock=True, collect=True, r_id=1,
                           r_loop=True, cri_id=1, cri_index=0, cr_progress=6)],
    "panel_templates": [], "rotation_lists": [], "cancel_num": 0},
    {"panel_info": {"2": {"undo_count": 0, "audio_status": True}},
     "panel_outputs": {"2": {"4": dict(_kc_os(4, 1, 1, "1920x1080@50HZ"), routed=True, connection="unicast_multicast",
                                       multicast_address="239.255.0.1", multicast_mask="255.255.255.0", ttl=4,
                                       on_switch="reset_connection", locked=True, favorite=True, rotation_list=1,
                                       rotation_loop=True, rotation_input=1, rotation_index=0,
                                       rotation_progress=6)}}})
_kc_tel("templates", "/template/query?id=1", [{"id": 1, "name": "default", "start": False},
                                              {"id": 3, "name": "1111", "start": True}],
        {"panel_templates": {"1": {"1": {"name": "default", "started": False},
                                   "3": {"name": "1111", "started": True}}}},
        state_before={"panel_templates": {"1": {"7": {"name": "deleted", "started": False}}}})

# Read-backs after a change the server accepted.
_KC_OKB = '{"result":"ok","msg":""}'


def _kc_reread(name, path, request, then, **extra):
    inbound = {"path": _KCP + path, "body": extra.pop("body", _KC_OKB)}
    if request is not None:
        inbound["request"] = request
    telemetry(KC, name, inbound_http=inbound,
              expect_then_send=[{"method": "GET", "target": _KCP + t} for t in then],
              expect_state=extra.pop("expect_state", {}), **extra)


_kc_reread("reread-user", "/user/add", {"name": "user1"}, ["/user/query"])
_kc_reread("reread-authorization", "/authorize/update", {"auth_code": "xxxx"}, ["/authorize/info"])
_kc_reread("reread-input", "/resource/input/modify", {"id": 1}, ["/resource/input/all", "/panel/all"])
_kc_reread("reread-output", "/resource/output/add", {"outputs": []}, ["/resource/output/all", "/panel/all"])
_kc_reread("reread-discovery", "/resource/input/server/add", {"dis_g_name": "public"},
           ["/resource/input/discovery/all", "/resource/input/all"])
_kc_reread("reread-rotation", "/resource/rotation-list/add", {"name": "r"}, ["/resource/rotation-list/all"])
_kc_reread("reread-rotation-delete", "/resource/rotation-list/del", {"id": 1}, ["/resource/rotation-list/all"],
           state_before={"rotation_sources": {"1": {"0": {"input": 1}}, "2": {"0": {"input": 2}}},
                         "rotation_in_use": {"1": True, "2": False}},
           expect_state={"rotation_sources": {"2": {"0": {"input": 2}}}, "rotation_in_use": {"2": False}})
_kc_reread("reread-panel-enable", "/panel/enable", {"id": 1, "enable": False}, ["/panel/all"])
_kc_reread("reread-panel-modify", "/panel/modify", {"id": 2, "name": "p", "users": []},
           ["/panel/all", "/panel/permission?id=2"])
_kc_reread("reread-route", "/panel/output/input", {"id": 3, "output_id": 1, "input_id": 2}, ["/panel/detail?id=3"])
_kc_reread("reread-panel-inputs", "/panel/input/manage", {"id": 2, "inputs": []},
           ["/panel/detail?id=2", "/resource/input/all"])
_kc_reread("reread-panel-outputs", "/panel/output/manage", {"id": 2, "outputs": []},
           ["/panel/detail?id=2", "/resource/output/all"])
_kc_reread("reread-permissions", "/panel/permission/modify", {"id": 1, "users": []}, ["/panel/permission?id=1"])
_kc_reread("reread-template-cancel", "/template/cancel", {"id": 2}, ["/panel/detail?id=2"])
_kc_reread("reread-template-switch", "/template/switch", {"switch_id": 2, "save": True}, ["/panel/all"])
_kc_reread("reread-route-error", "/panel/output/input", {"id": 3, "output_id": 1, "input_id": 2}, [],
           body='{"result":"error","error":{"code":"10010","info":"Output is locked"}}')
# Downloads saved to the host file named (never written by a vector).
V.append({"spec": "kiloview-ndi-core", "command": "download_logs", "input": {"to": {"file": "logs.tar.gz"}}, "expect_request": {"method": "GET", "target": "/api/cube/v2/log/download"}})


# The session: with a password, a command's first request is the login (a
# form, as the login command sends it). Every other vector leaves the
# password empty, so each command's own request is the one compared.
V.append({"spec": KC, "command": "logout", "input": {}, "file": "logout-login",
          "settings": {"username": "admin", "password": "admin"},
          "expect_request": {"method": "POST", "target": "/api/cube/v2/user/login",
                             "body": "grant_type=&username=admin&password=admin&scope=&client_id=&client_secret=",
                             "content_type": "application/x-www-form-urlencoded"}})
for _kc_v in V:
    if _kc_v["spec"] == KC and _kc_v.get("file") != "logout-login":
        _kc_v.setdefault("settings", {}).setdefault("password", "")
